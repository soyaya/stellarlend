use soroban_sdk::{contracterror, contracttype, Address, Env, String, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum MevProtectionError {
    InvalidConfig = 1,
    CommitNotFound = 2,
    CommitNotReady = 3,
    CommitExpired = 4,
    Unauthorized = 5,
    FeeCapExceeded = 6,
    InvalidAmount = 7,
    InvalidOperation = 8,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SensitiveOperation {
    Borrow,
    Withdraw,
    Liquidate,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TxOrderingHint {
    Default,
    PrivateMempool,
    BatchAuction,
    DelayedReveal,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MevProtectionConfig {
    pub commit_delay_secs: u64,
    pub commit_expiry_secs: u64,
    pub suspicious_window_secs: u64,
    pub fee_smoothing_bps: i128,
    pub base_protection_fee_bps: i128,
    pub surge_protection_fee_bps: i128,
    pub sandwich_threshold_bps: i128,
    pub private_mempool_enabled: bool,
    pub batching_enabled: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingCommit {
    pub id: u64,
    pub owner: Address,
    pub operation: SensitiveOperation,
    pub asset: Option<Address>,
    pub secondary_asset: Option<Address>,
    pub borrower: Option<Address>,
    pub amount: i128,
    pub max_fee_bps: i128,
    pub hint: TxOrderingHint,
    pub committed_at: u64,
    pub reveal_after: u64,
    pub expires_at: u64,
    pub commit_ledger: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrderingObservation {
    pub actor: Address,
    pub amount: i128,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrderingStats {
    pub suspicious_sequences: u64,
    pub sandwich_alerts: u64,
    pub last_alert_timestamp: u64,
    pub last_effective_fee_bps: i128,
}

#[contracttype]
#[derive(Clone)]
enum MevDataKey {
    Config,
    NextCommitId,
    Commit(u64),
    OrderingStats,
    LatestObservation(Symbol, Option<Address>),
    PreviousObservation(Symbol, Option<Address>),
    SmoothedFee(Symbol, Option<Address>),
}

const MAX_BPS: i128 = 10_000;

pub fn default_config() -> MevProtectionConfig {
    MevProtectionConfig {
        commit_delay_secs: 30,
        commit_expiry_secs: 300,
        suspicious_window_secs: 45,
        fee_smoothing_bps: 2_500,
        base_protection_fee_bps: 10,
        surge_protection_fee_bps: 60,
        sandwich_threshold_bps: 500,
        private_mempool_enabled: true,
        batching_enabled: true,
    }
}

pub fn configure(
    env: &Env,
    caller: Address,
    config: MevProtectionConfig,
) -> Result<(), MevProtectionError> {
    crate::risk_management::require_admin(env, &caller)
        .map_err(|_| MevProtectionError::Unauthorized)?;
    validate_config(&config)?;
    env.storage().persistent().set(&MevDataKey::Config, &config);
    Ok(())
}

pub fn get_config(env: &Env) -> MevProtectionConfig {
    env.storage()
        .persistent()
        .get(&MevDataKey::Config)
        .unwrap_or_else(default_config)
}

pub fn create_commit(
    env: &Env,
    owner: Address,
    operation: SensitiveOperation,
    asset: Option<Address>,
    secondary_asset: Option<Address>,
    borrower: Option<Address>,
    amount: i128,
    max_fee_bps: i128,
    hint: TxOrderingHint,
) -> Result<u64, MevProtectionError> {
    owner.require_auth();
    if amount <= 0 {
        return Err(MevProtectionError::InvalidAmount);
    }
    if !(0..=MAX_BPS).contains(&max_fee_bps) {
        return Err(MevProtectionError::InvalidConfig);
    }

    let cfg = get_config(env);
    let id = next_commit_id(env);
    let now = env.ledger().timestamp();
    let commit = PendingCommit {
        id,
        owner,
        operation,
        asset,
        secondary_asset,
        borrower,
        amount,
        max_fee_bps,
        hint,
        committed_at: now,
        reveal_after: now.saturating_add(cfg.commit_delay_secs),
        expires_at: now.saturating_add(cfg.commit_expiry_secs),
        commit_ledger: env.ledger().sequence(),
    };
    env.storage()
        .persistent()
        .set(&MevDataKey::Commit(id), &commit);
    Ok(id)
}

pub fn get_commit(env: &Env, commit_id: u64) -> Option<PendingCommit> {
    env.storage()
        .persistent()
        .get(&MevDataKey::Commit(commit_id))
}

pub fn cancel_commit(env: &Env, owner: Address, commit_id: u64) -> Result<(), MevProtectionError> {
    owner.require_auth();
    let commit = load_commit(env, commit_id)?;
    if commit.owner != owner {
        return Err(MevProtectionError::Unauthorized);
    }
    env.storage()
        .persistent()
        .remove(&MevDataKey::Commit(commit_id));
    Ok(())
}

pub fn preview_fee_bps(
    env: &Env,
    operation: SensitiveOperation,
    asset: Option<Address>,
    amount: i128,
) -> i128 {
    let cfg = get_config(env);
    let op_key = operation_symbol(env, &operation);
    let latest: Option<OrderingObservation> =
        env.storage()
            .persistent()
            .get(&MevDataKey::LatestObservation(
                op_key.clone(),
                asset.clone(),
            ));
    let prior = env
        .storage()
        .persistent()
        .get::<MevDataKey, i128>(&MevDataKey::SmoothedFee(op_key.clone(), asset.clone()))
        .unwrap_or(cfg.base_protection_fee_bps);

    let mut target = cfg.base_protection_fee_bps;
    if let Some(last) = latest {
        let now = env.ledger().timestamp();
        if now.saturating_sub(last.timestamp) <= cfg.suspicious_window_secs {
            target = cfg.surge_protection_fee_bps;
            if amounts_close(last.amount, amount, cfg.sandwich_threshold_bps) {
                target = target.saturating_add(cfg.base_protection_fee_bps);
            }
        }
    }

    let smoothed = prior
        .saturating_mul(MAX_BPS.saturating_sub(cfg.fee_smoothing_bps))
        .saturating_add(target.saturating_mul(cfg.fee_smoothing_bps))
        .saturating_div(MAX_BPS);
    smoothed.clamp(0, MAX_BPS)
}

pub fn execution_hint(env: &Env, requested: TxOrderingHint) -> TxOrderingHint {
    let cfg = get_config(env);
    match requested {
        TxOrderingHint::PrivateMempool if cfg.private_mempool_enabled => {
            TxOrderingHint::PrivateMempool
        }
        TxOrderingHint::BatchAuction if cfg.batching_enabled => TxOrderingHint::BatchAuction,
        TxOrderingHint::Default if cfg.private_mempool_enabled => TxOrderingHint::PrivateMempool,
        TxOrderingHint::Default if cfg.batching_enabled => TxOrderingHint::BatchAuction,
        _ => TxOrderingHint::DelayedReveal,
    }
}

pub fn user_guidance(env: &Env, operation: SensitiveOperation) -> String {
    match (operation, execution_hint(env, TxOrderingHint::Default)) {
        (SensitiveOperation::Borrow, TxOrderingHint::PrivateMempool) => String::from_str(
            env,
            "Commit borrow, wait for the reveal delay, then use a private mempool route.",
        ),
        (SensitiveOperation::Withdraw, TxOrderingHint::PrivateMempool) => String::from_str(
            env,
            "Commit withdrawal, wait for the reveal delay, then use a private mempool route.",
        ),
        (SensitiveOperation::Liquidate, TxOrderingHint::PrivateMempool) => String::from_str(
            env,
            "Commit liquidation, wait for the reveal delay, then use a private mempool route.",
        ),
        (_, TxOrderingHint::BatchAuction) => String::from_str(
            env,
            "Use commit/reveal and prefer batched execution during congested periods.",
        ),
        _ => String::from_str(
            env,
            "Use commit/reveal and avoid revealing during short bursts of ordering activity.",
        ),
    }
}

pub fn get_ordering_stats(env: &Env) -> OrderingStats {
    env.storage()
        .persistent()
        .get(&MevDataKey::OrderingStats)
        .unwrap_or(OrderingStats {
            suspicious_sequences: 0,
            sandwich_alerts: 0,
            last_alert_timestamp: 0,
            last_effective_fee_bps: 0,
        })
}

pub fn reveal_borrow(
    env: &Env,
    owner: Address,
    commit_id: u64,
) -> Result<(Option<Address>, i128, i128), MevProtectionError> {
    owner.require_auth();
    let commit = validate_reveal(env, &owner, commit_id, SensitiveOperation::Borrow)?;
    let effective_fee_bps = preview_fee_bps(
        env,
        SensitiveOperation::Borrow,
        commit.asset.clone(),
        commit.amount,
    );
    if effective_fee_bps > commit.max_fee_bps {
        return Err(MevProtectionError::FeeCapExceeded);
    }
    record_ordering_signal(
        env,
        owner,
        SensitiveOperation::Borrow,
        commit.asset.clone(),
        commit.amount,
        effective_fee_bps,
    );
    env.storage()
        .persistent()
        .remove(&MevDataKey::Commit(commit_id));
    Ok((commit.asset, commit.amount, effective_fee_bps))
}

pub fn reveal_withdraw(
    env: &Env,
    owner: Address,
    commit_id: u64,
) -> Result<(Option<Address>, i128), MevProtectionError> {
    owner.require_auth();
    let commit = validate_reveal(env, &owner, commit_id, SensitiveOperation::Withdraw)?;
    let effective_fee_bps = preview_fee_bps(
        env,
        SensitiveOperation::Withdraw,
        commit.asset.clone(),
        commit.amount,
    );
    if effective_fee_bps > commit.max_fee_bps {
        return Err(MevProtectionError::FeeCapExceeded);
    }
    record_ordering_signal(
        env,
        owner,
        SensitiveOperation::Withdraw,
        commit.asset.clone(),
        commit.amount,
        effective_fee_bps,
    );
    env.storage()
        .persistent()
        .remove(&MevDataKey::Commit(commit_id));
    Ok((commit.asset, commit.amount))
}

pub fn reveal_liquidation(
    env: &Env,
    owner: Address,
    commit_id: u64,
) -> Result<(Address, Option<Address>, Option<Address>, i128), MevProtectionError> {
    owner.require_auth();
    let commit = validate_reveal(env, &owner, commit_id, SensitiveOperation::Liquidate)?;
    let effective_fee_bps = preview_fee_bps(
        env,
        SensitiveOperation::Liquidate,
        commit.asset.clone(),
        commit.amount,
    );
    if effective_fee_bps > commit.max_fee_bps {
        return Err(MevProtectionError::FeeCapExceeded);
    }
    let borrower = commit
        .borrower
        .ok_or(MevProtectionError::InvalidOperation)?;
    record_ordering_signal(
        env,
        owner,
        SensitiveOperation::Liquidate,
        commit.asset.clone(),
        commit.amount,
        effective_fee_bps,
    );
    env.storage()
        .persistent()
        .remove(&MevDataKey::Commit(commit_id));
    Ok((
        borrower,
        commit.asset,
        commit.secondary_asset,
        commit.amount,
    ))
}

fn validate_reveal(
    env: &Env,
    owner: &Address,
    commit_id: u64,
    expected: SensitiveOperation,
) -> Result<PendingCommit, MevProtectionError> {
    let commit = load_commit(env, commit_id)?;
    if commit.owner != *owner {
        return Err(MevProtectionError::Unauthorized);
    }
    if commit.operation != expected {
        return Err(MevProtectionError::InvalidOperation);
    }
    let now = env.ledger().timestamp();
    if now < commit.reveal_after {
        return Err(MevProtectionError::CommitNotReady);
    }
    if now > commit.expires_at {
        return Err(MevProtectionError::CommitExpired);
    }
    Ok(commit)
}

fn record_ordering_signal(
    env: &Env,
    actor: Address,
    operation: SensitiveOperation,
    asset: Option<Address>,
    amount: i128,
    effective_fee_bps: i128,
) {
    let cfg = get_config(env);
    let op_key = operation_symbol(env, &operation);
    let latest_key = MevDataKey::LatestObservation(op_key.clone(), asset.clone());
    let previous_key = MevDataKey::PreviousObservation(op_key.clone(), asset.clone());
    let smoothed_key = MevDataKey::SmoothedFee(op_key, asset.clone());
    let now = env.ledger().timestamp();
    let latest: Option<OrderingObservation> = env.storage().persistent().get(&latest_key);
    let previous: Option<OrderingObservation> = env.storage().persistent().get(&previous_key);
    let mut stats = get_ordering_stats(env);

    if let Some(last) = latest.clone() {
        if now.saturating_sub(last.timestamp) <= cfg.suspicious_window_secs && last.actor != actor {
            stats.suspicious_sequences = stats.suspicious_sequences.saturating_add(1);
        }
    }

    if let (Some(prev), Some(last)) = (previous.clone(), latest.clone()) {
        let prev_recent = now.saturating_sub(prev.timestamp) <= cfg.suspicious_window_secs;
        let last_recent = now.saturating_sub(last.timestamp) <= cfg.suspicious_window_secs;
        if prev_recent
            && last_recent
            && prev.actor == actor
            && last.actor != actor
            && amounts_close(prev.amount, amount, cfg.sandwich_threshold_bps)
        {
            stats.sandwich_alerts = stats.sandwich_alerts.saturating_add(1);
            stats.last_alert_timestamp = now;
        }
    }

    stats.last_effective_fee_bps = effective_fee_bps;
    env.storage()
        .persistent()
        .set(&MevDataKey::OrderingStats, &stats);
    if let Some(last) = latest {
        env.storage().persistent().set(&previous_key, &last);
    }
    env.storage().persistent().set(
        &latest_key,
        &OrderingObservation {
            actor,
            amount,
            timestamp: now,
        },
    );
    env.storage()
        .persistent()
        .set(&smoothed_key, &effective_fee_bps);
}

fn load_commit(env: &Env, commit_id: u64) -> Result<PendingCommit, MevProtectionError> {
    env.storage()
        .persistent()
        .get(&MevDataKey::Commit(commit_id))
        .ok_or(MevProtectionError::CommitNotFound)
}

fn next_commit_id(env: &Env) -> u64 {
    let id = env
        .storage()
        .persistent()
        .get::<MevDataKey, u64>(&MevDataKey::NextCommitId)
        .unwrap_or(1);
    env.storage()
        .persistent()
        .set(&MevDataKey::NextCommitId, &id.saturating_add(1));
    id
}

fn validate_config(config: &MevProtectionConfig) -> Result<(), MevProtectionError> {
    if config.commit_delay_secs == 0
        || config.commit_expiry_secs <= config.commit_delay_secs
        || config.suspicious_window_secs == 0
        || !(0..=MAX_BPS).contains(&config.fee_smoothing_bps)
        || !(0..=MAX_BPS).contains(&config.base_protection_fee_bps)
        || !(0..=MAX_BPS).contains(&config.surge_protection_fee_bps)
        || !(0..=MAX_BPS).contains(&config.sandwich_threshold_bps)
    {
        return Err(MevProtectionError::InvalidConfig);
    }
    Ok(())
}

fn amounts_close(a: i128, b: i128, threshold_bps: i128) -> bool {
    if a == 0 && b == 0 {
        return true;
    }
    let max = if a.abs() > b.abs() { a.abs() } else { b.abs() };
    if max == 0 {
        return true;
    }
    let diff = (a - b).abs();
    diff.saturating_mul(MAX_BPS) <= max.saturating_mul(threshold_bps)
}

fn operation_symbol(env: &Env, operation: &SensitiveOperation) -> Symbol {
    match operation {
        SensitiveOperation::Borrow => Symbol::new(env, "borrow"),
        SensitiveOperation::Withdraw => Symbol::new(env, "withdraw"),
        SensitiveOperation::Liquidate => Symbol::new(env, "liquidate"),
    }
}

use soroban_sdk::{Address, Env, String, Vec, contracttype};

use crate::errors::GovernanceError;
use crate::storage;
use crate::types::ProposalType;

/// Minimum timelock delay (2 hours in seconds)
pub const MIN_TIMELOCK_DELAY: u64 = 7200;

/// Maximum timelock delay (48 hours in seconds)
pub const MAX_TIMELOCK_DELAY: u64 = 172800;

/// Default timelock delay (24 hours in seconds)
pub const DEFAULT_TIMELOCK_DELAY: u64 = 86400;

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum TimelockStatus {
    Pending,
    Ready,
    Executed,
    Cancelled,
    Expired,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct TimelockOperation {
    pub id: u64,
    pub proposal_type: ProposalType,
    pub description: String,
    pub proposer: Address,
    pub queued_at: u64,
    pub ready_at: u64,
    pub expires_at: u64,
    pub status: TimelockStatus,
    pub delay: u64,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct TimelockConfig {
    pub min_delay: u64,
    pub max_delay: u64,
    pub default_delay: u64,
    pub grace_period: u64, // Time after ready_at before expiration
}

impl Default for TimelockConfig {
    fn default() -> Self {
        Self {
            min_delay: MIN_TIMELOCK_DELAY,
            max_delay: MAX_TIMELOCK_DELAY,
            default_delay: DEFAULT_TIMELOCK_DELAY,
            grace_period: 86400, // 24 hours
        }
    }
}

/// Initialize timelock configuration
pub fn initialize_timelock(env: &Env, config: TimelockConfig) -> Result<(), GovernanceError> {
    if config.min_delay > config.max_delay {
        return Err(GovernanceError::InvalidTimelockConfig);
    }

    if config.default_delay < config.min_delay || config.default_delay > config.max_delay {
        return Err(GovernanceError::InvalidTimelockConfig);
    }

    let key = storage::GovernanceDataKey::TimelockConfig;
    env.storage().instance().set(&key, &config);

    let next_id_key = storage::GovernanceDataKey::NextTimelockId;
    env.storage().instance().set(&next_id_key, &0u64);

    Ok(())
}

/// Get timelock configuration
pub fn get_timelock_config(env: &Env) -> TimelockConfig {
    let key = storage::GovernanceDataKey::TimelockConfig;
    env.storage()
        .instance()
        .get(&key)
        .unwrap_or_default()
}

/// Queue a new timelock operation
pub fn queue_timelock_operation(
    env: &Env,
    proposer: Address,
    proposal_type: ProposalType,
    description: String,
    custom_delay: Option<u64>,
) -> Result<u64, GovernanceError> {
    proposer.require_auth();

    let config = get_timelock_config(env);
    let delay = custom_delay.unwrap_or(config.default_delay);

    if delay < config.min_delay || delay > config.max_delay {
        return Err(GovernanceError::InvalidTimelockDelay);
    }

    let next_id_key = storage::GovernanceDataKey::NextTimelockId;
    let operation_id: u64 = env.storage().instance().get(&next_id_key).unwrap_or(0);

    let now = env.ledger().timestamp();
    let ready_at = now + delay;
    let expires_at = ready_at + config.grace_period;

    let operation = TimelockOperation {
        id: operation_id,
        proposal_type,
        description,
        proposer: proposer.clone(),
        queued_at: now,
        ready_at,
        expires_at,
        status: TimelockStatus::Pending,
        delay,
    };

    let operation_key = storage::GovernanceDataKey::TimelockOperation(operation_id);
    env.storage().persistent().set(&operation_key, &operation);

    env.storage()
        .instance()
        .set(&next_id_key, &(operation_id + 1));

    // Emit event
    crate::events::TimelockQueuedEvent {
        operation_id,
        proposer,
        ready_at,
        expires_at,
        delay,
        timestamp: now,
    }
    .publish(env);

    Ok(operation_id)
}

/// Execute a timelock operation
pub fn execute_timelock_operation(
    env: &Env,
    executor: Address,
    operation_id: u64,
) -> Result<(), GovernanceError> {
    executor.require_auth();

    let operation_key = storage::GovernanceDataKey::TimelockOperation(operation_id);
    let mut operation: TimelockOperation = env
        .storage()
        .persistent()
        .get(&operation_key)
        .ok_or(GovernanceError::TimelockNotFound)?;

    if operation.status != TimelockStatus::Pending && operation.status != TimelockStatus::Ready {
        return Err(GovernanceError::InvalidTimelockStatus);
    }

    let now = env.ledger().timestamp();

    if now < operation.ready_at {
        return Err(GovernanceError::TimelockNotReady);
    }

    if now > operation.expires_at {
        operation.status = TimelockStatus::Expired;
        env.storage().persistent().set(&operation_key, &operation);
        return Err(GovernanceError::TimelockExpired);
    }

    // Execute the operation
    execute_proposal_type(env, &operation.proposal_type)?;

    operation.status = TimelockStatus::Executed;
    env.storage().persistent().set(&operation_key, &operation);

    // Emit event
    crate::events::TimelockExecutedEvent {
        operation_id,
        executor,
        timestamp: now,
    }
    .publish(env);

    Ok(())
}

/// Cancel a timelock operation (admin or proposer only)
pub fn cancel_timelock_operation(
    env: &Env,
    caller: Address,
    operation_id: u64,
) -> Result<(), GovernanceError> {
    caller.require_auth();

    let operation_key = storage::GovernanceDataKey::TimelockOperation(operation_id);
    let mut operation: TimelockOperation = env
        .storage()
        .persistent()
        .get(&operation_key)
        .ok_or(GovernanceError::TimelockNotFound)?;

    // Check authorization: must be proposer or admin
    let admin = crate::governance::get_admin(env).ok_or(GovernanceError::NotInitialized)?;
    if caller != operation.proposer && caller != admin {
        return Err(GovernanceError::Unauthorized);
    }

    if operation.status != TimelockStatus::Pending && operation.status != TimelockStatus::Ready {
        return Err(GovernanceError::InvalidTimelockStatus);
    }

    operation.status = TimelockStatus::Cancelled;
    env.storage().persistent().set(&operation_key, &operation);

    // Emit event
    crate::events::TimelockCancelledEvent {
        operation_id,
        caller,
        timestamp: env.ledger().timestamp(),
    }
    .publish(env);

    Ok(())
}

/// Get timelock operation
pub fn get_timelock_operation(env: &Env, operation_id: u64) -> Option<TimelockOperation> {
    let operation_key = storage::GovernanceDataKey::TimelockOperation(operation_id);
    env.storage().persistent().get(&operation_key)
}

/// Get all pending timelock operations
pub fn get_pending_timelock_operations(env: &Env) -> Vec<TimelockOperation> {
    let next_id_key = storage::GovernanceDataKey::NextTimelockId;
    let next_id: u64 = env.storage().instance().get(&next_id_key).unwrap_or(0);

    let mut pending = Vec::new(env);
    for id in 0..next_id {
        if let Some(operation) = get_timelock_operation(env, id) {
            if operation.status == TimelockStatus::Pending
                || operation.status == TimelockStatus::Ready
            {
                pending.push_back(operation);
            }
        }
    }

    pending
}

/// Update timelock configuration (admin only)
pub fn update_timelock_config(
    env: &Env,
    admin: Address,
    config: TimelockConfig,
) -> Result<(), GovernanceError> {
    admin.require_auth();

    let stored_admin = crate::governance::get_admin(env).ok_or(GovernanceError::NotInitialized)?;
    if admin != stored_admin {
        return Err(GovernanceError::Unauthorized);
    }

    if config.min_delay > config.max_delay {
        return Err(GovernanceError::InvalidTimelockConfig);
    }

    if config.default_delay < config.min_delay || config.default_delay > config.max_delay {
        return Err(GovernanceError::InvalidTimelockConfig);
    }

    let key = storage::GovernanceDataKey::TimelockConfig;
    env.storage().instance().set(&key, &config);

    Ok(())
}

/// Execute proposal type (internal helper)
fn execute_proposal_type(env: &Env, proposal_type: &ProposalType) -> Result<(), GovernanceError> {
    match proposal_type {
        ProposalType::MinCollateralRatio(ratio) => {
            crate::risk_params::set_risk_params(env, Some(*ratio), None, None, None)
                .map_err(|_| GovernanceError::ExecutionFailed)?;
        }
        ProposalType::RiskParams(mcr, lt, cf, li) => {
            crate::risk_params::set_risk_params(env, *mcr, *lt, *cf, *li)
                .map_err(|_| GovernanceError::ExecutionFailed)?;
        }
        ProposalType::InterestRateConfig(params) => {
            let admin = crate::governance::get_admin(env).ok_or(GovernanceError::NotInitialized)?;
            crate::interest_rate::update_interest_rate_config(
                env,
                admin,
                params.base_rate_bps,
                params.kink_utilization_bps,
                params.multiplier_bps,
                params.jump_multiplier_bps,
                params.rate_floor_bps,
                params.rate_ceiling_bps,
                params.spread_bps,
            )
            .map_err(|_| GovernanceError::ExecutionFailed)?;
        }
        ProposalType::PauseSwitch(op, paused) => {
            let admin = crate::governance::get_admin(env).ok_or(GovernanceError::NotInitialized)?;
            crate::risk_management::set_pause_switch(env, admin, op.clone(), *paused)
                .map_err(|_| GovernanceError::ExecutionFailed)?;
        }
        ProposalType::EmergencyPause(paused) => {
            let admin = crate::governance::get_admin(env).ok_or(GovernanceError::NotInitialized)?;
            crate::risk_management::set_emergency_pause(env, admin, *paused)
                .map_err(|_| GovernanceError::ExecutionFailed)?;
        }
        ProposalType::GenericAction(_) => {
            return Err(GovernanceError::InvalidProposalType);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger};

    #[test]
    fn test_initialize_timelock() {
        let env = Env::default();
        let config = TimelockConfig::default();

        initialize_timelock(&env, config.clone()).unwrap();
        let stored_config = get_timelock_config(&env);

        assert_eq!(stored_config.min_delay, config.min_delay);
        assert_eq!(stored_config.max_delay, config.max_delay);
        assert_eq!(stored_config.default_delay, config.default_delay);
    }

    #[test]
    fn test_queue_timelock_operation() {
        let env = Env::default();
        env.mock_all_auths();

        let config = TimelockConfig::default();
        initialize_timelock(&env, config).unwrap();

        let proposer = Address::generate(&env);
        let proposal_type = ProposalType::MinCollateralRatio(8000);
        let description = String::from_str(&env, "Test proposal");

        let operation_id =
            queue_timelock_operation(&env, proposer.clone(), proposal_type, description, None)
                .unwrap();

        let operation = get_timelock_operation(&env, operation_id).unwrap();
        assert_eq!(operation.status, TimelockStatus::Pending);
        assert_eq!(operation.proposer, proposer);
    }

    #[test]
    fn test_execute_timelock_too_early() {
        let env = Env::default();
        env.mock_all_auths();

        let config = TimelockConfig::default();
        initialize_timelock(&env, config).unwrap();

        let proposer = Address::generate(&env);
        let executor = Address::generate(&env);
        let proposal_type = ProposalType::MinCollateralRatio(8000);
        let description = String::from_str(&env, "Test proposal");

        let operation_id =
            queue_timelock_operation(&env, proposer, proposal_type, description, None).unwrap();

        // Try to execute immediately
        let result = execute_timelock_operation(&env, executor, operation_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_timelock_after_delay() {
        let env = Env::default();
        env.mock_all_auths();

        let config = TimelockConfig::default();
        initialize_timelock(&env, config.clone()).unwrap();

        let proposer = Address::generate(&env);
        let executor = Address::generate(&env);
        let proposal_type = ProposalType::MinCollateralRatio(8000);
        let description = String::from_str(&env, "Test proposal");

        let operation_id =
            queue_timelock_operation(&env, proposer, proposal_type, description, None).unwrap();

        // Advance time past the delay
        env.ledger().with_mut(|li| {
            li.timestamp += config.default_delay + 1;
        });

        // Initialize risk params first
        crate::risk_params::initialize_risk_params(&env).unwrap();

        // Now execution should succeed
        execute_timelock_operation(&env, executor, operation_id).unwrap();

        let operation = get_timelock_operation(&env, operation_id).unwrap();
        assert_eq!(operation.status, TimelockStatus::Executed);
    }

    #[test]
    fn test_cancel_timelock_operation() {
        let env = Env::default();
        env.mock_all_auths();

        let config = TimelockConfig::default();
        initialize_timelock(&env, config).unwrap();

        let proposer = Address::generate(&env);
        let proposal_type = ProposalType::MinCollateralRatio(8000);
        let description = String::from_str(&env, "Test proposal");

        let operation_id =
            queue_timelock_operation(&env, proposer.clone(), proposal_type, description, None)
                .unwrap();

        cancel_timelock_operation(&env, proposer, operation_id).unwrap();

        let operation = get_timelock_operation(&env, operation_id).unwrap();
        assert_eq!(operation.status, TimelockStatus::Cancelled);
    }
}

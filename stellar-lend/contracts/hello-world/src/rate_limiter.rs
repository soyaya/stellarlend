//! # Rate Limiter Module
//!
//! Provides per-user and global-per-pool rate limiting for sensitive operations.
//! Implemented as a token bucket with integer fixed-point math (1e6 scale).

#![allow(unused)]

use soroban_sdk::{contracterror, contracttype, Address, Env, Symbol, Vec};

use crate::admin;

const TOKEN_SCALE: i128 = 1_000_000; // fixed-point scale

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RateLimitError {
    /// Caller exceeded their rate limit.
    RateLimited = 1,
    /// Invalid configuration parameters.
    InvalidConfig = 2,
    /// Unauthorized configuration call.
    Unauthorized = 3,
    /// Arithmetic overflow.
    Overflow = 4,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct RateLimitConfig {
    /// Time window (seconds) used to derive refill rate.
    pub window_seconds: u64,
    /// Allowed calls per window (steady-state rate).
    pub max_calls_per_window: u32,
    /// Additional burst capacity on top of max_calls_per_window.
    pub burst_calls: u32,
    /// Extra burst calls granted to whitelisted/high-frequency users.
    pub grace_burst_calls: u32,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct BucketState {
    /// Current tokens (scaled by TOKEN_SCALE).
    pub tokens: i128,
    /// Last refill timestamp (ledger timestamp seconds).
    pub last_refill: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct RateLimitStatus {
    pub config: RateLimitConfig,
    pub bucket: BucketState,
    /// Capacity in tokens (scaled).
    pub capacity_tokens: i128,
    /// Refill rate per second (scaled tokens/sec).
    pub refill_per_second: i128,
    /// Whether this address is considered grace-enabled for this op.
    pub grace_enabled: bool,
}

#[contracttype]
#[derive(Clone)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub enum RateLimitDataKey {
    /// Default config for an operation
    OpConfig(Symbol),
    /// Config override for an operation + pool
    OpPoolConfig(Symbol, Address),
    /// Per-user bucket for (user, operation, pool)
    UserBucket(Address, Symbol, Address),
    /// Global bucket for (operation, pool)
    GlobalBucket(Symbol, Address),
    /// Optional per-user grace enable flag for an operation
    UserGrace(Address, Symbol),
}

fn default_config(env: &Env, op: &Symbol) -> RateLimitConfig {
    // Conservative defaults: set low but non-zero limits to make abuse harder,
    // while allowing typical UX. Admin can tune per operation/pool.
    // Borrow and liquidate are the primary targets.
    let name = op.to_string();
    if name == "borrow" {
        RateLimitConfig {
            window_seconds: 60,
            max_calls_per_window: 5,
            burst_calls: 3,
            grace_burst_calls: 10,
        }
    } else if name == "liquidate" {
        RateLimitConfig {
            window_seconds: 60,
            max_calls_per_window: 10,
            burst_calls: 5,
            grace_burst_calls: 20,
        }
    } else {
        RateLimitConfig {
            window_seconds: 60,
            max_calls_per_window: 30,
            burst_calls: 10,
            grace_burst_calls: 0,
        }
    }
}

fn get_op_config(env: &Env, op: &Symbol) -> RateLimitConfig {
    let key = RateLimitDataKey::OpConfig(op.clone());
    env.storage()
        .persistent()
        .get::<RateLimitDataKey, RateLimitConfig>(&key)
        .unwrap_or_else(|| default_config(env, op))
}

fn get_pool_config(env: &Env, op: &Symbol, pool: &Address) -> RateLimitConfig {
    let key = RateLimitDataKey::OpPoolConfig(op.clone(), pool.clone());
    env.storage()
        .persistent()
        .get::<RateLimitDataKey, RateLimitConfig>(&key)
        .unwrap_or_else(|| get_op_config(env, op))
}

fn is_grace_enabled(env: &Env, user: &Address, op: &Symbol) -> bool {
    let key = RateLimitDataKey::UserGrace(user.clone(), op.clone());
    env.storage()
        .persistent()
        .get::<RateLimitDataKey, bool>(&key)
        .unwrap_or(false)
}

fn capacity_tokens(cfg: &RateLimitConfig, grace: bool) -> i128 {
    // Capacity is derived from config; validate_config ensures non-zero window and max calls.
    let base = (cfg.max_calls_per_window as i128)
        .checked_add(cfg.burst_calls as i128)
        .unwrap_or(i128::MAX);
    let extra = if grace { cfg.grace_burst_calls as i128 } else { 0 };
    base.checked_add(extra)
        .and_then(|v| v.checked_mul(TOKEN_SCALE))
        .unwrap_or(i128::MAX)
}

fn refill_per_second(cfg: &RateLimitConfig) -> Result<i128, RateLimitError> {
    if cfg.window_seconds == 0 || cfg.max_calls_per_window == 0 {
        return Err(RateLimitError::InvalidConfig);
    }
    let per_window_tokens = (cfg.max_calls_per_window as i128)
        .checked_mul(TOKEN_SCALE)
        .ok_or(RateLimitError::Overflow)?;
    Ok(per_window_tokens
        .checked_div(cfg.window_seconds as i128)
        .ok_or(RateLimitError::Overflow)?)
}

fn refill_bucket(
    env: &Env,
    mut bucket: BucketState,
    cfg: &RateLimitConfig,
    cap_tokens: i128,
) -> Result<BucketState, RateLimitError> {
    let now = env.ledger().timestamp();
    if now <= bucket.last_refill {
        return Ok(bucket);
    }
    let dt = now - bucket.last_refill;
    let rate = refill_per_second(cfg)?;
    let add = rate
        .checked_mul(dt as i128)
        .ok_or(RateLimitError::Overflow)?;
    bucket.tokens = bucket
        .tokens
        .checked_add(add)
        .ok_or(RateLimitError::Overflow)?
        .min(cap_tokens);
    bucket.last_refill = now;
    Ok(bucket)
}

fn get_or_init_bucket(
    env: &Env,
    key: &RateLimitDataKey,
    cap_tokens: i128,
) -> BucketState {
    env.storage()
        .persistent()
        .get::<RateLimitDataKey, BucketState>(key)
        .unwrap_or(BucketState {
            tokens: cap_tokens,
            last_refill: env.ledger().timestamp(),
        })
}

fn set_bucket(env: &Env, key: &RateLimitDataKey, bucket: &BucketState) {
    env.storage().persistent().set(key, bucket);
}

fn is_bypassed(env: &Env, caller: &Address) -> bool {
    // Governance/admin bypass: admin can always act (e.g., emergency actions),
    // and a dedicated role can be granted for bots/keepers.
    if admin::get_admin(env).map(|a| a == *caller).unwrap_or(false) {
        return true;
    }
    admin::has_role(env, Symbol::new(env, "rate_limit_bypass"), caller.clone())
}

/// Configure default rate limit parameters for an operation (admin-only).
pub fn configure_operation_limit(
    env: &Env,
    caller: Address,
    op: Symbol,
    cfg: RateLimitConfig,
) -> Result<(), RateLimitError> {
    admin::require_admin(env, &caller).map_err(|_| RateLimitError::Unauthorized)?;
    validate_config(&cfg)?;
    let key = RateLimitDataKey::OpConfig(op);
    env.storage().persistent().set(&key, &cfg);
    Ok(())
}

/// Configure per-pool global rate limit parameters for an operation (admin-only).
pub fn configure_pool_limit(
    env: &Env,
    caller: Address,
    op: Symbol,
    pool: Address,
    cfg: RateLimitConfig,
) -> Result<(), RateLimitError> {
    admin::require_admin(env, &caller).map_err(|_| RateLimitError::Unauthorized)?;
    validate_config(&cfg)?;
    let key = RateLimitDataKey::OpPoolConfig(op, pool);
    env.storage().persistent().set(&key, &cfg);
    Ok(())
}

/// Enable/disable grace for a (user, operation) pair (admin-only).
pub fn set_user_grace(
    env: &Env,
    caller: Address,
    user: Address,
    op: Symbol,
    enabled: bool,
) -> Result<(), RateLimitError> {
    admin::require_admin(env, &caller).map_err(|_| RateLimitError::Unauthorized)?;
    let key = RateLimitDataKey::UserGrace(user, op);
    env.storage().persistent().set(&key, &enabled);
    Ok(())
}

fn validate_config(cfg: &RateLimitConfig) -> Result<(), RateLimitError> {
    if cfg.window_seconds == 0 {
        return Err(RateLimitError::InvalidConfig);
    }
    if cfg.max_calls_per_window == 0 {
        return Err(RateLimitError::InvalidConfig);
    }
    Ok(())
}

/// Consume one unit from the per-user and global-per-pool limit buckets.
///
/// This should be called at the beginning of sensitive entrypoints.
pub fn consume(
    env: &Env,
    caller: &Address,
    user: &Address,
    op: &Symbol,
    pool: &Address,
) -> Result<(), RateLimitError> {
    if is_bypassed(env, caller) {
        return Ok(());
    }

    let cfg = get_pool_config(env, op, pool);
    let grace = is_grace_enabled(env, user, op);
    let cap = capacity_tokens(&cfg, grace);

    // Per-user bucket
    let user_key = RateLimitDataKey::UserBucket(user.clone(), op.clone(), pool.clone());
    let user_bucket = get_or_init_bucket(env, &user_key, cap);
    let mut user_bucket = refill_bucket(env, user_bucket, &cfg, cap)?;
    if user_bucket.tokens < TOKEN_SCALE {
        return Err(RateLimitError::RateLimited);
    }
    user_bucket.tokens = user_bucket
        .tokens
        .checked_sub(TOKEN_SCALE)
        .ok_or(RateLimitError::Overflow)?;
    set_bucket(env, &user_key, &user_bucket);

    // Global bucket (per pool)
    let global_key = RateLimitDataKey::GlobalBucket(op.clone(), pool.clone());
    let global_bucket = get_or_init_bucket(env, &global_key, cap);
    let mut global_bucket = refill_bucket(env, global_bucket, &cfg, cap)?;
    if global_bucket.tokens < TOKEN_SCALE {
        return Err(RateLimitError::RateLimited);
    }
    global_bucket.tokens = global_bucket
        .tokens
        .checked_sub(TOKEN_SCALE)
        .ok_or(RateLimitError::Overflow)?;
    set_bucket(env, &global_key, &global_bucket);

    Ok(())
}

/// Read-only: return the current effective status for a per-user bucket.
pub fn get_user_status(env: &Env, user: Address, op: Symbol, pool: Address) -> RateLimitStatus {
    let cfg = get_pool_config(env, &op, &pool);
    let grace = is_grace_enabled(env, &user, &op);
    let cap = capacity_tokens(&cfg, grace);
    let key = RateLimitDataKey::UserBucket(user.clone(), op.clone(), pool.clone());
    let bucket = get_or_init_bucket(env, &key, cap);
    let refill = refill_per_second(&cfg).unwrap_or(0);
    RateLimitStatus {
        config: cfg,
        bucket,
        capacity_tokens: cap,
        refill_per_second: refill,
        grace_enabled: grace,
    }
}

/// Read-only: return the current effective status for the global-per-pool bucket.
pub fn get_global_status(env: &Env, op: Symbol, pool: Address) -> RateLimitStatus {
    let cfg = get_pool_config(env, &op, &pool);
    let cap = capacity_tokens(&cfg, false);
    let key = RateLimitDataKey::GlobalBucket(op.clone(), pool.clone());
    let bucket = get_or_init_bucket(env, &key, cap);
    let refill = refill_per_second(&cfg).unwrap_or(0);
    RateLimitStatus {
        config: cfg,
        bucket,
        capacity_tokens: cap,
        refill_per_second: refill,
        grace_enabled: false,
    }
}


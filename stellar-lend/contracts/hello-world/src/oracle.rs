//! # Oracle Module
//!
//! Manages price feeds for all protocol assets with staleness checks, deviation
//! guards, caching, multi-source aggregation, circuit breakers, TWAP, and
//! historical tracking.
//!
//! ## Price Resolution Order
//! 1. **Cache**: returns a cached price if the TTL has not expired.
//! 2. **Primary feed**: reads the on-chain `PriceFeed` entry; rejects if stale.
//! 3. **Fallback oracle**: if the primary is stale or missing, queries a
//!    configured fallback oracle address.
//!
//! ## Safety
//! - Price deviation between consecutive updates is bounded (default ±5%).
//! - Staleness threshold defaults to 1 hour; configurable by admin.
//! - Sanity-check bounds on min/max price are enforced on every update.
//! - Only the admin or the designated oracle address may submit price updates.
//! - Multiple sources can be configured per asset; aggregation uses a median
//!   and removes outliers beyond a configured deviation band.
//! - A per-asset circuit breaker can halt pricing when deviations are extreme.
//! - TWAP is computed over a configurable time window from stored observations.

#![allow(unused)]
use crate::admin::get_admin;
use crate::deposit::DepositDataKey;
use crate::events::{emit_price_updated, PriceUpdatedEvent};
use soroban_sdk::{contracterror, contracttype, Address, Env, IntoVal, Map, Symbol, Val, Vec};

/// Errors that can occur during oracle operations
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum OracleError {
    /// Invalid price (zero or negative)
    InvalidPrice = 1,
    /// Price is too stale (older than threshold)
    StalePrice = 2,
    /// Price deviation exceeds maximum allowed
    PriceDeviationExceeded = 3,
    /// Oracle address is invalid
    InvalidOracle = 4,
    /// Oracle update is paused
    OraclePaused = 5,
    /// Overflow occurred during calculation
    Overflow = 6,
    /// Unauthorized access
    Unauthorized = 7,
    /// Asset not supported
    AssetNotSupported = 8,
    /// Fallback oracle not configured
    FallbackNotConfigured = 9,
    /// Circuit breaker is open for this asset
    CircuitBreakerOpen = 10,
    /// Not enough valid sources to produce a safe price
    NotEnoughSources = 11,
}

/// Storage keys for oracle-related data
#[contracttype]
#[derive(Clone)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub enum OracleDataKey {
    /// Latest price feed data for a specific asset
    /// Value type: PriceFeed
    PriceFeed(Address),
    /// Address of the designated fallback oracle for an asset
    /// Value type: Address
    FallbackOracle(Address),
    /// Primary oracle address for an asset
    /// Value type: Address
    PrimaryOracle(Address),
    /// Fallback price feed for an asset
    /// Value type: PriceFeed
    FallbackFeed(Address),
    /// Transient price cache for improved gas efficiency
    /// Value type: CachedPrice
    PriceCache(Address),
    /// Global oracle safety and operational parameters
    OracleConfig,
    /// Pause switches specifically for oracle updates: Map<Symbol, bool>
    PauseSwitches,
    /// Configured additional oracle sources for an asset (excluding primary/fallback)
    /// Value type: Vec<Address>
    OracleSources(Address),
    /// Latest feed per (asset, source oracle)
    /// Value type: PriceFeed
    SourceFeed(Address, Address),
    /// Rolling price observations for TWAP per asset
    /// Value type: Vec<PriceObservation>
    PriceHistory(Address),
    /// Circuit breaker state per asset
    /// Value type: CircuitBreakerState
    CircuitBreaker(Address),
}

/// Price feed data structure
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct PriceFeed {
    /// Current price (in smallest unit, e.g., cents for USD)
    pub price: i128,
    /// Timestamp when price was last updated
    pub last_updated: u64,
    /// Oracle address that provided this price
    pub oracle: Address,
    /// Price decimals (e.g., 8 for BTC, 2 for USD)
    pub decimals: u32,
}

/// Cached price data
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct CachedPrice {
    /// Cached price
    pub price: i128,
    /// Timestamp when price was cached
    pub cached_at: u64,
    /// Cache TTL in seconds
    pub ttl: u64,
}

/// Oracle configuration
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct OracleConfig {
    /// Maximum price deviation in basis points (e.g., 500 = 5%)
    pub max_deviation_bps: i128,
    /// Maximum staleness in seconds
    pub max_staleness_seconds: u64,
    /// Cache TTL in seconds
    pub cache_ttl_seconds: u64,
    /// Minimum price sanity check
    pub min_price: i128,
    /// Maximum price sanity check
    pub max_price: i128,
    /// TWAP window in seconds (0 = disabled; use spot aggregation)
    pub twap_window_seconds: u64,
    /// Max number of observations stored per asset for TWAP/history
    pub max_observations: u32,
    /// Minimum number of sources required after outlier filtering
    pub min_sources: u32,
    /// Outlier filter band around median, in basis points (e.g. 1000 = 10%)
    pub outlier_deviation_bps: i128,
    /// Circuit breaker deviation vs last accepted TWAP/spot, in bps
    pub breaker_deviation_bps: i128,
    /// Circuit breaker cooldown in seconds
    pub breaker_cooldown_seconds: u64,
}

/// Default configuration values
const DEFAULT_MAX_DEVIATION_BPS: i128 = 500; // 5%
const DEFAULT_MAX_STALENESS_SECONDS: u64 = 3600; // 1 hour
const DEFAULT_CACHE_TTL_SECONDS: u64 = 300; // 5 minutes
const DEFAULT_MIN_PRICE: i128 = 1;
const DEFAULT_MAX_PRICE: i128 = i128::MAX;
const DEFAULT_TWAP_WINDOW_SECONDS: u64 = 900; // 15 minutes
const DEFAULT_MAX_OBSERVATIONS: u32 = 64;
const DEFAULT_MIN_SOURCES: u32 = 1;
const DEFAULT_OUTLIER_DEVIATION_BPS: i128 = 1000; // 10%
const DEFAULT_BREAKER_DEVIATION_BPS: i128 = 2500; // 25%
const DEFAULT_BREAKER_COOLDOWN_SECONDS: u64 = 600; // 10 minutes

/// Get default oracle configuration
fn get_default_config() -> OracleConfig {
    OracleConfig {
        max_deviation_bps: DEFAULT_MAX_DEVIATION_BPS,
        max_staleness_seconds: DEFAULT_MAX_STALENESS_SECONDS,
        cache_ttl_seconds: DEFAULT_CACHE_TTL_SECONDS,
        min_price: DEFAULT_MIN_PRICE,
        max_price: DEFAULT_MAX_PRICE,
        twap_window_seconds: DEFAULT_TWAP_WINDOW_SECONDS,
        max_observations: DEFAULT_MAX_OBSERVATIONS,
        min_sources: DEFAULT_MIN_SOURCES,
        outlier_deviation_bps: DEFAULT_OUTLIER_DEVIATION_BPS,
        breaker_deviation_bps: DEFAULT_BREAKER_DEVIATION_BPS,
        breaker_cooldown_seconds: DEFAULT_BREAKER_COOLDOWN_SECONDS,
    }
}

/// Get oracle configuration
fn get_oracle_config(env: &Env) -> OracleConfig {
    let config_key = OracleDataKey::OracleConfig;
    env.storage()
        .persistent()
        .get::<OracleDataKey, OracleConfig>(&config_key)
        .unwrap_or_else(get_default_config)
}

/// Get primary oracle for an asset
fn get_primary_oracle(env: &Env, asset: &Address) -> Option<Address> {
    let key = OracleDataKey::PrimaryOracle(asset.clone());
    env.storage()
        .persistent()
        .get::<OracleDataKey, Address>(&key)
}

/// Get fallback oracle for an asset
fn get_fallback_oracle(env: &Env, asset: &Address) -> Option<Address> {
    let key = OracleDataKey::FallbackOracle(asset.clone());
    env.storage()
        .persistent()
        .get::<OracleDataKey, Address>(&key)
}

/// Validate price against sanity checks
fn validate_price(env: &Env, price: i128) -> Result<(), OracleError> {
    if price <= 0 {
        return Err(OracleError::InvalidPrice);
    }

    let config = get_oracle_config(env);
    if price < config.min_price || price > config.max_price {
        return Err(OracleError::InvalidPrice);
    }

    Ok(())
}

/// Check if price is stale
fn is_price_stale(env: &Env, last_updated: u64) -> bool {
    let config = get_oracle_config(env);
    let current_time = env.ledger().timestamp();

    if current_time < last_updated {
        return true; // Invalid timestamp
    }

    let age = current_time - last_updated;
    age > config.max_staleness_seconds
}

/// Check price deviation between two prices
fn check_price_deviation(env: &Env, new_price: i128, old_price: i128) -> Result<(), OracleError> {
    if old_price == 0 {
        return Ok(()); // No previous price to compare
    }

    let config = get_oracle_config(env);

    // Calculate deviation: |new - old| / old * 10000 (basis points)
    let diff = if new_price > old_price {
        new_price
            .checked_sub(old_price)
            .ok_or(OracleError::Overflow)?
    } else {
        old_price
            .checked_sub(new_price)
            .ok_or(OracleError::Overflow)?
    };

    let deviation_bps = diff
        .checked_mul(10000)
        .ok_or(OracleError::Overflow)?
        .checked_div(old_price)
        .ok_or(OracleError::Overflow)?;

    if deviation_bps > config.max_deviation_bps {
        return Err(OracleError::PriceDeviationExceeded);
    }

    Ok(())
}

/// A single observation used for TWAP and analysis.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct PriceObservation {
    pub price: i128,
    pub timestamp: u64,
}

/// Per-asset circuit breaker state.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct CircuitBreakerState {
    /// If `open_until` is in the future, pricing is halted for this asset.
    pub open_until: u64,
    /// Last accepted safe price (spot or TWAP) used for breaker comparisons.
    pub last_safe_price: i128,
    /// Last time we tripped (for metrics/analysis).
    pub last_trip_timestamp: u64,
}

fn get_breaker_state(env: &Env, asset: &Address) -> CircuitBreakerState {
    let key = OracleDataKey::CircuitBreaker(asset.clone());
    env.storage()
        .persistent()
        .get::<OracleDataKey, CircuitBreakerState>(&key)
        .unwrap_or(CircuitBreakerState {
            open_until: 0,
            last_safe_price: 0,
            last_trip_timestamp: 0,
        })
}

fn set_breaker_state(env: &Env, asset: &Address, state: &CircuitBreakerState) {
    let key = OracleDataKey::CircuitBreaker(asset.clone());
    env.storage().persistent().set(&key, state);
}

fn is_breaker_open(env: &Env, asset: &Address) -> bool {
    let state = get_breaker_state(env, asset);
    env.ledger().timestamp() < state.open_until
}

fn maybe_trip_breaker(env: &Env, asset: &Address, candidate_price: i128) -> Result<(), OracleError> {
    let config = get_oracle_config(env);
    if config.breaker_deviation_bps <= 0 {
        return Ok(());
    }

    let mut state = get_breaker_state(env, asset);
    if state.last_safe_price <= 0 {
        // First price becomes baseline.
        state.last_safe_price = candidate_price;
        state.open_until = 0;
        state.last_trip_timestamp = 0;
        set_breaker_state(env, asset, &state);
        return Ok(());
    }

    // deviation_bps = |candidate - last_safe| / last_safe * 10000
    let diff = if candidate_price > state.last_safe_price {
        candidate_price
            .checked_sub(state.last_safe_price)
            .ok_or(OracleError::Overflow)?
    } else {
        state
            .last_safe_price
            .checked_sub(candidate_price)
            .ok_or(OracleError::Overflow)?
    };

    let deviation_bps = diff
        .checked_mul(10000)
        .ok_or(OracleError::Overflow)?
        .checked_div(state.last_safe_price)
        .ok_or(OracleError::Overflow)?;

    if deviation_bps > config.breaker_deviation_bps {
        let now = env.ledger().timestamp();
        state.open_until = now.saturating_add(config.breaker_cooldown_seconds);
        state.last_trip_timestamp = now;
        set_breaker_state(env, asset, &state);
        return Err(OracleError::CircuitBreakerOpen);
    }

    Ok(())
}

fn record_safe_price(env: &Env, asset: &Address, safe_price: i128) {
    let mut state = get_breaker_state(env, asset);
    state.last_safe_price = safe_price;
    set_breaker_state(env, asset, &state);
}

fn get_oracle_sources(env: &Env, asset: &Address) -> Vec<Address> {
    let key = OracleDataKey::OracleSources(asset.clone());
    env.storage()
        .persistent()
        .get::<OracleDataKey, Vec<Address>>(&key)
        .unwrap_or_else(|| Vec::new(env))
}

fn set_oracle_sources_internal(env: &Env, asset: &Address, sources: &Vec<Address>) {
    let key = OracleDataKey::OracleSources(asset.clone());
    env.storage().persistent().set(&key, sources);
}

fn get_source_feed(env: &Env, asset: &Address, source: &Address) -> Option<PriceFeed> {
    let key = OracleDataKey::SourceFeed(asset.clone(), source.clone());
    env.storage().persistent().get::<OracleDataKey, PriceFeed>(&key)
}

fn write_source_feed(env: &Env, asset: &Address, source: &Address, feed: &PriceFeed) {
    let key = OracleDataKey::SourceFeed(asset.clone(), source.clone());
    env.storage().persistent().set(&key, feed);
}

fn load_history(env: &Env, asset: &Address) -> Vec<PriceObservation> {
    let key = OracleDataKey::PriceHistory(asset.clone());
    env.storage()
        .persistent()
        .get::<OracleDataKey, Vec<PriceObservation>>(&key)
        .unwrap_or_else(|| Vec::new(env))
}

fn save_history(env: &Env, asset: &Address, history: &Vec<PriceObservation>) {
    let key = OracleDataKey::PriceHistory(asset.clone());
    env.storage().persistent().set(&key, history);
}

fn append_observation(env: &Env, asset: &Address, price: i128) {
    let config = get_oracle_config(env);
    if config.max_observations == 0 {
        return;
    }

    let mut history = load_history(env, asset);
    let now = env.ledger().timestamp();
    history.push_back(PriceObservation { price, timestamp: now });

    // Trim to max_observations (drop oldest).
    while history.len() > config.max_observations {
        history.pop_front();
    }
    save_history(env, asset, &history);
}

fn median_i128(env: &Env, mut values: Vec<i128>) -> Result<i128, OracleError> {
    let n = values.len();
    if n == 0 {
        return Err(OracleError::NotEnoughSources);
    }

    // Simple insertion sort (small n expected).
    let mut i = 1;
    while i < n {
        let key = values.get(i).unwrap();
        let mut j = i;
        while j > 0 {
            let prev = values.get(j - 1).unwrap();
            if prev <= key {
                break;
            }
            values.set(j, prev);
            j -= 1;
        }
        values.set(j, key);
        i += 1;
    }

    let mid = n / 2;
    Ok(values.get(mid).unwrap())
}

fn filter_outliers(env: &Env, median: i128, prices: Vec<i128>) -> Result<Vec<i128>, OracleError> {
    let config = get_oracle_config(env);
    if median <= 0 {
        return Err(OracleError::InvalidPrice);
    }
    if config.outlier_deviation_bps <= 0 {
        return Ok(prices);
    }

    let mut kept: Vec<i128> = Vec::new(env);
    for p in prices.iter() {
        if p <= 0 {
            continue;
        }
        let diff = if p > median {
            p.checked_sub(median).ok_or(OracleError::Overflow)?
        } else {
            median.checked_sub(p).ok_or(OracleError::Overflow)?
        };
        let deviation_bps = diff
            .checked_mul(10000)
            .ok_or(OracleError::Overflow)?
            .checked_div(median)
            .ok_or(OracleError::Overflow)?;
        if deviation_bps <= config.outlier_deviation_bps {
            kept.push_back(p);
        }
    }
    Ok(kept)
}

fn aggregate_spot_price(env: &Env, asset: &Address) -> Result<i128, OracleError> {
    let config = get_oracle_config(env);

    let mut candidates: Vec<i128> = Vec::new(env);
    let mut saw_any_feed: bool = false;
    let mut saw_stale_feed: bool = false;

    // Primary feed (if present and fresh)
    let primary_feed_key = OracleDataKey::PriceFeed(asset.clone());
    if let Some(feed) = env
        .storage()
        .persistent()
        .get::<OracleDataKey, PriceFeed>(&primary_feed_key)
    {
        saw_any_feed = true;
        if !is_price_stale(env, feed.last_updated) {
            candidates.push_back(feed.price);
        } else {
            saw_stale_feed = true;
        }
    }

    // Fallback feed (if present and fresh and from configured fallback oracle)
    if let Some(fallback_oracle) = get_fallback_oracle(env, asset) {
        let fallback_key = OracleDataKey::FallbackFeed(asset.clone());
        if let Some(feed) = env
            .storage()
            .persistent()
            .get::<OracleDataKey, PriceFeed>(&fallback_key)
        {
            saw_any_feed = true;
            if feed.oracle == fallback_oracle && !is_price_stale(env, feed.last_updated) {
                candidates.push_back(feed.price);
            } else if is_price_stale(env, feed.last_updated) {
                saw_stale_feed = true;
            }
        }
    }

    // Additional source feeds (configured by admin)
    let sources = get_oracle_sources(env, asset);
    for src in sources.iter() {
        if let Some(feed) = get_source_feed(env, asset, &src) {
            saw_any_feed = true;
            if !is_price_stale(env, feed.last_updated) {
                candidates.push_back(feed.price);
            } else {
                saw_stale_feed = true;
            }
        }
    }

    if candidates.len() == 0 {
        // Preserve historical behavior: if we saw feeds but they were stale, return StalePrice.
        if saw_any_feed && saw_stale_feed {
            return Err(OracleError::StalePrice);
        }
        // No feeds at all (or only mismatched fallback oracle); treat as missing.
        return Err(OracleError::FallbackNotConfigured);
    }

    let med = median_i128(env, candidates.clone())?;
    let filtered = filter_outliers(env, med, candidates)?;
    if filtered.len() < config.min_sources {
        return Err(OracleError::NotEnoughSources);
    }

    // Median again after filtering (robust to single extreme outlier).
    let med2 = median_i128(env, filtered)?;
    Ok(med2)
}

fn compute_twap(env: &Env, asset: &Address, spot_price: i128) -> Result<i128, OracleError> {
    let config = get_oracle_config(env);
    if config.twap_window_seconds == 0 {
        return Ok(spot_price);
    }

    let now = env.ledger().timestamp();
    let window_start = now.saturating_sub(config.twap_window_seconds);

    let history = load_history(env, asset);
    if history.len() == 0 {
        return Ok(spot_price);
    }

    // Time-weighted average over [window_start, now] using stored observations.
    // For the last segment, we assume the latest known price holds to `now`.
    let mut weighted_sum: i128 = 0;
    let mut total_time: u64 = 0;

    // Find the first observation within window; include the immediately prior one
    // (so TWAP includes continuity from before the window).
    let mut start_idx: usize = 0;
    let mut i: usize = 0;
    while i < history.len() {
        let obs = history.get(i).unwrap();
        if obs.timestamp >= window_start {
            start_idx = if i == 0 { 0 } else { i - 1 };
            break;
        }
        i += 1;
    }

    let mut prev = history.get(start_idx).unwrap();
    let mut prev_t = if prev.timestamp < window_start {
        window_start
    } else {
        prev.timestamp
    };

    let mut idx = start_idx + 1;
    while idx < history.len() {
        let cur = history.get(idx).unwrap();
        if cur.timestamp <= window_start {
            idx += 1;
            continue;
        }
        let cur_t = cur.timestamp;
        if cur_t > now {
            break;
        }
        if cur_t > prev_t {
            let dt = cur_t - prev_t;
            let dt_i128: i128 = dt as i128;
            weighted_sum = weighted_sum
                .checked_add(prev.price.checked_mul(dt_i128).ok_or(OracleError::Overflow)?)
                .ok_or(OracleError::Overflow)?;
            total_time = total_time.saturating_add(dt);
        }
        prev = cur;
        prev_t = cur_t;
        idx += 1;
    }

    // Last segment to now, using the latest observed price (or spot if none).
    let last_price = if prev.timestamp == 0 { spot_price } else { prev.price };
    if now > prev_t {
        let dt = now - prev_t;
        let dt_i128: i128 = dt as i128;
        weighted_sum = weighted_sum
            .checked_add(last_price.checked_mul(dt_i128).ok_or(OracleError::Overflow)?)
            .ok_or(OracleError::Overflow)?;
        total_time = total_time.saturating_add(dt);
    }

    if total_time == 0 {
        return Ok(spot_price);
    }

    Ok(weighted_sum
        .checked_div(total_time as i128)
        .ok_or(OracleError::Overflow)?)
}

/// Get cached price if valid
fn get_cached_price(env: &Env, asset: &Address) -> Option<i128> {
    let cache_key = OracleDataKey::PriceCache(asset.clone());
    if let Some(cached) = env
        .storage()
        .persistent()
        .get::<OracleDataKey, CachedPrice>(&cache_key)
    {
        let current_time = env.ledger().timestamp();
        if current_time >= cached.cached_at
            && current_time <= cached.cached_at.saturating_add(cached.ttl)
        {
            return Some(cached.price);
        }
    }
    None
}

/// Cache price
fn cache_price(env: &Env, asset: &Address, price: i128) {
    let config = get_oracle_config(env);
    let cache_key = OracleDataKey::PriceCache(asset.clone());
    let cached = CachedPrice {
        price,
        cached_at: env.ledger().timestamp(),
        ttl: config.cache_ttl_seconds,
    };
    env.storage().persistent().set(&cache_key, &cached);
}

/// Update price feed from oracle
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `caller` - The address calling this function (must be admin or oracle)
/// * `asset` - The asset address
/// * `price` - The new price
/// * `decimals` - Price decimals
/// * `oracle` - The oracle address providing this price
///
/// # Returns
/// Returns the updated price
pub fn update_price_feed(
    env: &Env,
    caller: Address,
    asset: Address,
    price: i128,
    decimals: u32,
    oracle: Address,
) -> Result<i128, OracleError> {
    // Check if oracle updates are paused
    let pause_key = OracleDataKey::PauseSwitches;
    if let Some(pause_map) = env
        .storage()
        .persistent()
        .get::<OracleDataKey, Map<Symbol, bool>>(&pause_key)
    {
        if let Some(paused) = pause_map.get(Symbol::new(env, "pause_oracle")) {
            if paused {
                return Err(OracleError::OraclePaused);
            }
        }
    }

    // Validate caller authorization
    let is_admin = get_admin(env).map(|admin| admin == caller).unwrap_or(false);
    let primary = get_primary_oracle(env, &asset);
    let fallback = get_fallback_oracle(env, &asset);

    let is_primary = primary.map(|p| p == caller).unwrap_or(false);
    let is_fallback = fallback.map(|f| f == caller).unwrap_or(false);

    if !is_admin && !is_primary && !is_fallback {
        return Err(OracleError::Unauthorized);
    }

    // Ensure oracle address matches caller if not admin
    if !is_admin && caller != oracle {
        return Err(OracleError::Unauthorized);
    }

    // Validate price
    validate_price(env, price)?;

    // Determine target storage key and get current feed for deviation check
    let feed_key = if is_fallback && !is_primary && !is_admin {
        OracleDataKey::FallbackFeed(asset.clone())
    } else {
        OracleDataKey::PriceFeed(asset.clone())
    };

    let current_feed = env
        .storage()
        .persistent()
        .get::<OracleDataKey, PriceFeed>(&feed_key);

    // Check price deviation if we have a previous price
    if let Some(ref feed) = current_feed {
        check_price_deviation(env, price, feed.price)?;
    }

    // Create new price feed
    let timestamp = env.ledger().timestamp();
    let oracle_clone = oracle.clone();
    let new_feed = PriceFeed {
        price,
        last_updated: timestamp,
        oracle: oracle_clone.clone(),
        decimals,
    };

    // Update storage
    env.storage().persistent().set(&feed_key, &new_feed);

    // Also store per-source feed (used for aggregation) when caller is not admin,
    // or when admin explicitly sets `oracle` for later authorization.
    // This lets the protocol aggregate across multiple configured sources.
    write_source_feed(env, &asset, &oracle, &new_feed);

    // When admin submits a price, register the oracle address as the primary oracle
    // for the asset so subsequent calls from that oracle are authorized.
    if is_admin {
        let primary_key = OracleDataKey::PrimaryOracle(asset.clone());
        env.storage().persistent().set(&primary_key, &oracle);
    }

    // Update cache
    cache_price(env, &asset, price);

    // Emit price update event
    emit_price_updated(
        env,
        PriceUpdatedEvent {
            actor: caller,
            asset: asset.clone(),
            price,
            decimals,
            oracle: oracle_clone,
            timestamp,
        },
    );

    Ok(price)
}

/// Get price for an asset with fallback support
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `asset` - The asset address
///
/// # Returns
/// Returns the current price, using cache or fallback if needed
pub fn get_price(env: &Env, asset: &Address) -> Result<i128, OracleError> {
    // Circuit breaker check (halts reads for this asset)
    if is_breaker_open(env, asset) {
        return Err(OracleError::CircuitBreakerOpen);
    }

    // Try cache first
    if let Some(cached_price) = get_cached_price(env, asset) {
        return Ok(cached_price);
    }

    // Aggregate spot from available sources, apply outlier removal.
    let spot = aggregate_spot_price(env, asset)?;

    // Circuit breaker trip check against last safe price.
    maybe_trip_breaker(env, asset, spot)?;

    // Store observation and compute TWAP (if enabled).
    append_observation(env, asset, spot);
    let twap = compute_twap(env, asset, spot)?;

    // Secondary circuit breaker check against TWAP output, to avoid returning
    // a fresh-but-manipulated TWAP when history is sparse.
    maybe_trip_breaker(env, asset, twap)?;

    // Cache and remember as last safe price.
    cache_price(env, asset, twap);
    record_safe_price(env, asset, twap);

    Ok(twap)
}

/// Get price from fallback oracle
fn get_fallback_price(env: &Env, asset: &Address) -> Result<i128, OracleError> {
    let fallback_key = OracleDataKey::FallbackOracle(asset.clone());
    if let Some(fallback_oracle) = env
        .storage()
        .persistent()
        .get::<OracleDataKey, Address>(&fallback_key)
    {
        // Get price from fallback oracle feed slot
        let feed_key = OracleDataKey::FallbackFeed(asset.clone());
        if let Some(feed) = env
            .storage()
            .persistent()
            .get::<OracleDataKey, PriceFeed>(&feed_key)
        {
            // Check if fallback price is valid and from authorized oracle
            if feed.oracle == fallback_oracle && !is_price_stale(env, feed.last_updated) {
                cache_price(env, asset, feed.price);
                return Ok(feed.price);
            }
        }
    }

    Err(OracleError::FallbackNotConfigured)
}

/// Set primary oracle for an asset
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `caller` - The address calling this function (must be admin)
/// * `asset` - The asset address
/// * `primary_oracle` - The primary oracle address
pub fn set_primary_oracle(
    env: &Env,
    caller: Address,
    asset: Address,
    primary_oracle: Address,
) -> Result<(), OracleError> {
    // Check authorization
    let admin = get_admin(env).ok_or(OracleError::Unauthorized)?;

    if caller != admin {
        return Err(OracleError::Unauthorized);
    }

    // Set primary oracle
    let primary_key = OracleDataKey::PrimaryOracle(asset);
    env.storage()
        .persistent()
        .set(&primary_key, &primary_oracle);

    Ok(())
}

/// Set fallback oracle for an asset
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `caller` - The address calling this function (must be admin)
/// * `asset` - The asset address
/// * `fallback_oracle` - The fallback oracle address
pub fn set_fallback_oracle(
    env: &Env,
    caller: Address,
    asset: Address,
    fallback_oracle: Address,
) -> Result<(), OracleError> {
    // Check authorization
    crate::admin::require_admin(env, &caller).map_err(|_| OracleError::Unauthorized)?;

    // Validate oracle address
    if fallback_oracle == env.current_contract_address() {
        return Err(OracleError::InvalidOracle);
    }

    // Set fallback oracle
    let fallback_key = OracleDataKey::FallbackOracle(asset);
    env.storage()
        .persistent()
        .set(&fallback_key, &fallback_oracle);

    Ok(())
}

/// Configure oracle parameters
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `caller` - The address calling this function (must be admin)
/// * `config` - The new oracle configuration
pub fn configure_oracle(
    env: &Env,
    caller: Address,
    config: OracleConfig,
) -> Result<(), OracleError> {
    // Check authorization
    crate::admin::require_admin(env, &caller).map_err(|_| OracleError::Unauthorized)?;

    // Validate configuration
    if config.max_deviation_bps <= 0 || config.max_deviation_bps > 10000 {
        return Err(OracleError::InvalidPrice);
    }

    if config.max_staleness_seconds == 0 {
        return Err(OracleError::InvalidPrice);
    }

    if config.min_sources == 0 {
        return Err(OracleError::InvalidPrice);
    }

    if config.max_observations > 256 {
        // Keep bounded storage.
        return Err(OracleError::InvalidPrice);
    }

    if config.outlier_deviation_bps <= 0 || config.outlier_deviation_bps > 10000 {
        return Err(OracleError::InvalidPrice);
    }

    if config.breaker_deviation_bps <= 0 || config.breaker_deviation_bps > 10000 {
        return Err(OracleError::InvalidPrice);
    }

    // Update configuration
    let config_key = OracleDataKey::OracleConfig;
    env.storage().persistent().set(&config_key, &config);

    Ok(())
}

/// Admin-only: set additional oracle sources for an asset.
///
/// These sources submit `update_price_feed` updates and are used for aggregation
/// alongside the primary (and fallback, if configured).
pub fn set_oracle_sources(
    env: &Env,
    caller: Address,
    asset: Address,
    sources: Vec<Address>,
) -> Result<(), OracleError> {
    crate::admin::require_admin(env, &caller).map_err(|_| OracleError::Unauthorized)?;
    set_oracle_sources_internal(env, &asset, &sources);
    Ok(())
}

/// Admin-only: emergency pause of oracle reads/writes for a specific asset.
///
/// Implemented by opening the circuit breaker for a long cooldown.
pub fn emergency_pause_asset_oracle(
    env: &Env,
    caller: Address,
    asset: Address,
    pause_seconds: u64,
) -> Result<(), OracleError> {
    crate::admin::require_admin(env, &caller).map_err(|_| OracleError::Unauthorized)?;
    let now = env.ledger().timestamp();
    let mut state = get_breaker_state(env, &asset);
    state.open_until = now.saturating_add(pause_seconds);
    state.last_trip_timestamp = now;
    set_breaker_state(env, &asset, &state);
    Ok(())
}

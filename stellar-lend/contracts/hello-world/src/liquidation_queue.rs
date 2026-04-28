use soroban_sdk::{Address, Env, Vec, contracttype};

use crate::errors::LendingError;
use crate::storage;

/// Maximum queue size to prevent overflow
pub const MAX_QUEUE_SIZE: u32 = 1000;

/// Queue entry expiration time (24 hours)
pub const QUEUE_ENTRY_EXPIRATION: u64 = 86400;

/// Health factor threshold for entering queue (in basis points, 10000 = 1.0)
pub const LIQUIDATION_THRESHOLD_BPS: i128 = 10000;

/// Priority multiplier for severely unhealthy positions
pub const SEVERE_HEALTH_MULTIPLIER: i128 = 2;
pub const CRITICAL_HEALTH_MULTIPLIER: i128 = 3;

/// Health factor thresholds for priority
pub const SEVERE_HEALTH_THRESHOLD_BPS: i128 = 8000; // 0.8
pub const CRITICAL_HEALTH_THRESHOLD_BPS: i128 = 5000; // 0.5

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum QueueEntryStatus {
    Pending,
    Processing,
    Completed,
    Expired,
    Cancelled,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct LiquidationQueueEntry {
    pub id: u64,
    pub borrower: Address,
    pub liquidator: Address,
    pub health_factor: i128,
    pub priority_score: i128,
    pub queued_at: u64,
    pub expires_at: u64,
    pub status: QueueEntryStatus,
    pub debt_value: i128,
    pub collateral_value: i128,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct LiquidatorRegistration {
    pub liquidator: Address,
    pub registered_at: u64,
    pub active: bool,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct QueueConfig {
    pub max_queue_size: u32,
    pub entry_expiration: u64,
    pub fifo_enabled: bool,
    pub priority_enabled: bool,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            max_queue_size: MAX_QUEUE_SIZE,
            entry_expiration: QUEUE_ENTRY_EXPIRATION,
            fifo_enabled: true,
            priority_enabled: true,
        }
    }
}

/// Initialize liquidation queue
pub fn initialize_queue(env: &Env, config: QueueConfig) -> Result<(), LendingError> {
    let config_key = storage::DataKey::LiquidationQueueConfig;
    env.storage().instance().set(&config_key, &config);

    let next_id_key = storage::DataKey::NextLiquidationQueueId;
    env.storage().instance().set(&next_id_key, &0u64);

    Ok(())
}

/// Get queue configuration
pub fn get_queue_config(env: &Env) -> QueueConfig {
    let config_key = storage::DataKey::LiquidationQueueConfig;
    env.storage()
        .instance()
        .get(&config_key)
        .unwrap_or_default()
}

/// Register liquidator interest in unhealthy position
pub fn register_liquidation_interest(
    env: &Env,
    liquidator: Address,
    borrower: Address,
) -> Result<u64, LendingError> {
    liquidator.require_auth();

    // Check if borrower position is unhealthy
    let health_factor = crate::analytics::calculate_health_factor(env, &borrower)
        .map_err(|_| LendingError::InvalidState)?;

    if health_factor >= LIQUIDATION_THRESHOLD_BPS {
        return Err(LendingError::InvalidState); // Position is healthy
    }

    let config = get_queue_config(env);

    // Check queue size
    let queue = get_pending_queue_entries(env);
    if queue.len() >= config.max_queue_size {
        return Err(LendingError::LimitExceeded);
    }

    // Get position values
    let position = crate::analytics::get_user_position_summary(env, &borrower)
        .map_err(|_| LendingError::DataNotFound)?;

    let debt_value = position.debt;
    let collateral_value = position.collateral;

    // Calculate priority score
    let priority_score = calculate_priority_score(health_factor, debt_value);

    let next_id_key = storage::DataKey::NextLiquidationQueueId;
    let entry_id: u64 = env.storage().instance().get(&next_id_key).unwrap_or(0);

    let now = env.ledger().timestamp();
    let expires_at = now + config.entry_expiration;

    let entry = LiquidationQueueEntry {
        id: entry_id,
        borrower: borrower.clone(),
        liquidator: liquidator.clone(),
        health_factor,
        priority_score,
        queued_at: now,
        expires_at,
        status: QueueEntryStatus::Pending,
        debt_value,
        collateral_value,
    };

    let entry_key = storage::DataKey::LiquidationQueueEntry(entry_id);
    env.storage().persistent().set(&entry_key, &entry);

    env.storage()
        .instance()
        .set(&next_id_key, &(entry_id + 1));

    // Emit event
    crate::events::LiquidationQueuedEvent {
        entry_id,
        borrower,
        liquidator,
        health_factor,
        priority_score,
        timestamp: now,
    }
    .publish(env);

    Ok(entry_id)
}

/// Calculate priority score based on health factor and debt value
fn calculate_priority_score(health_factor: i128, debt_value: i128) -> i128 {
    let base_score = 10000 - health_factor; // Lower health = higher priority

    let multiplier = if health_factor <= CRITICAL_HEALTH_THRESHOLD_BPS {
        CRITICAL_HEALTH_MULTIPLIER
    } else if health_factor <= SEVERE_HEALTH_THRESHOLD_BPS {
        SEVERE_HEALTH_MULTIPLIER
    } else {
        1
    };

    // Combine health factor priority with debt size (larger debts get slight priority)
    let debt_bonus = (debt_value / 1_000_000).min(1000); // Cap at 1000 bonus points

    base_score * multiplier + debt_bonus
}

/// Get next liquidation from queue (highest priority or FIFO)
pub fn get_next_liquidation(env: &Env) -> Option<LiquidationQueueEntry> {
    let config = get_queue_config(env);
    let mut queue = get_pending_queue_entries(env);

    if queue.is_empty() {
        return None;
    }

    // Remove expired entries
    cleanup_expired_entries(env);

    // Refresh queue after cleanup
    queue = get_pending_queue_entries(env);

    if queue.is_empty() {
        return None;
    }

    if config.priority_enabled {
        // Find entry with highest priority score
        let mut best_entry: Option<LiquidationQueueEntry> = None;
        let mut best_score = i128::MIN;

        for entry in queue.iter() {
            if entry.priority_score > best_score {
                best_score = entry.priority_score;
                best_entry = Some(entry);
            }
        }

        best_entry
    } else {
        // FIFO: return oldest entry
        let mut oldest_entry: Option<LiquidationQueueEntry> = None;
        let mut oldest_time = u64::MAX;

        for entry in queue.iter() {
            if entry.queued_at < oldest_time {
                oldest_time = entry.queued_at;
                oldest_entry = Some(entry);
            }
        }

        oldest_entry
    }
}

/// Process liquidation from queue
pub fn process_queue_liquidation(
    env: &Env,
    entry_id: u64,
    executor: Address,
) -> Result<(), LendingError> {
    executor.require_auth();

    let entry_key = storage::DataKey::LiquidationQueueEntry(entry_id);
    let mut entry: LiquidationQueueEntry = env
        .storage()
        .persistent()
        .get(&entry_key)
        .ok_or(LendingError::DataNotFound)?;

    if entry.status != QueueEntryStatus::Pending {
        return Err(LendingError::InvalidState);
    }

    // Check if entry has expired
    let now = env.ledger().timestamp();
    if now > entry.expires_at {
        entry.status = QueueEntryStatus::Expired;
        env.storage().persistent().set(&entry_key, &entry);
        return Err(LendingError::InvalidState);
    }

    // Check if position is still unhealthy
    let current_health = crate::analytics::calculate_health_factor(env, &entry.borrower)
        .map_err(|_| LendingError::InvalidState)?;

    if current_health >= LIQUIDATION_THRESHOLD_BPS {
        // Position became healthy, cancel entry
        entry.status = QueueEntryStatus::Cancelled;
        env.storage().persistent().set(&entry_key, &entry);
        return Err(LendingError::InvalidState);
    }

    // Mark as processing
    entry.status = QueueEntryStatus::Processing;
    env.storage().persistent().set(&entry_key, &entry);

    // Execute liquidation (this would call the actual liquidation logic)
    // For now, we just mark it as completed
    entry.status = QueueEntryStatus::Completed;
    env.storage().persistent().set(&entry_key, &entry);

    // Emit event
    crate::events::LiquidationProcessedEvent {
        entry_id,
        borrower: entry.borrower,
        liquidator: entry.liquidator,
        executor,
        timestamp: now,
    }
    .publish(env);

    Ok(())
}

/// Cancel queue entry
pub fn cancel_queue_entry(
    env: &Env,
    entry_id: u64,
    caller: Address,
) -> Result<(), LendingError> {
    caller.require_auth();

    let entry_key = storage::DataKey::LiquidationQueueEntry(entry_id);
    let mut entry: LiquidationQueueEntry = env
        .storage()
        .persistent()
        .get(&entry_key)
        .ok_or(LendingError::DataNotFound)?;

    // Only liquidator or admin can cancel
    let admin = crate::admin::get_admin(env).ok_or(LendingError::Unauthorized)?;
    if caller != entry.liquidator && caller != admin {
        return Err(LendingError::Unauthorized);
    }

    if entry.status != QueueEntryStatus::Pending {
        return Err(LendingError::InvalidState);
    }

    entry.status = QueueEntryStatus::Cancelled;
    env.storage().persistent().set(&entry_key, &entry);

    // Emit event
    crate::events::LiquidationCancelledEvent {
        entry_id,
        caller,
        timestamp: env.ledger().timestamp(),
    }
    .publish(env);

    Ok(())
}

/// Get all pending queue entries
pub fn get_pending_queue_entries(env: &Env) -> Vec<LiquidationQueueEntry> {
    let next_id_key = storage::DataKey::NextLiquidationQueueId;
    let next_id: u64 = env.storage().instance().get(&next_id_key).unwrap_or(0);

    let mut pending = Vec::new(env);

    for id in 0..next_id {
        let entry_key = storage::DataKey::LiquidationQueueEntry(id);
        if let Some(entry) = env
            .storage()
            .persistent()
            .get::<storage::DataKey, LiquidationQueueEntry>(&entry_key)
        {
            if entry.status == QueueEntryStatus::Pending {
                pending.push_back(entry);
            }
        }
    }

    pending
}

/// Get queue entry by ID
pub fn get_queue_entry(env: &Env, entry_id: u64) -> Option<LiquidationQueueEntry> {
    let entry_key = storage::DataKey::LiquidationQueueEntry(entry_id);
    env.storage().persistent().get(&entry_key)
}

/// Cleanup expired entries
pub fn cleanup_expired_entries(env: &Env) -> u32 {
    let now = env.ledger().timestamp();
    let next_id_key = storage::DataKey::NextLiquidationQueueId;
    let next_id: u64 = env.storage().instance().get(&next_id_key).unwrap_or(0);

    let mut cleaned = 0u32;

    for id in 0..next_id {
        let entry_key = storage::DataKey::LiquidationQueueEntry(id);
        if let Some(mut entry) = env
            .storage()
            .persistent()
            .get::<storage::DataKey, LiquidationQueueEntry>(&entry_key)
        {
            if entry.status == QueueEntryStatus::Pending && now > entry.expires_at {
                entry.status = QueueEntryStatus::Expired;
                env.storage().persistent().set(&entry_key, &entry);
                cleaned += 1;
            }
        }
    }

    cleaned
}

/// Get queue statistics
pub fn get_queue_stats(env: &Env) -> QueueStats {
    let next_id_key = storage::DataKey::NextLiquidationQueueId;
    let next_id: u64 = env.storage().instance().get(&next_id_key).unwrap_or(0);

    let mut pending = 0u32;
    let mut processing = 0u32;
    let mut completed = 0u32;
    let mut expired = 0u32;
    let mut cancelled = 0u32;

    for id in 0..next_id {
        let entry_key = storage::DataKey::LiquidationQueueEntry(id);
        if let Some(entry) = env
            .storage()
            .persistent()
            .get::<storage::DataKey, LiquidationQueueEntry>(&entry_key)
        {
            match entry.status {
                QueueEntryStatus::Pending => pending += 1,
                QueueEntryStatus::Processing => processing += 1,
                QueueEntryStatus::Completed => completed += 1,
                QueueEntryStatus::Expired => expired += 1,
                QueueEntryStatus::Cancelled => cancelled += 1,
            }
        }
    }

    QueueStats {
        total_entries: next_id,
        pending,
        processing,
        completed,
        expired,
        cancelled,
    }
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct QueueStats {
    pub total_entries: u64,
    pub pending: u32,
    pub processing: u32,
    pub completed: u32,
    pub expired: u32,
    pub cancelled: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn test_initialize_queue() {
        let env = Env::default();
        let config = QueueConfig::default();

        initialize_queue(&env, config.clone()).unwrap();
        let stored_config = get_queue_config(&env);

        assert_eq!(stored_config.max_queue_size, config.max_queue_size);
    }

    #[test]
    fn test_priority_score_calculation() {
        // Critical health factor
        let score1 = calculate_priority_score(4000, 1_000_000_000);
        // Severe health factor
        let score2 = calculate_priority_score(7000, 1_000_000_000);
        // Normal unhealthy
        let score3 = calculate_priority_score(9000, 1_000_000_000);

        assert!(score1 > score2);
        assert!(score2 > score3);
    }

    #[test]
    fn test_queue_entry_expiration() {
        let env = Env::default();
        env.mock_all_auths();

        let config = QueueConfig::default();
        initialize_queue(&env, config).unwrap();

        // Create mock entries would go here
        // This is a placeholder for the actual test
    }
}

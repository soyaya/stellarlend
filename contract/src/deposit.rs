pub use crate::events::VaultDepositEvent;
use crate::reentrancy_guard::{GuardKey, NonReentrant};  // ← ADD THIS

#[allow(dead_code)]
pub type DepositEvent = VaultDepositEvent;

use crate::pause::{self, PauseType};
use soroban_sdk::{contracterror, contracttype, Address, Env, token};

/// Errors that can occur during deposit operations
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum DepositError {
    InvalidAmount = 1,
    DepositPaused = 2,
    Overflow = 3,
    AssetNotSupported = 4,
    ExceedsDepositCap = 5,
    Unauthorized = 6,
    Reentrancy = 7,  // ← ADD THIS
}

/// Storage keys for deposit-related data
#[contracttype]
#[derive(Clone)]
#[allow(clippy::enum_variant_names)]
pub enum DepositDataKey {
    UserCollateral(Address),
    TotalAmount,
    CapAmount,
    MinAmount,
}

/// User deposit position
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct DepositCollateral {
    pub amount: i128,
    pub asset: Address,
    pub last_deposit_time: u64,
}

/// Deposit collateral into the protocol
///
/// # Security: CEI Pattern (Check-Effects-Interactions)
/// 1. CHECK: Validate all conditions first
/// 2. EFFECT: Update all state immediately
/// 3. INTERACTION: Only then make external calls
pub fn deposit(
    env: &Env,
    user: Address,
    asset: Address,
    amount: i128,
) -> Result<i128, DepositError> {
    // 🛡️ STEP 1: REENTRANCY GUARD - Prevent reentrancy
    let _guard = NonReentrant::new(env.clone(), GuardKey::DepositGuard)
        .map_err(|_| DepositError::Reentrancy)?;

    // ✓ STEP 2: CHECK - Verify all conditions FIRST
    user.require_auth();

    if pause::is_paused(env, PauseType::Deposit) {
        return Err(DepositError::DepositPaused);
    }

    if amount <= 0 {
        return Err(DepositError::InvalidAmount);
    }

    let min_deposit = get_min_deposit_amount(env);
    if amount < min_deposit {
        return Err(DepositError::InvalidAmount);
    }

    let total_deposits = get_total_deposits(env);
    let deposit_cap = get_deposit_cap(env);
    let new_total = total_deposits
        .checked_add(amount)
        .ok_or(DepositError::Overflow)?;

    if new_total > deposit_cap {
        return Err(DepositError::ExceedsDepositCap);
    }

    // ✓ STEP 3: EFFECT - Update state BEFORE external calls
    let mut position = get_deposit_position(env, &user, &asset);
    position.amount = position
        .amount
        .checked_add(amount)
        .ok_or(DepositError::Overflow)?;
    position.last_deposit_time = env.ledger().timestamp();
    position.asset = asset.clone();

    // Save state IMMEDIATELY (before any external calls)
    save_deposit_position(env, &user, &position);
    set_total_deposits(env, new_total);
    
    // Emit event before transfer
    emit_deposit_event(env, user.clone(), asset.clone(), amount, position.amount);

    // ✓ STEP 4: INTERACTION - Only now make external calls
    // Even if the token callback tries to re-enter, the guard will catch it
    let token_client = token::Client::new(env, &asset);
    token_client.transfer(&user, &env.current_contract_address(), &amount);

    // Guard automatically exits here (Drop trait)
    Ok(position.amount)
}

/// Initialize deposit settings
pub fn initialize_deposit_settings(
    env: &Env,
    deposit_cap: i128,
    min_deposit_amount: i128,
) -> Result<(), DepositError> {
    env.storage()
        .persistent()
        .set(&DepositDataKey::CapAmount, &deposit_cap);
    env.storage()
        .persistent()
        .set(&DepositDataKey::MinAmount, &min_deposit_amount);
    Ok(())
}

pub fn get_user_collateral(env: &Env, user: &Address, asset: &Address) -> DepositCollateral {
    get_deposit_position(env, user, asset)
}

fn get_deposit_position(env: &Env, user: &Address, asset: &Address) -> DepositCollateral {
    env.storage()
        .persistent()
        .get(&DepositDataKey::UserCollateral(user.clone()))
        .unwrap_or(DepositCollateral {
            amount: 0,
            asset: asset.clone(),
            last_deposit_time: env.ledger().timestamp(),
        })
}

fn save_deposit_position(env: &Env, user: &Address, position: &DepositCollateral) {
    env.storage()
        .persistent()
        .set(&DepositDataKey::UserCollateral(user.clone()), position);
}

fn get_total_deposits(env: &Env) -> i128 {
    env.storage()
        .persistent()
        .get(&DepositDataKey::TotalAmount)
        .unwrap_or(0)
}

fn set_total_deposits(env: &Env, amount: i128) {
    env.storage()
        .persistent()
        .set(&DepositDataKey::TotalAmount, &amount);
}

fn get_deposit_cap(env: &Env) -> i128 {
    env.storage()
        .persistent()
        .get(&DepositDataKey::CapAmount)
        .unwrap_or(i128::MAX)
}

fn get_min_deposit_amount(env: &Env) -> i128 {
    env.storage()
        .persistent()
        .get(&DepositDataKey::MinAmount)
        .unwrap_or(0)
}

fn emit_deposit_event(env: &Env, user: Address, asset: Address, amount: i128, new_balance: i128) {
    VaultDepositEvent {
        user,
        asset,
        amount,
        new_balance,
        timestamp: env.ledger().timestamp(),
    }
    .publish(env);
}
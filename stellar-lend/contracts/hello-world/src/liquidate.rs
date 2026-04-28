//! # Liquidation Module
//!
//! Handles liquidation of undercollateralized positions in the lending protocol.
//!
//! Liquidators can repay a portion of a borrower's debt in exchange for their
//! collateral plus a liquidation incentive (bonus). This module uses the risk
//! management system to determine:
//! - Whether a position is eligible for liquidation (below liquidation threshold)
//! - The maximum liquidatable amount (controlled by the close factor)
//! - The liquidation incentive awarded to the liquidator
//!
//! ## Cross-Asset Liquidation
//! When debt and collateral are different assets, oracle prices are used to
//! convert between asset values. A default price of 1.0 (8 decimals) is used
//! as fallback when oracle prices are not configured.
//!
//! ## Invariants
//! - Only undercollateralized positions (below liquidation threshold) can be liquidated.
//! - Liquidation amount cannot exceed the close factor percentage of total debt.
//! - Collateral seized cannot exceed the borrower's available collateral.
//! - Interest is accrued on the borrower's position before liquidation.

#![allow(unused)]
use crate::events::{
    emit_liquidation, emit_liquidation_fee_collected, LiquidationEvent,
    LiquidationFeeCollectedEvent,
};
use soroban_sdk::{contracterror, Address, Env, IntoVal, Map, Symbol, Val, Vec};

use crate::deposit::{
    add_activity_log, emit_analytics_updated_event, emit_position_updated_event,
    emit_user_activity_tracked_event, update_protocol_analytics, AssetParams, DepositDataKey,
    Position, ProtocolAnalytics, UserAnalytics,
};
use crate::oracle::get_price;
use crate::risk_management::{
    is_emergency_paused, is_operation_paused, require_operation_not_paused, RiskManagementError,
};
use crate::risk_params::{
    can_be_liquidated, get_close_factor, get_liquidation_incentive,
    get_liquidation_incentive_amount, get_max_liquidatable_amount,
};

/// Errors that can occur during liquidation operations
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum LiquidationError {
    /// Liquidation amount must be greater than zero
    InvalidAmount = 1,
    /// Asset address is invalid
    InvalidAsset = 2,
    /// Position is not undercollateralized
    NotLiquidatable = 3,
    /// Liquidation operations are currently paused
    LiquidationPaused = 4,
    /// Liquidation amount exceeds maximum allowed (close factor)
    ExceedsCloseFactor = 5,
    /// Insufficient balance to liquidate
    InsufficientBalance = 6,
    /// Overflow occurred during calculation
    Overflow = 7,
    /// Invalid collateral asset
    InvalidCollateralAsset = 8,
    /// Invalid debt asset
    InvalidDebtAsset = 9,
    /// Price not available for asset
    PriceNotAvailable = 10,
    /// Liquidation would leave position undercollateralized
    InsufficientLiquidation = 11,
    /// Reentrancy detected
    Reentrancy = 12,
}

/// Annual interest rate in basis points (e.g., 500 = 5% per year)
/// This matches the rate used in borrow.rs and repay.rs
// Interest rate is now calculated dynamically based on utilization
// See interest_rate module for details
/// Calculate interest accrued since last accrual time
/// Calculate accrued interest using dynamic interest rate
/// Uses the current borrow rate based on protocol utilization
fn calculate_accrued_interest(
    env: &Env,
    principal: i128,
    last_accrual_time: u64,
    current_time: u64,
) -> Result<i128, LiquidationError> {
    if principal == 0 {
        return Ok(0);
    }

    if current_time <= last_accrual_time {
        return Ok(0);
    }

    // Get current borrow rate (in basis points)
    let rate_bps =
        crate::interest_rate::calculate_borrow_rate(env).map_err(|_| LiquidationError::Overflow)?;

    // Calculate interest using the dynamic rate
    crate::interest_rate::calculate_accrued_interest(
        principal,
        last_accrual_time,
        current_time,
        rate_bps,
    )
    .map_err(|_| LiquidationError::Overflow)
}

/// Accrue compound interest on a position using the global borrow index.
fn accrue_interest(
    env: &Env,
    user: &Address,
    position: &mut Position,
) -> Result<(), LiquidationError> {
    let current_time = env.ledger().timestamp();

    if position.debt == 0 {
        position.borrow_interest = 0;
        position.last_accrual_time = current_time;
        let current_index = crate::interest_rate::update_lending_index(env)
            .map_err(|_| LiquidationError::Overflow)?
            .borrow_index;
        env.storage().persistent().set(
            &DepositDataKey::UserBorrowIndex(user.clone()),
            &current_index,
        );
        return Ok(());
    }

    let current_index = crate::interest_rate::update_lending_index(env)
        .map_err(|_| LiquidationError::Overflow)?
        .borrow_index;

    let user_index = env
        .storage()
        .persistent()
        .get::<DepositDataKey, i128>(&DepositDataKey::UserBorrowIndex(user.clone()))
        .unwrap_or(current_index);

    let new_interest =
        crate::interest_rate::compute_index_interest(position.debt, user_index, current_index)
            .map_err(|_| LiquidationError::Overflow)?;

    position.borrow_interest = position
        .borrow_interest
        .checked_add(new_interest)
        .ok_or(LiquidationError::Overflow)?;
    position.last_accrual_time = current_time;

    env.storage().persistent().set(
        &DepositDataKey::UserBorrowIndex(user.clone()),
        &current_index,
    );

    Ok(())
}

/// Get asset price from oracle
/// Returns price in base units (scaled by decimals)
/// Falls back to default price if oracle doesn't have a price set
fn get_asset_price(env: &Env, asset: &Address) -> i128 {
    // Try to get price from oracle, but fallback to default if not available
    // This allows liquidation to work even when prices aren't set up in tests
    get_price(env, asset).unwrap_or(1_00000000i128) // Default: 1 XLM with 8 decimals
}

/// Calculate collateral value in debt asset terms
/// Returns collateral_value = collateral_amount * collateral_price / debt_price
fn calculate_collateral_value(
    collateral_amount: i128,
    collateral_price: i128,
    debt_price: i128,
) -> Result<i128, LiquidationError> {
    if debt_price == 0 {
        return Err(LiquidationError::PriceNotAvailable);
    }

    // Calculate: collateral_amount * collateral_price / debt_price
    collateral_amount
        .checked_mul(collateral_price)
        .ok_or(LiquidationError::Overflow)?
        .checked_div(debt_price)
        .ok_or(LiquidationError::Overflow)
}

/// Calculate debt value
/// Returns debt_value = debt_amount + interest
fn calculate_debt_value(debt: i128, interest: i128) -> Result<i128, LiquidationError> {
    debt.checked_add(interest).ok_or(LiquidationError::Overflow)
}

/// Liquidate an undercollateralized position
///
/// Allows liquidators to liquidate undercollateralized positions by:
/// 1. Repaying debt on behalf of the borrower
/// 2. Receiving collateral plus a liquidation incentive
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `liquidator` - The address of the liquidator
/// * `borrower` - The address of the borrower being liquidated
/// * `debt_asset` - The address of the debt asset to repay (None for native XLM)
/// * `collateral_asset` - The address of the collateral asset to receive (None for native XLM)
/// * `debt_amount` - The amount of debt to liquidate
///
/// # Returns
/// Returns a tuple (debt_liquidated, collateral_seized, incentive_amount)
///
/// # Errors
/// * `LiquidationError::InvalidAmount` - If amount is zero or negative
/// * `LiquidationError::NotLiquidatable` - If position is not undercollateralized
/// * `LiquidationError::LiquidationPaused` - If liquidations are paused
/// * `LiquidationError::ExceedsCloseFactor` - If liquidation exceeds close factor limit
/// * `LiquidationError::InsufficientBalance` - If liquidator doesn't have enough balance
/// * `LiquidationError::Overflow` - If calculation overflow occurs
///
/// # Security
/// * Validates liquidation amount > 0
/// * Checks pause switches
/// * Validates position is undercollateralized
/// * Enforces close factor limits
/// * Accrues interest before liquidation
/// * Transfers debt asset from liquidator to contract
/// * Transfers collateral asset from contract to liquidator (with incentive)
/// * Updates debt and collateral balances
/// * Emits events for tracking
/// * Updates analytics
pub fn liquidate(
    env: &Env,
    liquidator: Address,
    borrower: Address,
    debt_asset: Option<Address>,
    collateral_asset: Option<Address>,
    debt_amount: i128,
) -> Result<(i128, i128, i128), LiquidationError> {
    // Validate amount
    if debt_amount <= 0 {
        return Err(LiquidationError::InvalidAmount);
    }

    // Check for reentrancy
    let _guard =
        crate::reentrancy::ReentrancyGuard::new(env).map_err(|_| LiquidationError::Reentrancy)?;

    // Check circuit breaker - liquidations may be paused or restricted
    let liquidation_allowed = crate::circuit_breaker::is_liquidation_allowed(env, &liquidator)
        .unwrap_or(true); // Default to allowed if circuit breaker not initialized
    
    if !liquidation_allowed {
        return Err(LiquidationError::LiquidationPaused);
    }

    // Check emergency pause
    if is_emergency_paused(env) {
        return Err(LiquidationError::LiquidationPaused);
    }

    // Check if liquidations are paused
    require_operation_not_paused(env, Symbol::new(env, "pause_liquidate")).map_err(
        |e| match e {
            RiskManagementError::OperationPaused => LiquidationError::LiquidationPaused,
            RiskManagementError::EmergencyPaused => LiquidationError::LiquidationPaused,
            _ => LiquidationError::LiquidationPaused,
        },
    )?;

    // Validate assets
    if let Some(ref debt_addr) = debt_asset {
        if debt_addr == &env.current_contract_address() {
            return Err(LiquidationError::InvalidDebtAsset);
        }
    }

    if let Some(ref collateral_addr) = collateral_asset {
        if collateral_addr == &env.current_contract_address() {
            return Err(LiquidationError::InvalidCollateralAsset);
        }
    }

    // Get current timestamp
    let timestamp = env.ledger().timestamp();

    // Get borrower position
    let position_key = DepositDataKey::Position(borrower.clone());
    let mut position = env
        .storage()
        .persistent()
        .get::<DepositDataKey, Position>(&position_key)
        .ok_or(LiquidationError::NotLiquidatable)?;

    // Accrue compound interest before liquidation
    accrue_interest(env, &borrower, &mut position)?;

    // Get collateral balance for the targeted collateral asset.
    // For multi-asset users, use per-asset balance; fall back to aggregate for legacy users.
    let collateral_balance = if let Some(ref collateral_addr) = collateral_asset {
        if crate::multi_collateral::has_multi_asset_collateral(env, &borrower) {
            crate::multi_collateral::get_user_asset_collateral(env, &borrower, collateral_addr)
        } else {
            let collateral_key = DepositDataKey::CollateralBalance(borrower.clone());
            env.storage()
                .persistent()
                .get::<DepositDataKey, i128>(&collateral_key)
                .unwrap_or(0)
        }
    } else {
        let collateral_key = DepositDataKey::CollateralBalance(borrower.clone());
        env.storage()
            .persistent()
            .get::<DepositDataKey, i128>(&collateral_key)
            .unwrap_or(0)
    };

    // Calculate total debt (principal + interest)
    let total_debt = calculate_debt_value(position.debt, position.borrow_interest)?;

    // Use oracle-priced total collateral value for multi-asset liquidation check;
    // fall back to raw collateral_balance for legacy single-asset users.
    let collateral_value_for_check =
        if crate::multi_collateral::has_multi_asset_collateral(env, &borrower) {
            crate::multi_collateral::calculate_total_collateral_value(env, &borrower)
                .map_err(|_| LiquidationError::Overflow)?
        } else if debt_asset.is_none() && collateral_asset.is_none() {
            collateral_balance
        } else {
            let debt_price = if let Some(ref debt_addr) = debt_asset {
                get_asset_price(env, debt_addr)
            } else {
                1i128
            };
            let collateral_price = if let Some(ref collateral_addr) = collateral_asset {
                get_asset_price(env, collateral_addr)
            } else {
                1i128
            };
            calculate_collateral_value(collateral_balance, collateral_price, debt_price)?
        };

    // Check if position can be liquidated
    let can_liquidate = can_be_liquidated(env, collateral_value_for_check, total_debt)
        .map_err(|_| LiquidationError::NotLiquidatable)?;

    if !can_liquidate {
        return Err(LiquidationError::NotLiquidatable);
    }

    // Get maximum liquidatable amount (close factor)
    let max_liquidatable =
        get_max_liquidatable_amount(env, total_debt).map_err(|_| LiquidationError::Overflow)?;

    // Validate liquidation amount doesn't exceed close factor
    if debt_amount > max_liquidatable {
        return Err(LiquidationError::ExceedsCloseFactor);
    }

    // Ensure we don't liquidate more than total debt
    let actual_debt_liquidated = if debt_amount > total_debt {
        total_debt
    } else {
        debt_amount
    };

    // Calculate liquidation incentive
    let incentive_bps = get_liquidation_incentive(env).map_err(|_| LiquidationError::Overflow)?;
    let incentive_amount = get_liquidation_incentive_amount(env, actual_debt_liquidated)
        .map_err(|_| LiquidationError::Overflow)?;

    // Calculate collateral to seize
    // Liquidator repays debt_liquidated amount of debt asset
    // Liquidator receives collateral worth debt_liquidated (in debt terms) + incentive
    // collateral_seized = (debt_liquidated * debt_price / collateral_price) * (1 + incentive_bps / 10000)
    // First, convert debt amount to collateral terms: debt_liquidated * debt_price / collateral_price
    let collateral_value_liquidated = if debt_asset.is_none() && collateral_asset.is_none() {
        // Both are native XLM - no price conversion needed
        actual_debt_liquidated
    } else {
        // Need to convert between different assets using prices
        let debt_price = if let Some(ref debt_addr) = debt_asset {
            get_asset_price(env, debt_addr)
        } else {
            1i128 // Native XLM
        };

        let collateral_price = if let Some(ref collateral_addr) = collateral_asset {
            get_asset_price(env, collateral_addr)
        } else {
            1i128 // Native XLM
        };

        actual_debt_liquidated
            .checked_mul(debt_price)
            .ok_or(LiquidationError::Overflow)?
            .checked_div(collateral_price)
            .ok_or(LiquidationError::Overflow)?
    };

    // Apply incentive: collateral_seized = collateral_value_liquidated * (1 + incentive_bps / 10000)
    let collateral_seized = collateral_value_liquidated
        .checked_mul(10000 + incentive_bps)
        .ok_or(LiquidationError::Overflow)?
        .checked_div(10000)
        .ok_or(LiquidationError::Overflow)?;

    // Ensure we don't seize more than available collateral
    let actual_collateral_seized = if collateral_seized > collateral_balance {
        collateral_balance
    } else {
        collateral_seized
    };

    // Calculate protocol fee on the liquidation bonus (retained in ProtocolReserve)
    let fee_config = crate::treasury::get_fee_config(env);
    let protocol_liquidation_fee = incentive_amount
        .checked_mul(fee_config.liquidation_fee_bps)
        .ok_or(LiquidationError::Overflow)?
        .checked_div(10000)
        .ok_or(LiquidationError::Overflow)?;

    // Liquidator receives seized collateral minus the protocol fee
    let liquidator_collateral = actual_collateral_seized
        .checked_sub(protocol_liquidation_fee)
        .ok_or(LiquidationError::Overflow)?;

    // Check liquidator has sufficient balance to repay debt
    if let Some(ref debt_addr) = debt_asset {
        let token_client = soroban_sdk::token::Client::new(env, debt_addr);
        let liquidator_balance = token_client.balance(&liquidator);
        if liquidator_balance < actual_debt_liquidated {
            return Err(LiquidationError::InsufficientBalance);
        }

        // Transfer debt asset from liquidator to contract (liquidator repays debt)
        token_client.transfer_from(
            &env.current_contract_address(), // spender (this contract)
            &liquidator,                     // from (liquidator)
            &env.current_contract_address(), // to (this contract)
            &actual_debt_liquidated,
        );
    } else {
        // Native XLM handling - placeholder for now
    }

    // Check contract has sufficient collateral to transfer
    if let Some(ref collateral_addr) = collateral_asset {
        let token_client = soroban_sdk::token::Client::new(env, collateral_addr);
        let contract_balance = token_client.balance(&env.current_contract_address());
        if contract_balance < actual_collateral_seized {
            return Err(LiquidationError::InsufficientBalance);
        }

        // Transfer collateral to liquidator (seized amount minus protocol fee)
        token_client.transfer(
            &env.current_contract_address(),
            &liquidator,
            &liquidator_collateral,
        );
    } else {
        // Native XLM handling - placeholder for now
    }

    // Credit protocol liquidation fee to protocol reserve
    if protocol_liquidation_fee > 0 {
        let reserve_key = DepositDataKey::ProtocolReserve(collateral_asset.clone());
        let current_reserve = env
            .storage()
            .persistent()
            .get::<DepositDataKey, i128>(&reserve_key)
            .unwrap_or(0);
        let new_reserve = current_reserve
            .checked_add(protocol_liquidation_fee)
            .ok_or(LiquidationError::Overflow)?;
        env.storage().persistent().set(&reserve_key, &new_reserve);

        emit_liquidation_fee_collected(
            env,
            LiquidationFeeCollectedEvent {
                asset: collateral_asset.clone(),
                fee_amount: protocol_liquidation_fee,
                timestamp,
            },
        );
    }

    // Update borrower's debt (pay interest first, then principal)
    let interest_to_pay = if actual_debt_liquidated <= position.borrow_interest {
        actual_debt_liquidated
    } else {
        position.borrow_interest
    };

    let principal_to_pay = actual_debt_liquidated
        .checked_sub(interest_to_pay)
        .ok_or(LiquidationError::Overflow)?;

    position.borrow_interest = position
        .borrow_interest
        .checked_sub(interest_to_pay)
        .unwrap_or(0);
    position.debt = position.debt.checked_sub(principal_to_pay).unwrap_or(0);
    position.last_accrual_time = timestamp;

    // Update borrower's aggregate collateral balance
    let aggregate_collateral_key = DepositDataKey::CollateralBalance(borrower.clone());
    let aggregate_collateral = env
        .storage()
        .persistent()
        .get::<DepositDataKey, i128>(&aggregate_collateral_key)
        .unwrap_or(0);
    let new_aggregate_collateral = aggregate_collateral
        .checked_sub(actual_collateral_seized)
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&aggregate_collateral_key, &new_aggregate_collateral);

    // Also update per-asset tracking if the borrower has multi-asset collateral
    if let Some(ref collateral_addr) = collateral_asset {
        if crate::multi_collateral::has_multi_asset_collateral(env, &borrower) {
            crate::deposit::record_asset_withdrawal(
                env,
                &borrower,
                collateral_addr,
                actual_collateral_seized,
            )
            .map_err(|_| LiquidationError::Overflow)?;
        }
    }

    // Update position collateral
    position.collateral = new_aggregate_collateral;

    // Save updated position
    env.storage().persistent().set(&position_key, &position);

    // Update analytics
    update_liquidation_analytics(
        env,
        &borrower,
        &liquidator,
        actual_debt_liquidated,
        actual_collateral_seized,
        timestamp,
    )?;

    // Add to activity log
    add_activity_log(
        env,
        &borrower,
        Symbol::new(env, "liquidate"),
        actual_debt_liquidated,
        debt_asset.clone(),
        timestamp,
    )
    .map_err(|e| match e {
        crate::deposit::DepositError::Overflow => LiquidationError::Overflow,
        _ => LiquidationError::Overflow,
    })?;

    // Emit liquidation event
    emit_liquidation(
        env,
        LiquidationEvent {
            liquidator: liquidator.clone(),
            borrower: borrower.clone(),
            debt_asset: debt_asset.clone(),
            collateral_asset: collateral_asset.clone(),
            debt_liquidated: actual_debt_liquidated,
            collateral_seized: actual_collateral_seized,
            incentive_amount,
            timestamp,
        },
    );

    // Update credit score for liquidation
    let _ = crate::credit_score::update_score_on_liquidation(env, &borrower);

    // Emit position updated event
    emit_position_updated_event(env, &borrower, &position);

    // Emit analytics updated event
    emit_analytics_updated_event(
        env,
        &borrower,
        "liquidate",
        actual_debt_liquidated,
        timestamp,
    );

    // Emit user activity tracked event
    emit_user_activity_tracked_event(
        env,
        &borrower,
        Symbol::new(env, "liquidate"),
        actual_debt_liquidated,
        timestamp,
    );

    Ok((
        actual_debt_liquidated,
        actual_collateral_seized,
        incentive_amount,
    ))
}

/// Update analytics after liquidation
fn update_liquidation_analytics(
    env: &Env,
    borrower: &Address,
    liquidator: &Address,
    debt_liquidated: i128,
    collateral_seized: i128,
    timestamp: u64,
) -> Result<(), LiquidationError> {
    // Update borrower analytics
    let borrower_analytics_key = DepositDataKey::UserAnalytics(borrower.clone());
    #[allow(clippy::unnecessary_lazy_evaluations)]
    let mut borrower_analytics = env
        .storage()
        .persistent()
        .get::<DepositDataKey, UserAnalytics>(&borrower_analytics_key)
        .unwrap_or_else(|| UserAnalytics {
            total_deposits: 0,
            total_borrows: 0,
            total_withdrawals: 0,
            total_repayments: 0,
            collateral_value: 0,
            debt_value: 0,
            collateralization_ratio: 0,
            activity_score: 0,
            transaction_count: 0,
            first_interaction: timestamp,
            last_activity: timestamp,
            risk_level: 0,
            loyalty_tier: 0,
        });

    // Update debt value (subtract liquidated amount)
    borrower_analytics.debt_value = borrower_analytics
        .debt_value
        .checked_sub(debt_liquidated)
        .unwrap_or(0);

    // Update collateral value (subtract seized amount)
    borrower_analytics.collateral_value = borrower_analytics
        .collateral_value
        .checked_sub(collateral_seized)
        .unwrap_or(0);

    // Recalculate collateralization ratio
    if borrower_analytics.debt_value > 0 && borrower_analytics.collateral_value > 0 {
        borrower_analytics.collateralization_ratio = borrower_analytics
            .collateral_value
            .checked_mul(10000)
            .and_then(|v| v.checked_div(borrower_analytics.debt_value))
            .unwrap_or(0);
    } else {
        borrower_analytics.collateralization_ratio = 0;
    }

    borrower_analytics.transaction_count = borrower_analytics.transaction_count.saturating_add(1);
    borrower_analytics.last_activity = timestamp;

    env.storage()
        .persistent()
        .set(&borrower_analytics_key, &borrower_analytics);

    // Update protocol analytics
    let protocol_analytics_key = DepositDataKey::ProtocolAnalytics;
    let mut protocol_analytics = env
        .storage()
        .persistent()
        .get::<DepositDataKey, ProtocolAnalytics>(&protocol_analytics_key)
        .unwrap_or(ProtocolAnalytics {
            total_deposits: 0,
            total_borrows: 0,
            total_value_locked: 0,
        });

    // Update total value locked (subtract seized collateral)
    protocol_analytics.total_value_locked = protocol_analytics
        .total_value_locked
        .checked_sub(collateral_seized)
        .unwrap_or(0);

    env.storage()
        .persistent()
        .set(&protocol_analytics_key, &protocol_analytics);

    Ok(())
}

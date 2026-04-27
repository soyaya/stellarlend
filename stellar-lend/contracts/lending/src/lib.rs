#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Bytes, Env, Val, Vec};

pub mod borrow;
mod deposit;
pub mod events;
mod flash_loan;
pub mod invariants;
pub mod pause;
mod token_receiver;
mod withdraw;
pub mod yield_farming;

// Re-export contract types used in the public interface so downstream tooling
// can construct and inspect them without relying on private module paths.
pub use borrow::{BorrowCollateral, BorrowError, DebtPosition, StablecoinConfig};
pub use deposit::{DepositCollateral, DepositError};
pub use flash_loan::FlashLoanError;
pub use pause::PauseType;
pub use views::{ProtocolMetrics, ProtocolReport, StablecoinAssetStats, UserPositionSummary};
pub use withdraw::WithdrawError;

pub use commitments::{
    BorrowCommitment, CommitmentError, CommitmentStatus, PriceTrigger, TriggerCombiner,
};
pub use events::RiskAlertSeverity;
pub use risk_monitor::{RiskAlertThresholds, RiskMonitorError};

use borrow::{
    borrow as borrow_cmd, deposit as borrow_deposit, get_admin as get_borrow_admin,
    get_stablecoin_config as get_stablecoin_config_logic,
    get_user_collateral as get_borrow_collateral, get_user_debt as get_borrow_debt,
    initialize_borrow_settings as initialize_borrow_logic, repay as borrow_repay,
    set_admin as set_borrow_admin,
    set_liquidation_threshold_bps as set_liquidation_threshold_logic,
    set_oracle as set_oracle_logic, set_stablecoin_config as set_stablecoin_config_logic,
};
use deposit::{
    deposit as deposit_logic, get_user_collateral as get_deposit_collateral,
    initialize_deposit_settings as initialize_deposit_logic,
};
use flash_loan::{
    flash_loan as flash_loan_logic, set_flash_loan_fee_bps as set_flash_loan_fee_logic,
};
use pause::{is_paused, set_pause as set_pause_logic};
use token_receiver::receive as receive_logic;

pub mod views;
use views::{
    get_collateral_balance as view_collateral_balance,
    get_collateral_value as view_collateral_value, get_debt_balance as view_debt_balance,
    get_debt_value as view_debt_value, get_health_factor as view_health_factor,
    get_user_position as view_user_position,
};

use withdraw::{
    initialize_withdraw_settings as initialize_withdraw_logic,
    set_withdraw_paused as set_withdraw_paused_logic, withdraw as withdraw_logic,
};

#[derive(Clone)]
#[contracttype]
pub enum BadDebtKey {
    Total,
    User(Address),
}

#[derive(Clone)]
#[contracttype]
pub enum ReserveKey {
    ProtocolReserves,
}

mod commitments;
mod data_store;
mod risk_monitor;
pub mod upgrade;

#[cfg(test)]
mod commitments_test;
#[cfg(test)]
mod borrow_test;
#[cfg(test)]
mod data_store_test;
#[cfg(test)]
mod deposit_test;
#[cfg(test)]
mod flash_loan_test;
#[cfg(test)]
mod math_safety_test;
#[cfg(test)]
mod pause_test;
#[cfg(test)]
mod stablecoin_test;
#[cfg(test)]
mod token_receiver_test;
#[cfg(test)]
mod upgrade_test;
#[cfg(test)]
mod views_test;
#[cfg(test)]
mod withdraw_test;

#[contract]
pub struct LendingContract;

#[contractimpl]
impl LendingContract {
    /// Initialize the protocol with admin and settings.
    pub fn initialize(
        env: Env,
        admin: Address,
        debt_ceiling: i128,
        min_borrow_amount: i128,
    ) -> Result<(), BorrowError> {
        if get_borrow_admin(&env).is_some() {
            return Err(BorrowError::Unauthorized);
        }
        set_borrow_admin(&env, &admin);
        initialize_borrow_logic(&env, debt_ceiling, min_borrow_amount)?;
        Ok(())
    }

    pub fn get_total_bad_debt(env: &Env) -> i128 {
        env.storage()
            .persistent()
            .get(&BadDebtKey::Total)
            .unwrap_or(0)
    }

    pub fn add_bad_debt(env: &Env, user: &Address, amount: i128) {
        let mut total = Self::get_total_bad_debt(env);
        total += amount;

        env.storage().persistent().set(&BadDebtKey::Total, &total);

        let user_key = BadDebtKey::User(user.clone());
        let mut user_debt = env.storage().persistent().get(&user_key).unwrap_or(0);
        user_debt += amount;

        env.storage().persistent().set(&user_key, &user_debt);
    }

    pub fn get_reserves(env: &Env) -> i128 {
        env.storage()
            .persistent()
            .get(&ReserveKey::ProtocolReserves)
            .unwrap_or(0)
    }

    /// Borrow assets against deposited collateral.
    pub fn borrow(
        env: Env,
        user: Address,
        asset: Address,
        amount: i128,
        collateral_asset: Address,
        collateral_amount: i128,
    ) -> Result<(), BorrowError> {
        borrow_cmd(
            &env,
            user,
            asset,
            amount,
            collateral_asset,
            collateral_amount,
        )
    }

    /// Set protocol pause state for a specific operation.
    pub fn set_pause(
        env: Env,
        admin: Address,
        pause_type: PauseType,
        paused: bool,
    ) -> Result<(), BorrowError> {
        let current_admin = get_borrow_admin(&env).ok_or(BorrowError::Unauthorized)?;
        if admin != current_admin {
            return Err(BorrowError::Unauthorized);
        }
        admin.require_auth();
        set_pause_logic(&env, admin, pause_type, paused);
        Ok(())
    }

    /// Repay borrowed assets.
    pub fn repay(env: Env, user: Address, asset: Address, amount: i128) -> Result<(), BorrowError> {
        user.require_auth();
        if is_paused(&env, PauseType::Repay) {
            return Err(BorrowError::ProtocolPaused);
        }
        borrow_repay(&env, user, asset, amount)
    }

    /// Deposit collateral into the protocol.
    pub fn deposit(
        env: Env,
        user: Address,
        asset: Address,
        amount: i128,
    ) -> Result<i128, DepositError> {
        if is_paused(&env, PauseType::Deposit) {
            return Err(DepositError::DepositPaused);
        }
        deposit_logic(&env, user, asset, amount)
    }

    /// Deposit collateral for a borrow position.
    pub fn deposit_collateral(
        env: Env,
        user: Address,
        asset: Address,
        amount: i128,
    ) -> Result<(), BorrowError> {
        user.require_auth();
        if is_paused(&env, PauseType::Deposit) {
            return Err(BorrowError::ProtocolPaused);
        }
        borrow_deposit(&env, user, asset, amount)
    }

    /// Liquidate a position and record any unrecovered debt.
    pub fn liquidate(
        env: Env,
        liquidator: Address,
        borrower: Address,
        _debt_asset: Address,
        _collateral_asset: Address,
        repay_amount: i128,
    ) -> Result<(), BorrowError> {
        liquidator.require_auth();

        if is_paused(&env, PauseType::Liquidation) {
            return Err(BorrowError::ProtocolPaused);
        }

        let debt_value = view_debt_value(&env, &borrower);

        let health = view_health_factor(&env, &borrower);
        if health >= 10000 {
            return Err(BorrowError::PositionHealthy);
        }

        let recovered_value = repay_amount;
        if recovered_value < debt_value {
            let bad_debt = debt_value - recovered_value;
            Self::add_bad_debt(&env, &borrower, bad_debt);
            events::emit_bad_debt(&env, &borrower, bad_debt);
        }

        Ok(())
    }

    /// Get user's debt position.
    pub fn get_user_debt(env: Env, user: Address) -> DebtPosition {
        get_borrow_debt(&env, &user)
    }

    /// Get user's collateral position from the borrow module.
    pub fn get_user_collateral(env: Env, user: Address) -> BorrowCollateral {
        get_borrow_collateral(&env, &user)
    }

    /// Returns the user's collateral balance.
    pub fn get_collateral_balance(env: Env, user: Address) -> i128 {
        view_collateral_balance(&env, &user)
    }

    /// Returns the user's debt balance.
    pub fn get_debt_balance(env: Env, user: Address) -> i128 {
        view_debt_balance(&env, &user)
    }

    /// Returns the user's collateral value.
    pub fn get_collateral_value(env: Env, user: Address) -> i128 {
        view_collateral_value(&env, &user)
    }

    /// Returns the user's debt value.
    pub fn get_debt_value(env: Env, user: Address) -> i128 {
        view_debt_value(&env, &user)
    }

    /// Returns health factor, scaled as 10000 = 1.0.
    pub fn get_health_factor(env: Env, user: Address) -> i128 {
        view_health_factor(&env, &user)
    }

    /// Returns full position summary.
    pub fn get_user_position(env: Env, user: Address) -> UserPositionSummary {
        view_user_position(&env, &user)
    }

    /// Set oracle address for price feeds.
    pub fn set_oracle(env: Env, admin: Address, oracle: Address) -> Result<(), BorrowError> {
        set_oracle_logic(&env, &admin, oracle)
    }

    /// Set liquidation threshold in basis points.
    pub fn set_liquidation_threshold_bps(
        env: Env,
        admin: Address,
        bps: i128,
    ) -> Result<(), BorrowError> {
        set_liquidation_threshold_logic(&env, &admin, bps)
    }

    /// Initialize deposit settings.
    pub fn initialize_deposit_settings(
        env: Env,
        deposit_cap: i128,
        min_deposit_amount: i128,
    ) -> Result<(), DepositError> {
        initialize_deposit_logic(&env, deposit_cap, min_deposit_amount)
    }

    /// Deprecated: use set_pause instead.
    pub fn set_deposit_paused(env: Env, paused: bool) -> Result<(), DepositError> {
        env.storage()
            .persistent()
            .set(&pause::PauseDataKey::State(PauseType::Deposit), &paused);
        Ok(())
    }

    /// Get user's deposit collateral position.
    pub fn get_user_collateral_deposit(
        env: Env,
        user: Address,
        asset: Address,
    ) -> DepositCollateral {
        get_deposit_collateral(&env, &user, &asset)
    }

    /// Get protocol admin.
    pub fn get_admin(env: Env) -> Option<Address> {
        get_borrow_admin(&env)
    }

    /// Execute a flash loan.
    pub fn flash_loan(
        env: Env,
        receiver: Address,
        asset: Address,
        amount: i128,
        params: Bytes,
    ) -> Result<(), FlashLoanError> {
        flash_loan_logic(&env, receiver, asset, amount, params)
    }

    /// Set the flash loan fee in basis points.
    pub fn set_flash_loan_fee_bps(env: Env, fee_bps: i128) -> Result<(), FlashLoanError> {
        let current_admin = get_borrow_admin(&env).ok_or(FlashLoanError::Unauthorized)?;
        current_admin.require_auth();
        set_flash_loan_fee_logic(&env, fee_bps)
    }

    /// Withdraw collateral from the protocol.
    pub fn withdraw(
        env: Env,
        user: Address,
        asset: Address,
        amount: i128,
    ) -> Result<i128, WithdrawError> {
        if is_paused(&env, PauseType::Withdraw) {
            return Err(WithdrawError::WithdrawPaused);
        }
        withdraw_logic(&env, user, asset, amount)
    }

    /// Initialize withdraw settings.
    pub fn initialize_withdraw_settings(
        env: Env,
        min_withdraw_amount: i128,
    ) -> Result<(), WithdrawError> {
        initialize_withdraw_logic(&env, min_withdraw_amount)
    }

    /// Set withdraw pause state.
    pub fn set_withdraw_paused(env: Env, paused: bool) -> Result<(), WithdrawError> {
        set_withdraw_paused_logic(&env, paused)
    }

    /// Token receiver hook.
    pub fn receive(
        env: Env,
        token_asset: Address,
        from: Address,
        amount: i128,
        payload: Vec<Val>,
    ) -> Result<(), BorrowError> {
        receive_logic(env, token_asset, from, amount, payload)
    }

    /// Initialize borrow settings.
    pub fn initialize_borrow_settings(
        env: Env,
        debt_ceiling: i128,
        min_borrow_amount: i128,
    ) -> Result<(), BorrowError> {
        initialize_borrow_logic(&env, debt_ceiling, min_borrow_amount)
    }

    /// Set stablecoin configuration for an asset.
    pub fn set_stablecoin_config(
        env: Env,
        admin: Address,
        asset: Address,
        config: StablecoinConfig,
    ) -> Result<(), BorrowError> {
        set_stablecoin_config_logic(&env, &admin, asset, config)
    }

    /// Get stablecoin configuration for an asset.
    pub fn get_stablecoin_config(env: Env, asset: Address) -> Option<StablecoinConfig> {
        get_stablecoin_config_logic(&env, &asset)
    }

    /// Get protocol report including stablecoin stats.
    pub fn get_protocol_report(env: Env, stablecoin_assets: Vec<Address>) -> ProtocolReport {
        views::get_protocol_report(&env, stablecoin_assets)
    }

    /// Configure utilization threshold alerts (warning / critical / emergency in basis points of debt vs ceiling).
    pub fn set_risk_alert_thresholds(
        env: Env,
        admin: Address,
        thresholds: RiskAlertThresholds,
    ) -> Result<(), RiskMonitorError> {
        risk_monitor::set_risk_alert_thresholds(&env, admin, thresholds)
    }

    pub fn get_risk_alert_thresholds(env: Env) -> Option<RiskAlertThresholds> {
        risk_monitor::get_risk_alert_thresholds(&env)
    }

    pub fn create_borrow_commitment(
        env: Env,
        owner: Address,
        triggers: Vec<PriceTrigger>,
        combiner: TriggerCombiner,
        borrow_asset: Address,
        collateral_asset: Address,
        borrow_amount: i128,
        collateral_amount: i128,
        min_fill_bps: u32,
        expiry_timestamp: u64,
    ) -> Result<u64, CommitmentError> {
        commitments::create_borrow_commitment(
            &env,
            owner,
            triggers,
            combiner,
            borrow_asset,
            collateral_asset,
            borrow_amount,
            collateral_amount,
            min_fill_bps,
            expiry_timestamp,
        )
    }

    pub fn cancel_borrow_commitment(
        env: Env,
        owner: Address,
        commitment_id: u64,
    ) -> Result<(), CommitmentError> {
        commitments::cancel_borrow_commitment(&env, owner, commitment_id)
    }

    /// Keeper or user: execute commitment when oracle triggers are satisfied (no owner signature).
    pub fn execute_borrow_commitment(env: Env, commitment_id: u64) -> Result<(), CommitmentError> {
        commitments::execute_borrow_commitment(&env, commitment_id)
    }

    pub fn get_borrow_commitment(env: Env, commitment_id: u64) -> Option<BorrowCommitment> {
        commitments::get_borrow_commitment(&env, commitment_id)
    }

    pub fn recover_bad_debt(env: Env, admin: Address, amount: i128) -> Result<(), BorrowError> {
        let current_admin = get_borrow_admin(&env).ok_or(BorrowError::Unauthorized)?;
        if admin != current_admin {
            return Err(BorrowError::Unauthorized);
        }
        admin.require_auth();

        let mut reserves = Self::get_reserves(&env);
        let mut bad_debt = Self::get_total_bad_debt(&env);

        if reserves < amount {
            return Err(BorrowError::InsufficientReserves);
        }

        let repay_amount = if amount > bad_debt { bad_debt } else { amount };

        reserves -= repay_amount;
        bad_debt -= repay_amount;

        env.storage()
            .persistent()
            .set(&ReserveKey::ProtocolReserves, &reserves);
        env.storage()
            .persistent()
            .set(&BadDebtKey::Total, &bad_debt);

        events::emit_bad_debt_recovered(&env, repay_amount);

        Ok(())
    }
}

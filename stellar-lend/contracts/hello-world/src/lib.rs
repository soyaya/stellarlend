#![allow(clippy::too_many_arguments)]
#![allow(deprecated)]

use soroban_sdk::{contract, contractimpl, Address, Env, String, Vec};

pub mod admin;
pub mod analytics;
pub mod borrow;
pub mod bridge;
pub mod config;
pub mod cross_asset;
pub mod deposit;
pub mod errors;
pub mod events;
pub mod flash_loan;
pub mod governance;
pub mod interest_rate;
pub mod liquidate;
pub mod multi_collateral;
pub mod multisig;
pub mod oracle;
pub mod recovery;
pub mod reentrancy;
pub mod repay;
pub mod reserve;
pub mod risk_management;
pub mod risk_params;
pub mod storage;
pub mod treasury;
pub mod types;
pub mod withdraw;

use crate::deposit::Position;
use crate::errors::LendingError;
use crate::interest_rate::InterestRateError;
use crate::risk_management::RiskManagementError;

/// The StellarLend core contract.
#[contract]
pub struct HelloContract;

#[contractimpl]
impl HelloContract {
    pub fn hello(env: Env) -> String {
        String::from_str(&env, "Hello")
    }

    pub fn gov_initialize(
        env: Env,
        admin: Address,
        vote_token: Address,
        voting_period: Option<u64>,
        execution_delay: Option<u64>,
        quorum_bps: Option<u32>,
        proposal_threshold: Option<i128>,
        timelock_duration: Option<u64>,
        default_voting_threshold: Option<i128>,
    ) -> Result<(), LendingError> {
        governance::initialize(
            &env,
            admin,
            vote_token,
            voting_period,
            execution_delay,
            quorum_bps,
            proposal_threshold,
            timelock_duration,
            default_voting_threshold,
        )
        .map_err(Into::into)
    }

    pub fn initialize(env: Env, admin: Address) -> Result<(), LendingError> {
        if crate::admin::has_admin(&env) {
            return Err(LendingError::Unauthorized);
        }
        crate::admin::set_admin(&env, admin.clone(), None)
            .map_err(|_| RiskManagementError::Unauthorized)?;
        risk_management::initialize_risk_management(&env, admin.clone())?;
        risk_params::initialize_risk_params(&env)
            .map_err(|_| RiskManagementError::InvalidParameter)?;
        interest_rate::initialize_interest_rate_config(&env, admin).map_err(|e| {
            if e == InterestRateError::AlreadyInitialized {
                RiskManagementError::AlreadyInitialized
            } else {
                RiskManagementError::Unauthorized
            }
        })?;
        Ok(())
    }

    pub fn transfer_admin(
        env: Env,
        caller: Address,
        new_admin: Address,
    ) -> Result<(), LendingError> {
        admin::set_admin(&env, new_admin, Some(caller)).map_err(Into::into)
    }

    pub fn deposit_collateral(
        env: Env,
        user: Address,
        asset: Option<Address>,
        amount: i128,
    ) -> Result<i128, LendingError> {
        deposit::deposit_collateral(&env, user, asset, amount).map_err(Into::into)
    }

    pub fn set_risk_params(
        env: Env,
        caller: Address,
        min_collateral_ratio: Option<i128>,
        liquidation_threshold: Option<i128>,
        close_factor: Option<i128>,
        liquidation_incentive: Option<i128>,
    ) -> Result<(), LendingError> {
        // Authorization is handled by risk_management::require_admin.
        risk_management::require_admin(&env, &caller)?;
        risk_params::set_risk_params(
            &env,
            min_collateral_ratio,
            liquidation_threshold,
            close_factor,
            liquidation_incentive,
        )
        .map_err(|_| RiskManagementError::InvalidParameter)?;

        Ok(())
    }

    pub fn borrow_asset(
        env: Env,
        user: Address,
        asset: Option<Address>,
        amount: i128,
    ) -> Result<i128, LendingError> {
        borrow::borrow_asset(&env, user, asset, amount).map_err(Into::into)
    }

    pub fn repay_debt(
        env: Env,
        user: Address,
        asset: Option<Address>,
        amount: i128,
    ) -> Result<(i128, i128, i128), LendingError> {
        repay::repay_debt(&env, user, asset, amount).map_err(Into::into)
    }

    pub fn withdraw_collateral(
        env: Env,
        user: Address,
        asset: Option<Address>,
        amount: i128,
    ) -> Result<i128, LendingError> {
        withdraw::withdraw_collateral(&env, user, asset, amount).map_err(Into::into)
    }

    pub fn liquidate(
        env: Env,
        liquidator: Address,
        borrower: Address,
        debt_asset: Option<Address>,
        collateral_asset: Option<Address>,
        debt_amount: i128,
    ) -> Result<(i128, i128, i128), LendingError> {
        liquidator.require_auth();
        liquidate::liquidate(
            &env,
            liquidator,
            borrower,
            debt_asset,
            collateral_asset,
            debt_amount,
        )
        .map_err(Into::into)
    }

    pub fn set_emergency_pause(
        env: Env,
        caller: Address,
        paused: bool,
    ) -> Result<(), LendingError> {
        // Authorization is handled by risk_management::require_admin.
        risk_management::require_admin(&env, &caller)?;
        risk_management::set_emergency_pause(&env, caller, paused)
            .map_err(Into::into)
    }

    pub fn execute_flash_loan(
        env: Env,
        user: Address,
        asset: Address,
        amount: i128,
        callback: Address,
    ) -> Result<i128, LendingError> {
        flash_loan::execute_flash_loan(&env, user, asset, amount, callback).map_err(Into::into)
    }

    pub fn repay_flash_loan(
        env: Env,
        user: Address,
        asset: Address,
        amount: i128,
    ) -> Result<(), LendingError> {
        flash_loan::repay_flash_loan(&env, user, asset, amount).map_err(Into::into)
    }

    pub fn can_be_liquidated(
        env: Env,
        collateral_value: i128,
        debt_value: i128,
    ) -> Result<bool, LendingError> {
        risk_params::can_be_liquidated(&env, collateral_value, debt_value).map_err(Into::into)
    }

    pub fn get_max_liquidatable_amount(
        env: Env,
        debt_value: i128,
    ) -> Result<i128, LendingError> {
        risk_params::get_max_liquidatable_amount(&env, debt_value).map_err(Into::into)
    }

    pub fn get_liquidation_incentive_amount(
        env: Env,
        liquidated_amount: i128,
    ) -> Result<i128, LendingError> {
        risk_params::get_liquidation_incentive_amount(&env, liquidated_amount).map_err(Into::into)
    }

    pub fn require_min_collateral_ratio(
        env: Env,
        collateral_value: i128,
        debt_value: i128,
    ) -> Result<(), LendingError> {
        risk_params::require_min_collateral_ratio(&env, collateral_value, debt_value)
            .map_err(Into::into)
    }

    // -------------------------------------------------------------------------
    // Treasury & Fee Management
    // -------------------------------------------------------------------------

    /// Set the protocol treasury address (admin-only)
    pub fn set_treasury(
        env: Env,
        caller: Address,
        treasury: Address,
    ) -> Result<(), LendingError> {
        treasury::set_treasury(&env, caller, treasury).map_err(Into::into)
    }

    /// Return the configured treasury address
    pub fn get_treasury(env: Env) -> Option<Address> {
        treasury::get_treasury(&env)
    }

    /// Return accumulated protocol reserves for the given asset
    pub fn get_reserve_balance(env: Env, asset: Option<Address>) -> i128 {
        treasury::get_reserve_balance(&env, asset)
    }

    /// Withdraw protocol reserves to a recipient (admin-only)
    pub fn claim_reserves(
        env: Env,
        caller: Address,
        asset: Option<Address>,
        recipient: Address,
        amount: i128,
    ) -> Result<(), LendingError> {
        treasury::claim_reserves(&env, caller, asset, recipient, amount).map_err(Into::into)
    }

    /// Update protocol fee percentages (admin-only)
    pub fn set_fee_config(
        env: Env,
        caller: Address,
        interest_fee_bps: i128,
        liquidation_fee_bps: i128,
    ) -> Result<(), LendingError> {
        treasury::set_fee_config(
            &env,
            caller,
            treasury::TreasuryFeeConfig {
                interest_fee_bps,
                liquidation_fee_bps,
            },
        )
        .map_err(Into::into)
    }

    /// Return the current fee configuration
    pub fn get_fee_config(env: Env) -> treasury::TreasuryFeeConfig {
        treasury::get_fee_config(&env)
    }

    // -------------------------------------------------------------------------
    // Multi-Asset Collateral
    // -------------------------------------------------------------------------

    /// Return the collateral balance for a specific (user, asset) pair
    pub fn get_user_asset_collateral(env: Env, user: Address, asset: Address) -> i128 {
        multi_collateral::get_user_asset_collateral(&env, &user, &asset)
    }

    /// Return the list of assets in which the user currently holds collateral
    pub fn get_user_asset_list(env: Env, user: Address) -> Vec<Address> {
        multi_collateral::get_user_asset_list(&env, &user)
    }

    /// Return the oracle-weighted total collateral value across all of the
    /// user's deposited assets (collateral factors applied per asset).
    /// Returns 0 for legacy single-asset users.
    pub fn get_user_total_collateral_value(env: Env, user: Address) -> i128 {
        multi_collateral::calculate_total_collateral_value(&env, &user).unwrap_or(0)
    }

    // -------------------------------------------------------------------------
    // Analytics
    // -------------------------------------------------------------------------

    /// Read-only user health factor query (collateral/debt in basis points).
    pub fn get_health_factor(env: Env, user: Address) -> Result<i128, LendingError> {
        analytics::calculate_health_factor(&env, &user).map_err(Into::into)
    }

    /// Read-only protocol metrics snapshot.
    pub fn get_protocol_stats(env: Env) -> Result<analytics::ProtocolMetrics, LendingError> {
        analytics::get_protocol_stats(&env).map_err(Into::into)
    }

    /// Read-only protocol analytics report.
    pub fn get_protocol_report(env: Env) -> Result<analytics::ProtocolReport, LendingError> {
        analytics::generate_protocol_report(&env).map_err(Into::into)
    }

    /// Read-only user position query.
    pub fn get_user_position(env: Env, user: Address) -> Result<Position, LendingError> {
        analytics::get_user_position_summary(&env, &user).map_err(Into::into)
    }

    /// Read-only user analytics report.
    pub fn get_user_report(
        env: Env,
        user: Address,
    ) -> Result<analytics::UserReport, LendingError> {
        analytics::generate_user_report(&env, &user).map_err(Into::into)
    }

    /// Read-only recent protocol activity feed query.
    pub fn get_recent_activity(
        env: Env,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<analytics::ActivityEntry>, LendingError> {
        analytics::get_recent_activity(&env, limit, offset).map_err(Into::into)
    }

    // -------------------------------------------------------------------------
    // Asset Configuration
    // -------------------------------------------------------------------------

    /// Set per-asset deposit/collateral parameters (admin-only).
    pub fn update_asset_config(
        env: Env,
        asset: Address,
        params: deposit::AssetParams,
    ) -> Result<(), LendingError> {
        let admin = crate::admin::get_admin(&env).ok_or(LendingError::Unauthorized)?;
        admin.require_auth();
        deposit::set_asset_params(&env, admin, asset, params).map_err(Into::into)
    }

    // -------------------------------------------------------------------------
    // Flash Loan Configuration
    // -------------------------------------------------------------------------

    /// Configure flash loan parameters (admin-only).
    pub fn configure_flash_loan(
        env: Env,
        caller: Address,
        config: flash_loan::FlashLoanConfig,
    ) -> Result<(), LendingError> {
        flash_loan::set_flash_loan_config(&env, caller, config).map_err(Into::into)
    }
}

#[cfg(test)]
#[path = "tests/cross_contract_test.rs"]
mod cross_contract_test;
#[cfg(test)]
mod flash_loan_test;
#[cfg(test)]
mod multi_collateral_test;
#[cfg(test)]
mod test_reentrancy;
#[cfg(test)]
mod test_zero_amount;
#[cfg(test)]
mod treasury_test;

#![no_std]

pub mod borrow;
mod deposit;
pub mod events;
mod flash_loan;
pub mod invariants;
pub mod pause;
mod token_receiver;
mod withdraw;
pub mod yield_farming;

use soroban_sdk::{
    contract, contractevent, contractimpl, symbol_short, Address, Bytes, Env, Symbol, Val, Vec,
};

const BORROW: Symbol = symbol_short!("borrow");
const REPAY: Symbol = symbol_short!("repay");
const DEPOSIT: Symbol = symbol_short!("deposit");
const WITHDRAW: Symbol = symbol_short!("withdraw");
const FLASH_LOAN: Symbol = symbol_short!("flash");
const BAD_DEBT: Symbol = symbol_short!("baddebt");
const BAD_DEBT_RECOVER: Symbol = symbol_short!("bdrec");

#[contractevent]
#[derive(Clone, Debug)]
pub struct BorrowEventV1 {
    #[topic]
    pub event: Symbol,
    #[topic]
    pub user: Address,
    pub asset: Address,
    pub amount: i128,
    pub collateral: i128,
    pub timestamp: u64,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct RepayEventV1 {
    #[topic]
    pub event: Symbol,
    #[topic]
    pub user: Address,
    pub asset: Address,
    pub amount: i128,
    pub timestamp: u64,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct DepositEventV1 {
    #[topic]
    pub event: Symbol,
    #[topic]
    pub user: Address,
    pub asset: Address,
    pub amount: i128,
    pub new_balance: i128,
    pub timestamp: u64,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct WithdrawEventV1 {
    #[topic]
    pub event: Symbol,
    #[topic]
    pub user: Address,
    pub asset: Address,
    pub amount: i128,
    pub remaining_balance: i128,
    pub timestamp: u64,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct FlashLoanEventV1 {
    #[topic]
    pub event: Symbol,
    #[topic]
    pub receiver: Address,
    pub asset: Address,
    pub amount: i128,
    pub fee: i128,
    pub timestamp: u64,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct BadDebtEventV1 {
    #[topic]
    pub event: Symbol,
    #[topic]
    pub user: Address,
    pub amount: i128,
    pub timestamp: u64,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct BadDebtRecoveredEventV1 {
    #[topic]
    pub event: Symbol,
    pub amount: i128,
    pub timestamp: u64,
}

pub fn emit_borrow(env: &Env, user: Address, asset: Address, amount: i128, collateral: i128) {
    BorrowEventV1 {
        event: BORROW,
        user,
        asset,
        amount,
        collateral,
        timestamp: env.ledger().timestamp(),
    }
    .publish(env);
}

pub fn emit_repay(env: &Env, user: Address, asset: Address, amount: i128) {
    RepayEventV1 {
        event: REPAY,
        user,
        asset,
        amount,
        timestamp: env.ledger().timestamp(),
    }
    .publish(env);
}

pub fn emit_deposit(env: &Env, user: Address, asset: Address, amount: i128, balance: i128) {
    DepositEventV1 {
        event: DEPOSIT,
        user,
        asset,
        amount,
        new_balance: balance,
        timestamp: env.ledger().timestamp(),
    }
    .publish(env);
}

pub fn emit_withdraw(env: &Env, user: Address, asset: Address, amount: i128, remaining: i128) {
    WithdrawEventV1 {
        event: WITHDRAW,
        user,
        asset,
        amount,
        remaining_balance: remaining,
        timestamp: env.ledger().timestamp(),
    }
    .publish(env);
}

pub fn emit_flash_loan(env: &Env, receiver: Address, asset: Address, amount: i128, fee: i128) {
    FlashLoanEventV1 {
        event: FLASH_LOAN,
        receiver,
        asset,
        amount,
        fee,
        timestamp: env.ledger().timestamp(),
    }
    .publish(env);
}

pub fn emit_bad_debt(env: &Env, user: &Address, amount: i128) {
    BadDebtEventV1 {
        event: BAD_DEBT,
        user: user.clone(),
        amount,
        timestamp: env.ledger().timestamp(),
    }
    .publish(env);
}

pub fn emit_bad_debt_recovered(env: &Env, amount: i128) {
    BadDebtRecoveredEventV1 {
        event: BAD_DEBT_RECOVER,
        amount,
        timestamp: env.ledger().timestamp(),
    }
    .publish(env);
}

pub use borrow::{BorrowCollateral, BorrowError, DebtPosition, StablecoinConfig};
pub use deposit::{DepositCollateral, DepositError};
pub use flash_loan::FlashLoanError;
pub use pause::PauseType;
pub use views::{ProtocolMetrics, ProtocolReport, StablecoinAssetStats, UserPositionSummary};
pub use withdraw::WithdrawError;

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

#[allow(dead_code)]
mod data_store;
pub mod upgrade;

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

    pub fn repay(env: Env, user: Address, asset: Address, amount: i128) -> Result<(), BorrowError> {
        user.require_auth();
        if is_paused(&env, PauseType::Repay) {
            return Err(BorrowError::ProtocolPaused);
        }
        borrow_repay(&env, user, asset, amount)
    }

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

    pub fn liquidate(
        env: Env,
        liquidator: Address,
        _borrower: Address,
        _debt_asset: Address,
        _collateral_asset: Address,
        _amount: i128,
    ) -> Result<(), BorrowError> {
        liquidator.require_auth();
        if is_paused(&env, PauseType::Liquidation) {
            return Err(BorrowError::ProtocolPaused);
        }
        Ok(())
    }

    pub fn get_user_debt(env: Env, user: Address) -> DebtPosition {
        get_borrow_debt(&env, &user)
    }

    pub fn get_user_collateral(env: Env, user: Address) -> BorrowCollateral {
        get_borrow_collateral(&env, &user)
    }

    pub fn get_collateral_balance(env: Env, user: Address) -> i128 {
        view_collateral_balance(&env, &user)
    }

    pub fn get_debt_balance(env: Env, user: Address) -> i128 {
        view_debt_balance(&env, &user)
    }

    pub fn get_collateral_value(env: Env, user: Address) -> i128 {
        view_collateral_value(&env, &user)
    }

    pub fn get_debt_value(env: Env, user: Address) -> i128 {
        view_debt_value(&env, &user)
    }

    pub fn get_health_factor(env: Env, user: Address) -> i128 {
        view_health_factor(&env, &user)
    }

    pub fn get_user_position(env: Env, user: Address) -> UserPositionSummary {
        view_user_position(&env, &user)
    }

    pub fn set_oracle(env: Env, admin: Address, oracle: Address) -> Result<(), BorrowError> {
        set_oracle_logic(&env, &admin, oracle)
    }

    pub fn set_liquidation_threshold_bps(
        env: Env,
        admin: Address,
        bps: i128,
    ) -> Result<(), BorrowError> {
        set_liquidation_threshold_logic(&env, &admin, bps)
    }

    pub fn initialize_deposit_settings(
        env: Env,
        deposit_cap: i128,
        min_deposit_amount: i128,
    ) -> Result<(), DepositError> {
        initialize_deposit_logic(&env, deposit_cap, min_deposit_amount)
    }

    pub fn set_deposit_paused(env: Env, paused: bool) -> Result<(), DepositError> {
        env.storage()
            .persistent()
            .set(&pause::PauseDataKey::State(PauseType::Deposit), &paused);
        Ok(())
    }

    pub fn get_user_collateral_deposit(
        env: Env,
        user: Address,
        asset: Address,
    ) -> DepositCollateral {
        get_deposit_collateral(&env, &user, &asset)
    }

    pub fn get_admin(env: Env) -> Option<Address> {
        get_borrow_admin(&env)
    }

    pub fn flash_loan(
        env: Env,
        receiver: Address,
        asset: Address,
        amount: i128,
        params: Bytes,
    ) -> Result<(), FlashLoanError> {
        flash_loan_logic(&env, receiver, asset, amount, params)
    }

    pub fn set_flash_loan_fee_bps(env: Env, fee_bps: i128) -> Result<(), FlashLoanError> {
        let current_admin = get_borrow_admin(&env).ok_or(FlashLoanError::Unauthorized)?;
        current_admin.require_auth();
        set_flash_loan_fee_logic(&env, fee_bps)
    }

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

    pub fn initialize_withdraw_settings(
        env: Env,
        min_withdraw_amount: i128,
    ) -> Result<(), WithdrawError> {
        initialize_withdraw_logic(&env, min_withdraw_amount)
    }

    pub fn set_withdraw_paused(env: Env, paused: bool) -> Result<(), WithdrawError> {
        set_withdraw_paused_logic(&env, paused)
    }

    pub fn receive(
        env: Env,
        token_asset: Address,
        from: Address,
        amount: i128,
        payload: Vec<Val>,
    ) -> Result<(), BorrowError> {
        receive_logic(env, token_asset, from, amount, payload)
    }

    pub fn initialize_borrow_settings(
        env: Env,
        debt_ceiling: i128,
        min_borrow_amount: i128,
    ) -> Result<(), BorrowError> {
        initialize_borrow_logic(&env, debt_ceiling, min_borrow_amount)
    }

    pub fn set_stablecoin_config(
        env: Env,
        admin: Address,
        asset: Address,
        config: StablecoinConfig,
    ) -> Result<(), BorrowError> {
        set_stablecoin_config_logic(&env, &admin, asset, config)
    }

    pub fn get_stablecoin_config(env: Env, asset: Address) -> Option<StablecoinConfig> {
        get_stablecoin_config_logic(&env, &asset)
    }

    pub fn get_protocol_report(env: Env, stablecoin_assets: Vec<Address>) -> ProtocolReport {
        views::get_protocol_report(&env, stablecoin_assets)
    }
}

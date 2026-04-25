//! Standardized contract events (V1 schema)

pub mod borrow;
mod deposit;
pub mod events;
mod flash_loan;
pub mod invariants;
pub mod pause;
mod token_receiver;
mod withdraw;
use soroban_sdk::{contractevent, Address, Symbol};

// ─────────────────────────────────────────
// Shared constants
// ─────────────────────────────────────────

const BORROW: Symbol = Symbol::short("borrow");
const REPAY: Symbol = Symbol::short("repay");
const DEPOSIT: Symbol = Symbol::short("deposit");
const WITHDRAW: Symbol = Symbol::short("withdraw");
const FLASH_LOAN: Symbol = Symbol::short("flash");
const BAD_DEBT: Symbol = Symbol::short("baddebt");
const BAD_DEBT_RECOVER: Symbol = Symbol::short("bdrec");

// ─────────────────────────────────────────
// Lending Events (STANDARDIZED)
// ─────────────────────────────────────────

#[contractevent]
#[derive(Clone, Debug)]
pub struct BorrowEventV1 {
    #[topic]
    pub event: Symbol, // "borrow"

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
    pub event: Symbol, // "repay"

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
    pub event: Symbol, // "deposit"

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
    pub event: Symbol, // "withdraw"

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
    pub event: Symbol, // "flash"

    #[topic]
    pub receiver: Address,

    pub asset: Address,
    pub amount: i128,
    pub fee: i128,
    pub timestamp: u64,
}

// ─────────────────────────────────────────
// Bad Debt Events (NEW)
// ─────────────────────────────────────────

#[contractevent]
#[derive(Clone, Debug)]
pub struct BadDebtEventV1 {
    #[topic]
    pub event: Symbol, // "baddebt"

    #[topic]
    pub user: Address,

    pub amount: i128,
    pub timestamp: u64,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct BadDebtRecoveredEventV1 {
    #[topic]
    pub event: Symbol, // "bdrec"

    pub amount: i128,
    pub timestamp: u64,
}

use soroban_sdk::Env;

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

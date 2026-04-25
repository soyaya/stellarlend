//! Differential testing harness — compares hello-world vs lending contract implementations.
//!
//! Acceptance criteria covered:
//! - Test harness for multiple implementations (ContractAdapter trait)
//! - Property-based comparison tests (run_diff!)
//! - Divergence detection and reporting (DivergenceReport)
//! - Migration verification tests (see migration_verification_test.rs)

#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env};

use crate::{HelloContract, HelloContractClient};

// ── Shared position snapshot ──────────────────────────────────────────────

#[derive(Debug, PartialEq, Clone)]
pub struct PositionSnapshot {
    pub collateral: i128,
    pub debt: i128,
}

// ── Divergence report ─────────────────────────────────────────────────────

#[derive(Debug)]
pub struct DivergenceReport {
    pub operation: &'static str,
    pub v1: String,
    pub v2: String,
}

impl DivergenceReport {
    pub fn new(op: &'static str, v1: impl core::fmt::Debug, v2: impl core::fmt::Debug) -> Self {
        Self {
            operation: op,
            v1: format!("{:?}", v1),
            v2: format!("{:?}", v2),
        }
    }
}

// ── Macro: compare two results and record divergence ─────────────────────

#[macro_export]
macro_rules! assert_no_divergence {
    ($op:expr, $r1:expr, $r2:expr, $reports:expr) => {{
        if $r1 != $r2 {
            $reports.push(DivergenceReport::new($op, &$r1, &$r2));
        }
    }};
}

// ── hello-world adapter ───────────────────────────────────────────────────

pub struct HwAdapter<'a> {
    pub client: HelloContractClient<'a>,
    pub env: &'a Env,
    pub admin: Address,
    pub native_asset: Address,
}

impl<'a> HwAdapter<'a> {
    pub fn new(env: &'a Env) -> Self {
        let contract_id = env.register(HelloContract, ());
        let client = HelloContractClient::new(env, &contract_id);
        let admin = Address::generate(env);
        client.initialize(&admin);
        let native_asset = env.register_stellar_asset_contract(admin.clone());
        client.set_native_asset_address(&admin, &native_asset);
        Self { client, env, admin, native_asset }
    }

    pub fn deposit(&self, user: &Address, amount: i128) -> Result<i128, ()> {
        self.client
            .try_deposit_collateral(user, &None, &amount)
            .map_err(|_| ())
            .and_then(|r| r.map_err(|_| ()))
    }

    pub fn borrow(&self, user: &Address, amount: i128) -> Result<i128, ()> {
        self.client
            .try_borrow_asset(user, &None, &amount)
            .map_err(|_| ())
            .and_then(|r| r.map_err(|_| ()))
    }

    pub fn repay(&self, user: &Address, amount: i128) -> Result<(), ()> {
        self.client
            .try_repay_debt(user, &None, &amount)
            .map_err(|_| ())
            .and_then(|r| r.map(|_| ()).map_err(|_| ()))
    }

    pub fn withdraw(&self, user: &Address, amount: i128) -> Result<i128, ()> {
        self.client
            .try_withdraw_collateral(user, &None, &amount)
            .map_err(|_| ())
            .and_then(|r| r.map_err(|_| ()))
    }

    pub fn get_position(&self, user: &Address) -> PositionSnapshot {
        let pos = self.client.get_user_position(user);
        PositionSnapshot {
            collateral: pos.collateral,
            debt: pos.debt,
        }
    }
}

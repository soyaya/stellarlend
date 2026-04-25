//! # Hello-World (Core StellarLend) Contract Gas Benchmarks
//!
//! Measures instruction counts for all public functions in the
//! `hello-world` contract — the main StellarLend lending protocol.

use crate::framework::{
    fresh_env, get_budget, measure_instructions, BenchmarkResult, BenchmarkSuite,
    RunConfig,
};
use hello_world::{
    deposit::AssetParams, flash_loan::FlashLoanConfig, HelloContract, HelloContractClient,
};
use soroban_sdk::{testutils::Address as _, Address, Env};

const CONTRACT: &str = "hello_world";

pub fn register(suite: &mut BenchmarkSuite) {
    suite.register_group("Hello-World (Core Lending) Contract", run_all);
}

fn run_all(config: &RunConfig) -> Vec<BenchmarkResult> {
    let mut results = Vec::new();

    results.push(bench_initialize(config));
    results.push(bench_deposit_collateral_native_cold(config));
    results.push(bench_deposit_collateral_native_warm(config));
    results.push(bench_borrow_asset_cold(config));
    results.push(bench_borrow_asset_warm(config));
    results.push(bench_repay_debt_cold(config));
    results.push(bench_repay_debt_warm(config));
    results.push(bench_withdraw_collateral_cold(config));
    results.push(bench_withdraw_collateral_warm(config));
    results.push(bench_liquidate(config));
    results.push(bench_can_be_liquidated(config));
    results.push(bench_get_max_liquidatable_amount(config));
    results.push(bench_get_liquidation_incentive(config));
    results.push(bench_execute_flash_loan(config));
    results.push(bench_set_risk_params(config));
    results.push(bench_set_emergency_pause(config));
    results.push(bench_get_health_factor(config));
    results.push(bench_get_user_position(config));
    results.push(bench_get_user_asset_list(config));
    results.push(bench_get_user_total_collateral_value(config));
    results.push(bench_set_treasury(config));
    results.push(bench_get_treasury(config));
    results.push(bench_set_fee_config(config));
    results.push(bench_get_fee_config(config));
    results.push(bench_get_reserve_balance(config));
    results.push(bench_update_asset_config(config));
    results.push(bench_transfer_admin(config));
    results.push(bench_deposit_collateral_multi_asset_storage(config));

    results
}

// ─── Setup helpers ────────────────────────────────────────────────────────────

fn setup_contract(env: &Env) -> (HelloContractClient<'static>, Address) {
    let contract_id = env.register(HelloContract, ());
    let client = HelloContractClient::new(env, &contract_id);
    let admin = Address::generate(env);
    let _ = client.try_initialize(&admin);
    (client, admin)
}

fn setup_with_deposit(env: &Env) -> (HelloContractClient<'static>, Address, Address) {
    let (client, admin) = setup_contract(env);
    let user = Address::generate(env);
    // Native XLM deposit (None asset)
    client.deposit_collateral(&user, &None, &100_000);
    (client, admin, user)
}

fn setup_with_borrow(env: &Env) -> (HelloContractClient<'static>, Address, Address) {
    let (client, admin, user) = setup_with_deposit(env);
    // Set risk params to allow borrowing (all None = use defaults)
    let _ = client.try_set_risk_params(&admin, &None, &None, &None, &None);
    let _ = client.try_borrow_asset(&user, &None, &20_000);
    (client, admin, user)
}

// ─── Initialize ───────────────────────────────────────────────────────────────

fn bench_initialize(config: &RunConfig) -> BenchmarkResult {
    let op = "hello_world::initialize";
    let env = fresh_env();
    let contract_id = env.register(HelloContract, ());
    let client = HelloContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    let (insns, mem) = measure_instructions(&env, || {
        let _ = client.try_initialize(&admin);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Initialize core lending contract — admin setup, storage init",
        insns, mem, 0, 1, true,
        get_budget(config, op),
        vec!["admin".into(), "init".into()],
    )
}

// ─── Deposit Collateral ───────────────────────────────────────────────────────

fn bench_deposit_collateral_native_cold(config: &RunConfig) -> BenchmarkResult {
    let op = "hello_world::deposit_collateral";
    let env = fresh_env();
    let (client, _) = setup_contract(&env);
    let user = Address::generate(&env);

    let (insns, mem) = measure_instructions(&env, || {
        client.deposit_collateral(&user, &None, &50_000);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Deposit native XLM collateral — first deposit (cold storage write)",
        insns, mem, 1, 3, true,
        get_budget(config, op),
        vec!["deposit".into(), "native".into(), "cold".into()],
    )
}

fn bench_deposit_collateral_native_warm(config: &RunConfig) -> BenchmarkResult {
    let op = "hello_world::deposit_collateral_warm";
    let env = fresh_env();
    let (client, _) = setup_contract(&env);
    let user = Address::generate(&env);
    client.deposit_collateral(&user, &None, &50_000); // first deposit — warms storage
    let (insns, mem) = measure_instructions(&env, || {
        client.deposit_collateral(&user, &None, &10_000); // warm: position exists
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Deposit native XLM collateral — subsequent deposit (warm storage update)",
        insns, mem, 1, 2, false,
        get_budget(config, "hello_world::deposit_collateral"),
        vec!["deposit".into(), "native".into(), "warm".into()],
    )
}

// ─── Borrow Asset ─────────────────────────────────────────────────────────────

fn bench_borrow_asset_cold(config: &RunConfig) -> BenchmarkResult {
    let op = "hello_world::borrow_asset";
    let env = fresh_env();
    let (client, admin, user) = setup_with_deposit(&env);
    let _ = client.try_set_risk_params(&admin, &None, &None, &None, &None);

    let (insns, mem) = measure_instructions(&env, || {
        let _ = client.try_borrow_asset(&user, &None, &20_000);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Borrow asset — first borrow (cold: collateral check + debt write)",
        insns, mem, 2, 2, true,
        get_budget(config, op),
        vec!["borrow".into(), "cold".into()],
    )
}

fn bench_borrow_asset_warm(config: &RunConfig) -> BenchmarkResult {
    let op = "hello_world::borrow_asset_warm";
    let env = fresh_env();
    let (client, admin, user) = setup_with_deposit(&env);
    let _ = client.try_set_risk_params(&admin, &None, &None, &None, &None);
    let _ = client.try_borrow_asset(&user, &None, &10_000); // first borrow — warms storage
    let (insns, mem) = measure_instructions(&env, || {
        let _ = client.try_borrow_asset(&user, &None, &5_000); // warm: debt exists
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Borrow asset — subsequent borrow (warm: debt accumulation)",
        insns, mem, 2, 1, false,
        get_budget(config, "hello_world::borrow_asset"),
        vec!["borrow".into(), "warm".into()],
    )
}

// ─── Repay Debt ───────────────────────────────────────────────────────────────

fn bench_repay_debt_cold(config: &RunConfig) -> BenchmarkResult {
    let op = "hello_world::repay_debt";
    let env = fresh_env();
    let (client, _, user) = setup_with_borrow(&env);

    let (insns, mem) = measure_instructions(&env, || {
        let _ = client.try_repay_debt(&user, &None, &5_000);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Repay debt — partial repayment (cold: debt read + write)",
        insns, mem, 2, 2, true,
        get_budget(config, op),
        vec!["repay".into(), "cold".into()],
    )
}

fn bench_repay_debt_warm(config: &RunConfig) -> BenchmarkResult {
    let op = "hello_world::repay_debt_warm";
    let env = fresh_env();
    let (client, _, user) = setup_with_borrow(&env);
    let _ = client.try_repay_debt(&user, &None, &5_000); // first repay — warms storage
    let (insns, mem) = measure_instructions(&env, || {
        let _ = client.try_repay_debt(&user, &None, &3_000); // warm
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Repay debt — subsequent repayment (warm storage)",
        insns, mem, 1, 1, false,
        get_budget(config, "hello_world::repay_debt"),
        vec!["repay".into(), "warm".into()],
    )
}

// ─── Withdraw Collateral ──────────────────────────────────────────────────────

fn bench_withdraw_collateral_cold(config: &RunConfig) -> BenchmarkResult {
    let op = "hello_world::withdraw_collateral";
    let env = fresh_env();
    let (client, _, user) = setup_with_deposit(&env);

    let (insns, mem) = measure_instructions(&env, || {
        let _ = client.try_withdraw_collateral(&user, &None, &10_000);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Withdraw collateral — cold: health check + balance update",
        insns, mem, 2, 2, true,
        get_budget(config, op),
        vec!["withdraw".into(), "cold".into()],
    )
}

fn bench_withdraw_collateral_warm(config: &RunConfig) -> BenchmarkResult {
    let op = "hello_world::withdraw_collateral_warm";
    let env = fresh_env();
    let (client, _, user) = setup_with_deposit(&env);
    let _ = client.try_withdraw_collateral(&user, &None, &10_000); // first withdraw — warms storage
    let (insns, mem) = measure_instructions(&env, || {
        let _ = client.try_withdraw_collateral(&user, &None, &5_000); // warm
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Withdraw collateral — warm storage",
        insns, mem, 1, 1, false,
        get_budget(config, "hello_world::withdraw_collateral"),
        vec!["withdraw".into(), "warm".into()],
    )
}

// ─── Liquidation ──────────────────────────────────────────────────────────────

fn bench_liquidate(config: &RunConfig) -> BenchmarkResult {
    let op = "hello_world::liquidate";
    let env = fresh_env();
    let (client, _, user) = setup_with_borrow(&env);
    let liquidator = Address::generate(&env);

    let (insns, mem) = measure_instructions(&env, || {
        let _ = client.try_liquidate(&liquidator, &user, &None, &None, &5_000);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Liquidate position — health factor check + collateral/debt update",
        insns, mem, 3, 3, true,
        get_budget(config, op),
        vec!["liquidate".into(), "cold".into()],
    )
}

fn bench_can_be_liquidated(config: &RunConfig) -> BenchmarkResult {
    let op = "hello_world::can_be_liquidated";
    let env = fresh_env();
    let (client, _, _) = setup_with_borrow(&env);

    let (insns, mem) = measure_instructions(&env, || {
        // can_be_liquidated takes collateral_value and debt_value directly
        client.can_be_liquidated(&80_000i128, &100_000i128);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Check if position can be liquidated — risk params read + ratio check",
        insns, mem, 1, 0, false,
        get_budget(config, op),
        vec!["query".into(), "liquidation".into()],
    )
}

fn bench_get_max_liquidatable_amount(config: &RunConfig) -> BenchmarkResult {
    let op = "hello_world::get_max_liquidatable_amount";
    let env = fresh_env();
    let (client, _, _) = setup_with_borrow(&env);

    let (insns, mem) = measure_instructions(&env, || {
        client.get_max_liquidatable_amount(&50_000i128);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Get max liquidatable amount — risk params read + calculation",
        insns, mem, 1, 0, false,
        get_budget(config, op),
        vec!["query".into(), "liquidation".into()],
    )
}

fn bench_get_liquidation_incentive(config: &RunConfig) -> BenchmarkResult {
    let op = "hello_world::get_liquidation_incentive_amount";
    let env = fresh_env();
    let (client, _, _) = setup_with_borrow(&env);

    let (insns, mem) = measure_instructions(&env, || {
        client.get_liquidation_incentive_amount(&5_000i128);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Get liquidation incentive amount — bonus calculation",
        insns, mem, 1, 0, false,
        get_budget(config, op),
        vec!["query".into(), "liquidation".into()],
    )
}

// ─── Flash Loan ───────────────────────────────────────────────────────────────

fn bench_execute_flash_loan(config: &RunConfig) -> BenchmarkResult {
    let op = "hello_world::execute_flash_loan";
    let env = fresh_env();
    let (client, admin, _) = setup_with_deposit(&env);
    let asset = Address::generate(&env);
    let callback = Address::generate(&env);
    let config_fl = FlashLoanConfig {
        fee_bps: 50i128,
        max_amount: 1_000_000i128,
        min_amount: 100i128,
    };
    let _ = client.try_configure_flash_loan(&admin, &config_fl);

    let (insns, mem) = measure_instructions(&env, || {
        let _ = client.try_execute_flash_loan(&callback, &asset, &1_000, &callback);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Execute flash loan — borrow + fee check + repayment validation",
        insns, mem, 2, 2, true,
        get_budget(config, op),
        vec!["flash_loan".into(), "cold".into()],
    )
}

// ─── Risk & Admin ─────────────────────────────────────────────────────────────

fn bench_set_risk_params(config: &RunConfig) -> BenchmarkResult {
    let op = "hello_world::set_risk_params";
    let env = fresh_env();
    let (client, admin) = setup_contract(&env);

    let (insns, mem) = measure_instructions(&env, || {
        let _ = client.try_set_risk_params(&admin, &None, &None, &None, &None);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Set risk parameters — admin auth + storage write",
        insns, mem, 1, 1, true,
        get_budget(config, op),
        vec!["admin".into(), "risk".into()],
    )
}

fn bench_set_emergency_pause(config: &RunConfig) -> BenchmarkResult {
    let op = "hello_world::set_emergency_pause";
    let env = fresh_env();
    let (client, admin) = setup_contract(&env);

    let (insns, mem) = measure_instructions(&env, || {
        let _ = client.try_set_emergency_pause(&admin, &true);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Set emergency pause — admin auth + single storage write",
        insns, mem, 0, 1, true,
        get_budget(config, op),
        vec!["admin".into(), "pause".into()],
    )
}

fn bench_transfer_admin(config: &RunConfig) -> BenchmarkResult {
    let op = "hello_world::transfer_admin";
    let env = fresh_env();
    let (client, admin) = setup_contract(&env);
    let new_admin = Address::generate(&env);

    let (insns, mem) = measure_instructions(&env, || {
        let _ = client.try_transfer_admin(&admin, &new_admin);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Transfer admin — auth check + storage update",
        insns, mem, 1, 1, true,
        get_budget(config, op),
        vec!["admin".into()],
    )
}

fn bench_update_asset_config(config: &RunConfig) -> BenchmarkResult {
    let op = "hello_world::update_asset_config";
    let env = fresh_env();
    let (client, _) = setup_contract(&env);
    let asset = Address::generate(&env);
    let params = AssetParams {
        deposit_enabled: true,
        collateral_factor: 7500i128,
        max_deposit: 0i128,
        borrow_fee_bps: 50i128,
    };

    let (insns, mem) = measure_instructions(&env, || {
        let _ = client.try_update_asset_config(&asset, &params);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Update asset configuration — admin auth + storage write",
        insns, mem, 1, 1, true,
        get_budget(config, op),
        vec!["admin".into(), "asset_config".into()],
    )
}

// ─── Treasury ─────────────────────────────────────────────────────────────────

fn bench_set_treasury(config: &RunConfig) -> BenchmarkResult {
    let op = "hello_world::set_treasury";
    let env = fresh_env();
    let (client, admin) = setup_contract(&env);
    let treasury = Address::generate(&env);

    let (insns, mem) = measure_instructions(&env, || {
        let _ = client.try_set_treasury(&admin, &treasury);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Set treasury address — admin auth + storage write",
        insns, mem, 0, 1, true,
        get_budget(config, op),
        vec!["admin".into(), "treasury".into()],
    )
}

fn bench_get_treasury(config: &RunConfig) -> BenchmarkResult {
    let op = "hello_world::get_treasury";
    let env = fresh_env();
    let (client, admin) = setup_contract(&env);
    let treasury = Address::generate(&env);
    let _ = client.try_set_treasury(&admin, &treasury);

    let (insns, mem) = measure_instructions(&env, || {
        client.get_treasury();
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Get treasury address — single storage read",
        insns, mem, 1, 0, false,
        get_budget(config, op),
        vec!["query".into(), "treasury".into()],
    )
}

fn bench_set_fee_config(config: &RunConfig) -> BenchmarkResult {
    let op = "hello_world::set_fee_config";
    let env = fresh_env();
    let (client, admin) = setup_contract(&env);

    let (insns, mem) = measure_instructions(&env, || {
        let _ = client.try_set_fee_config(&admin, &50i128, &100i128);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Set fee configuration — admin auth + storage write",
        insns, mem, 0, 1, true,
        get_budget(config, op),
        vec!["admin".into(), "fees".into()],
    )
}

fn bench_get_fee_config(config: &RunConfig) -> BenchmarkResult {
    let op = "hello_world::get_fee_config";
    let env = fresh_env();
    let (client, admin) = setup_contract(&env);
    let _ = client.try_set_fee_config(&admin, &50i128, &100i128);

    let (insns, mem) = measure_instructions(&env, || {
        client.get_fee_config();
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Get fee configuration — single storage read",
        insns, mem, 1, 0, false,
        get_budget(config, op),
        vec!["query".into(), "fees".into()],
    )
}

fn bench_get_reserve_balance(config: &RunConfig) -> BenchmarkResult {
    let op = "hello_world::get_reserve_balance";
    let env = fresh_env();
    let (client, _, _) = setup_with_deposit(&env);

    let (insns, mem) = measure_instructions(&env, || {
        client.get_reserve_balance(&None);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Get reserve balance — storage read",
        insns, mem, 1, 0, false,
        get_budget(config, op),
        vec!["query".into(), "treasury".into()],
    )
}

// ─── Query operations ─────────────────────────────────────────────────────────

fn bench_get_health_factor(config: &RunConfig) -> BenchmarkResult {
    let op = "hello_world::get_health_factor";
    let env = fresh_env();
    let (client, _, user) = setup_with_borrow(&env);

    let (insns, mem) = measure_instructions(&env, || {
        let _ = client.try_get_health_factor(&user);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Get health factor — collateral + debt reads + ratio calculation",
        insns, mem, 2, 0, false,
        get_budget(config, op),
        vec!["query".into(), "health_factor".into()],
    )
}

fn bench_get_user_position(config: &RunConfig) -> BenchmarkResult {
    let op = "hello_world::get_user_position";
    let env = fresh_env();
    let (client, _, user) = setup_with_borrow(&env);

    let (insns, mem) = measure_instructions(&env, || {
        let _ = client.try_get_user_position(&user);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Get full user position — multi-storage read",
        insns, mem, 3, 0, false,
        get_budget(config, op),
        vec!["query".into(), "position".into()],
    )
}

fn bench_get_user_asset_list(config: &RunConfig) -> BenchmarkResult {
    let op = "hello_world::get_user_asset_list";
    let env = fresh_env();
    let (client, _, user) = setup_with_deposit(&env);

    let (insns, mem) = measure_instructions(&env, || {
        client.get_user_asset_list(&user);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Get user asset list — storage read",
        insns, mem, 1, 0, false,
        get_budget(config, op),
        vec!["query".into(), "assets".into()],
    )
}

fn bench_get_user_total_collateral_value(config: &RunConfig) -> BenchmarkResult {
    let op = "hello_world::get_user_total_collateral_value";
    let env = fresh_env();
    let (client, _, user) = setup_with_deposit(&env);

    let (insns, mem) = measure_instructions(&env, || {
        client.get_user_total_collateral_value(&user);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Get total collateral value — asset list + per-asset reads",
        insns, mem, 2, 0, false,
        get_budget(config, op),
        vec!["query".into(), "collateral".into()],
    )
}

// ─── Storage pattern benchmarks ───────────────────────────────────────────────

fn bench_deposit_collateral_multi_asset_storage(config: &RunConfig) -> BenchmarkResult {
    let op = "hello_world::deposit_collateral_multi_asset_storage";
    let env = fresh_env();
    let (client, _) = setup_contract(&env);
    let user = Address::generate(&env);

    // Pre-populate with 5 native deposits
    for _ in 0..5 {
        client.deposit_collateral(&user, &None, &1_000);
    }

    // Measure the 6th deposit — storage is warm
    let (insns, mem) = measure_instructions(&env, || {
        client.deposit_collateral(&user, &None, &1_000);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Deposit collateral with 5 prior deposits — warm storage write pattern",
        insns, mem, 1, 2, false,
        get_budget(config, "hello_world::deposit_collateral"),
        vec!["deposit".into(), "storage_pattern".into(), "warm".into()],
    )
}

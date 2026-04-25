//! # AMM Contract Gas Benchmarks
//!
//! Measures instruction counts for all public functions in the
//! `stellarlend-amm` contract.

use crate::framework::{
    fresh_env, get_budget, measure_instructions, BenchmarkResult, BenchmarkSuite,
    RunConfig,
};
use soroban_sdk::{symbol_short, testutils::Address as _, vec as soroban_vec, Address, Env};
use stellarlend_amm::{
    AmmCallbackData, AmmContract, AmmContractClient, AmmProtocolConfig, AmmSettings,
    LiquidityParams, SwapParams, TokenPair,
};

const CONTRACT: &str = "amm";

pub fn register(suite: &mut BenchmarkSuite) {
    suite.register_group("AMM Contract", run_all);
}

fn run_all(config: &RunConfig) -> Vec<BenchmarkResult> {
    let mut results = Vec::new();

    results.push(bench_initialize_amm_settings(config));
    results.push(bench_add_amm_protocol(config));
    results.push(bench_update_amm_settings(config));
    results.push(bench_execute_swap_cold(config));
    results.push(bench_execute_swap_warm(config));
    results.push(bench_add_liquidity_cold(config));
    results.push(bench_add_liquidity_warm(config));
    results.push(bench_remove_liquidity(config));
    results.push(bench_auto_swap_for_collateral(config));
    results.push(bench_validate_amm_callback(config));
    results.push(bench_get_amm_settings(config));
    results.push(bench_get_amm_protocols(config));
    results.push(bench_get_swap_history_empty(config));
    results.push(bench_get_swap_history_populated(config));
    results.push(bench_get_liquidity_history(config));

    results
}

// ─── Setup helpers ────────────────────────────────────────────────────────────

fn default_protocol_config(env: &Env, protocol_addr: &Address) -> AmmProtocolConfig {
    let pool = Address::generate(env);
    AmmProtocolConfig {
        protocol_address: protocol_addr.clone(),
        protocol_name: symbol_short!("testamm"),
        enabled: true,
        fee_tier: 30i128,
        min_swap_amount: 100i128,
        max_swap_amount: 1_000_000_000i128,
        supported_pairs: soroban_vec![
            env,
            TokenPair {
                token_a: None,
                token_b: None,
                pool_address: pool,
            }
        ],
    }
}

fn setup_initialized(env: &Env) -> (AmmContractClient<'static>, Address) {
    let contract_id = env.register(AmmContract, ());
    let client = AmmContractClient::new(env, &contract_id);
    let admin = Address::generate(env);
    client
        .initialize_amm_settings(&admin, &100i128, &500i128, &1_000i128)
        ;
    (client, admin)
}

fn setup_with_protocol(env: &Env) -> (AmmContractClient<'static>, Address, Address) {
    let (client, admin) = setup_initialized(env);
    let protocol = Address::generate(env);
    let config = default_protocol_config(env, &protocol);
    client.add_amm_protocol(&admin, &config);
    (client, admin, protocol)
}

fn default_swap_params(env: &Env, protocol: &Address) -> SwapParams {
    SwapParams {
        protocol: protocol.clone(),
        token_in: None,
        token_out: None,
        amount_in: 10_000i128,
        min_amount_out: 9_000i128,
        slippage_tolerance: 100i128,
        deadline: env.ledger().timestamp() + 3600,
    }
}

fn default_liquidity_params(env: &Env, protocol: &Address) -> LiquidityParams {
    LiquidityParams {
        protocol: protocol.clone(),
        token_a: None,
        token_b: None,
        amount_a: 50_000i128,
        amount_b: 50_000i128,
        min_amount_a: 0i128,
        min_amount_b: 0i128,
        deadline: env.ledger().timestamp() + 3600,
    }
}

// ─── Initialize ───────────────────────────────────────────────────────────────

fn bench_initialize_amm_settings(config: &RunConfig) -> BenchmarkResult {
    let op = "amm::initialize_amm_settings";
    let env = fresh_env();
    let contract_id = env.register(AmmContract, ());
    let client = AmmContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    let (insns, mem) = measure_instructions(&env, || {
        client
            .initialize_amm_settings(&admin, &100i128, &500i128, &1_000i128)
            ;
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Initialize AMM settings — admin setup + storage write",
        insns, mem, 0, 1, true,
        get_budget(config, op),
        vec!["admin".into(), "init".into()],
    )
}

fn bench_add_amm_protocol(config: &RunConfig) -> BenchmarkResult {
    let op = "amm::add_amm_protocol";
    let env = fresh_env();
    let (client, admin) = setup_initialized(&env);
    let protocol = Address::generate(&env);
    let protocol_config = default_protocol_config(&env, &protocol);

    let (insns, mem) = measure_instructions(&env, || {
        client.add_amm_protocol(&admin, &protocol_config);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Register AMM protocol — admin auth + protocol map write",
        insns, mem, 1, 1, true,
        get_budget(config, op),
        vec!["admin".into(), "protocol".into()],
    )
}

fn bench_update_amm_settings(config: &RunConfig) -> BenchmarkResult {
    let op = "amm::update_amm_settings";
    let env = fresh_env();
    let (client, admin) = setup_initialized(&env);
    let new_settings = AmmSettings {
        default_slippage: 150i128,
        max_slippage: 600i128,
        swap_enabled: true,
        liquidity_enabled: true,
        auto_swap_threshold: 2_000i128,
    };

    let (insns, mem) = measure_instructions(&env, || {
        client.update_amm_settings(&admin, &new_settings);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Update AMM settings — admin auth + settings overwrite",
        insns, mem, 1, 1, false,
        get_budget(config, op),
        vec!["admin".into(), "settings".into()],
    )
}

// ─── Execute Swap ─────────────────────────────────────────────────────────────

fn bench_execute_swap_cold(config: &RunConfig) -> BenchmarkResult {
    let op = "amm::execute_swap";
    let env = fresh_env();
    let (client, _, protocol) = setup_with_protocol(&env);
    let user = Address::generate(&env);
    let params = default_swap_params(&env, &protocol);

    let (insns, mem) = measure_instructions(&env, || {
        let _ = client.execute_swap(&user, &params);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Execute token swap — cold: protocol lookup + slippage check + history write",
        insns, mem, 2, 2, true,
        get_budget(config, op),
        vec!["swap".into(), "cold".into()],
    )
}

fn bench_execute_swap_warm(config: &RunConfig) -> BenchmarkResult {
    let op = "amm::execute_swap_warm";
    let env = fresh_env();
    let (client, _, protocol) = setup_with_protocol(&env);
    let user = Address::generate(&env);
    let params = default_swap_params(&env, &protocol);
    let _ = client.execute_swap(&user, &params); // first swap — warms history storage
    let (insns, mem) = measure_instructions(&env, || {
        let _ = client.execute_swap(&user, &params); // warm: history exists
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Execute token swap — warm: history append (existing swap records)",
        insns, mem, 1, 1, false,
        get_budget(config, "amm::execute_swap"),
        vec!["swap".into(), "warm".into()],
    )
}

// ─── Add Liquidity ────────────────────────────────────────────────────────────

fn bench_add_liquidity_cold(config: &RunConfig) -> BenchmarkResult {
    let op = "amm::add_liquidity";
    let env = fresh_env();
    let (client, _, protocol) = setup_with_protocol(&env);
    let user = Address::generate(&env);
    let params = default_liquidity_params(&env, &protocol);

    let (insns, mem) = measure_instructions(&env, || {
        let _ = client.add_liquidity(&user, &params);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Add liquidity — cold: protocol lookup + LP token calc + history write",
        insns, mem, 2, 2, true,
        get_budget(config, op),
        vec!["liquidity".into(), "cold".into()],
    )
}

fn bench_add_liquidity_warm(config: &RunConfig) -> BenchmarkResult {
    let op = "amm::add_liquidity_warm";
    let env = fresh_env();
    let (client, _, protocol) = setup_with_protocol(&env);
    let user = Address::generate(&env);
    let params = default_liquidity_params(&env, &protocol);
    let _ = client.add_liquidity(&user, &params); // first add — warms history storage
    let (insns, mem) = measure_instructions(&env, || {
        let _ = client.add_liquidity(&user, &params); // warm: history exists
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Add liquidity — warm: history append",
        insns, mem, 1, 1, false,
        get_budget(config, "amm::add_liquidity"),
        vec!["liquidity".into(), "warm".into()],
    )
}

// ─── Remove Liquidity ─────────────────────────────────────────────────────────

fn bench_remove_liquidity(config: &RunConfig) -> BenchmarkResult {
    let op = "amm::remove_liquidity";
    let env = fresh_env();
    let (client, _, protocol) = setup_with_protocol(&env);
    let user = Address::generate(&env);
    // Add liquidity first
    let lp_params = default_liquidity_params(&env, &protocol);
    let _ = client.add_liquidity(&user, &lp_params);

    let deadline = env.ledger().timestamp() + 3600;
    let (insns, mem) = measure_instructions(&env, || {
        let _ = client.remove_liquidity(
            &user,
            &protocol,
            &None,
            &None,
            &1_000i128,
            &0i128,
            &0i128,
            &deadline,
        );
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Remove liquidity — LP token burn + underlying token return + history write",
        insns, mem, 2, 2, false,
        get_budget(config, op),
        vec!["liquidity".into(), "remove".into()],
    )
}

// ─── Auto Swap ────────────────────────────────────────────────────────────────

fn bench_auto_swap_for_collateral(config: &RunConfig) -> BenchmarkResult {
    let op = "amm::auto_swap_for_collateral";
    let env = fresh_env();
    let (client, _, _) = setup_with_protocol(&env);
    let user = Address::generate(&env);

    let (insns, mem) = measure_instructions(&env, || {
        let _ = client.auto_swap_for_collateral(&user, &None, &5_000i128);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Auto-swap for collateral optimization — threshold check + best protocol selection",
        insns, mem, 2, 1, true,
        get_budget(config, op),
        vec!["swap".into(), "collateral".into()],
    )
}

// ─── Callback Validation ──────────────────────────────────────────────────────

fn bench_validate_amm_callback(config: &RunConfig) -> BenchmarkResult {
    let op = "amm::validate_amm_callback";
    let env = fresh_env();
    let (client, _, protocol) = setup_with_protocol(&env);
    let user = Address::generate(&env);

    let callback_data = AmmCallbackData {
        nonce: 1u64,
        operation: symbol_short!("swap"),
        user: user.clone(),
        expected_amounts: soroban_vec![&env, 9_000i128],
        deadline: env.ledger().timestamp() + 3600,
    };

    let (insns, mem) = measure_instructions(&env, || {
        let _ = client.validate_amm_callback(&protocol, &callback_data);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Validate AMM callback — nonce check + replay protection write",
        insns, mem, 1, 1, true,
        get_budget(config, op),
        vec!["callback".into(), "security".into()],
    )
}

// ─── Query operations ─────────────────────────────────────────────────────────

fn bench_get_amm_settings(config: &RunConfig) -> BenchmarkResult {
    let op = "amm::get_amm_settings";
    let env = fresh_env();
    let (client, _) = setup_initialized(&env);

    let (insns, mem) = measure_instructions(&env, || {
        client.get_amm_settings();
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Get AMM settings — single storage read",
        insns, mem, 1, 0, false,
        get_budget(config, op),
        vec!["query".into(), "settings".into()],
    )
}

fn bench_get_amm_protocols(config: &RunConfig) -> BenchmarkResult {
    let op = "amm::get_amm_protocols";
    let env = fresh_env();
    let (client, _, _) = setup_with_protocol(&env);

    let (insns, mem) = measure_instructions(&env, || {
        client.get_amm_protocols();
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Get all AMM protocols — protocol map read",
        insns, mem, 1, 0, false,
        get_budget(config, op),
        vec!["query".into(), "protocols".into()],
    )
}

fn bench_get_swap_history_empty(config: &RunConfig) -> BenchmarkResult {
    let op = "amm::get_swap_history_empty";
    let env = fresh_env();
    let (client, _) = setup_initialized(&env);

    let (insns, mem) = measure_instructions(&env, || {
        client.get_swap_history(&None, &10u32);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Get swap history — empty history (cold read miss)",
        insns, mem, 1, 0, true,
        get_budget(config, "amm::get_swap_history"),
        vec!["query".into(), "history".into(), "empty".into()],
    )
}

fn bench_get_swap_history_populated(config: &RunConfig) -> BenchmarkResult {
    let op = "amm::get_swap_history_populated";
    let env = fresh_env();
    let (client, _, protocol) = setup_with_protocol(&env);
    let user = Address::generate(&env);
    // Populate with 5 swaps
    for _ in 0..5 {
        let params = default_swap_params(&env, &protocol);
        let _ = client.execute_swap(&user, &params);
    }

    let (insns, mem) = measure_instructions(&env, || {
        client.get_swap_history(&Some(user.clone()), &10u32);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Get swap history — 5 records (warm read, deserialization cost)",
        insns, mem, 1, 0, false,
        get_budget(config, "amm::get_swap_history"),
        vec!["query".into(), "history".into(), "populated".into()],
    )
}

fn bench_get_liquidity_history(config: &RunConfig) -> BenchmarkResult {
    let op = "amm::get_liquidity_history";
    let env = fresh_env();
    let (client, _, protocol) = setup_with_protocol(&env);
    let user = Address::generate(&env);
    let params = default_liquidity_params(&env, &protocol);
    let _ = client.add_liquidity(&user, &params);

    let (insns, mem) = measure_instructions(&env, || {
        client.get_liquidity_history(&Some(user.clone()), &10u32);
    });

    BenchmarkResult::new(
        op, CONTRACT,
        "Get liquidity history — 1 record",
        insns, mem, 1, 0, false,
        get_budget(config, op),
        vec!["query".into(), "history".into()],
    )
}

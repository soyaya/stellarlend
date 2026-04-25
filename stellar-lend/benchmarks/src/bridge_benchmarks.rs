//! # Bridge Contract Gas Benchmarks
//!
//! Measures instruction counts for all public functions in the
//! `bridge` contract:
//! - init, register_bridge, set_bridge_fee, set_bridge_active
//! - bridge_deposit, bridge_withdraw
//! - transfer_admin, get_bridge_config, list_bridges, get_admin, compute_fee
//!
//! Also covers storage write patterns:
//! - Cold: first bridge registration
//! - Warm: subsequent operations on existing bridge

use crate::framework::{
    fresh_env, get_budget, measure_instructions, BenchmarkResult, BenchmarkSuite, RunConfig,
};
use bridge::{BridgeContract, BridgeContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

const CONTRACT: &str = "bridge";

pub fn register(suite: &mut BenchmarkSuite) {
    suite.register_group("Bridge Contract", run_all);
}

fn run_all(config: &RunConfig) -> Vec<BenchmarkResult> {
    vec![
        bench_init(config),
        bench_register_bridge_cold(config),
        bench_register_bridge_warm(config),
        bench_set_bridge_fee(config),
        bench_set_bridge_active(config),
        bench_bridge_deposit_cold(config),
        bench_bridge_deposit_warm(config),
        bench_bridge_withdraw_cold(config),
        bench_bridge_withdraw_warm(config),
        bench_transfer_admin(config),
        bench_get_bridge_config(config),
        bench_list_bridges_empty(config),
        bench_list_bridges_populated(config),
        bench_get_admin(config),
        bench_compute_fee_normal(config),
        bench_compute_fee_zero_rate(config),
        bench_bridge_deposit_multiple_bridges_storage(config),
    ]
}

// ─── Setup helpers ────────────────────────────────────────────────────────────

fn s(env: &Env, v: &str) -> String {
    String::from_str(env, v)
}

fn setup(env: &Env) -> (BridgeContractClient<'static>, Address) {
    let contract_id = env.register(BridgeContract, ());
    let client = BridgeContractClient::new(env, &contract_id);
    let admin = Address::generate(env);
    client.init(&admin);
    (client, admin)
}

fn setup_with_bridge(env: &Env) -> (BridgeContractClient<'static>, Address) {
    let (client, admin) = setup(env);
    client.register_bridge(&admin, &s(env, "eth-mainnet"), &50u64, &1_000i128);
    (client, admin)
}

// ─── Init ─────────────────────────────────────────────────────────────────────

fn bench_init(config: &RunConfig) -> BenchmarkResult {
    let op = "bridge::init";
    let env = fresh_env();
    let contract_id = env.register(BridgeContract, ());
    let client = BridgeContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    let (insns, mem) = measure_instructions(&env, || {
        client.init(&admin);
    });

    BenchmarkResult::new(
        op,
        CONTRACT,
        "Initialize bridge contract — admin storage write",
        insns,
        mem,
        0,
        1,
        true,
        get_budget(config, op),
        vec!["admin".into(), "init".into()],
    )
}

// ─── Register Bridge ──────────────────────────────────────────────────────────

fn bench_register_bridge_cold(config: &RunConfig) -> BenchmarkResult {
    let op = "bridge::register_bridge";
    let env = fresh_env();
    let (client, admin) = setup(&env);

    let (insns, mem) = measure_instructions(&env, || {
        client.register_bridge(&admin, &s(&env, "eth-mainnet"), &50u64, &1_000i128);
    });

    BenchmarkResult::new(
        op,
        CONTRACT,
        "Register bridge — cold: bridge list init + config write",
        insns,
        mem,
        1,
        2,
        true,
        get_budget(config, op),
        vec!["admin".into(), "register".into(), "cold".into()],
    )
}

fn bench_register_bridge_warm(config: &RunConfig) -> BenchmarkResult {
    let op = "bridge::register_bridge_warm";
    let env = fresh_env();
    let (client, admin) = setup(&env);
    client.register_bridge(&admin, &s(&env, "eth-mainnet"), &50u64, &1_000i128);
    // Measure second registration — bridge list already exists (warm)
    let (insns, mem) = measure_instructions(&env, || {
        client.register_bridge(&admin, &s(&env, "polygon"), &30u64, &500i128);
    });

    BenchmarkResult::new(
        op,
        CONTRACT,
        "Register bridge — warm: bridge list append (existing list)",
        insns,
        mem,
        1,
        1,
        false,
        get_budget(config, "bridge::register_bridge"),
        vec!["admin".into(), "register".into(), "warm".into()],
    )
}

// ─── Set Bridge Fee ───────────────────────────────────────────────────────────

fn bench_set_bridge_fee(config: &RunConfig) -> BenchmarkResult {
    let op = "bridge::set_bridge_fee";
    let env = fresh_env();
    let (client, admin) = setup_with_bridge(&env);

    let (insns, mem) = measure_instructions(&env, || {
        client.set_bridge_fee(&admin, &s(&env, "eth-mainnet"), &100u64);
    });

    BenchmarkResult::new(
        op,
        CONTRACT,
        "Set bridge fee — admin auth + bridge config read + write",
        insns,
        mem,
        1,
        1,
        false,
        get_budget(config, op),
        vec!["admin".into(), "fee".into()],
    )
}

// ─── Set Bridge Active ────────────────────────────────────────────────────────

fn bench_set_bridge_active(config: &RunConfig) -> BenchmarkResult {
    let op = "bridge::set_bridge_active";
    let env = fresh_env();
    let (client, admin) = setup_with_bridge(&env);

    let (insns, mem) = measure_instructions(&env, || {
        client.set_bridge_active(&admin, &s(&env, "eth-mainnet"), &false);
    });

    BenchmarkResult::new(
        op,
        CONTRACT,
        "Set bridge active/inactive — admin auth + config update",
        insns,
        mem,
        1,
        1,
        false,
        get_budget(config, op),
        vec!["admin".into(), "active".into()],
    )
}

// ─── Bridge Deposit ───────────────────────────────────────────────────────────

fn bench_bridge_deposit_cold(config: &RunConfig) -> BenchmarkResult {
    let op = "bridge::bridge_deposit";
    let env = fresh_env();
    let (client, _) = setup_with_bridge(&env);
    let user = Address::generate(&env);

    let (insns, mem) = measure_instructions(&env, || {
        client.bridge_deposit(&user, &s(&env, "eth-mainnet"), &100_000i128);
    });

    BenchmarkResult::new(
        op,
        CONTRACT,
        "Bridge deposit — cold: bridge config read + fee calc + total_deposited write",
        insns,
        mem,
        1,
        1,
        true,
        get_budget(config, op),
        vec!["deposit".into(), "cold".into()],
    )
}

fn bench_bridge_deposit_warm(config: &RunConfig) -> BenchmarkResult {
    let op = "bridge::bridge_deposit_warm";
    let env = fresh_env();
    let (client, _) = setup_with_bridge(&env);
    let user = Address::generate(&env);
    client.bridge_deposit(&user, &s(&env, "eth-mainnet"), &100_000i128); // first deposit — warms storage
    let (insns, mem) = measure_instructions(&env, || {
        client.bridge_deposit(&user, &s(&env, "eth-mainnet"), &50_000i128); // warm
    });

    BenchmarkResult::new(
        op,
        CONTRACT,
        "Bridge deposit — warm: bridge config cached, total_deposited accumulation",
        insns,
        mem,
        1,
        1,
        false,
        get_budget(config, "bridge::bridge_deposit"),
        vec!["deposit".into(), "warm".into()],
    )
}

// ─── Bridge Withdraw ──────────────────────────────────────────────────────────

fn bench_bridge_withdraw_cold(config: &RunConfig) -> BenchmarkResult {
    let op = "bridge::bridge_withdraw";
    let env = fresh_env();
    let (client, admin) = setup_with_bridge(&env);
    let recipient = Address::generate(&env);

    let (insns, mem) = measure_instructions(&env, || {
        client.bridge_withdraw(&admin, &s(&env, "eth-mainnet"), &recipient, &10_000i128);
    });

    BenchmarkResult::new(
        op,
        CONTRACT,
        "Bridge withdraw — cold: admin auth + bridge config read + total_withdrawn write",
        insns,
        mem,
        1,
        1,
        true,
        get_budget(config, op),
        vec!["withdraw".into(), "cold".into()],
    )
}

fn bench_bridge_withdraw_warm(config: &RunConfig) -> BenchmarkResult {
    let op = "bridge::bridge_withdraw_warm";
    let env = fresh_env();
    let (client, admin) = setup_with_bridge(&env);
    let recipient = Address::generate(&env);
    client.bridge_withdraw(&admin, &s(&env, "eth-mainnet"), &recipient, &10_000i128); // first withdraw — warms storage
    let (insns, mem) = measure_instructions(&env, || {
        client.bridge_withdraw(&admin, &s(&env, "eth-mainnet"), &recipient, &5_000i128);
        // warm
    });

    BenchmarkResult::new(
        op,
        CONTRACT,
        "Bridge withdraw — warm: bridge config cached",
        insns,
        mem,
        1,
        1,
        false,
        get_budget(config, "bridge::bridge_withdraw"),
        vec!["withdraw".into(), "warm".into()],
    )
}

// ─── Admin operations ─────────────────────────────────────────────────────────

fn bench_transfer_admin(config: &RunConfig) -> BenchmarkResult {
    let op = "bridge::transfer_admin";
    let env = fresh_env();
    let (client, admin) = setup(&env);
    let new_admin = Address::generate(&env);

    let (insns, mem) = measure_instructions(&env, || {
        client.transfer_admin(&admin, &new_admin);
    });

    BenchmarkResult::new(
        op,
        CONTRACT,
        "Transfer admin — auth check + admin storage overwrite",
        insns,
        mem,
        1,
        1,
        false,
        get_budget(config, op),
        vec!["admin".into()],
    )
}

// ─── Query operations ─────────────────────────────────────────────────────────

fn bench_get_bridge_config(config: &RunConfig) -> BenchmarkResult {
    let op = "bridge::get_bridge_config";
    let env = fresh_env();
    let (client, _) = setup_with_bridge(&env);

    let (insns, mem) = measure_instructions(&env, || {
        client.get_bridge_config(&s(&env, "eth-mainnet"));
    });

    BenchmarkResult::new(
        op,
        CONTRACT,
        "Get bridge config — single storage read",
        insns,
        mem,
        1,
        0,
        false,
        get_budget(config, op),
        vec!["query".into(), "config".into()],
    )
}

fn bench_list_bridges_empty(config: &RunConfig) -> BenchmarkResult {
    let op = "bridge::list_bridges_empty";
    let env = fresh_env();
    let (client, _) = setup(&env);

    let (insns, mem) = measure_instructions(&env, || {
        client.list_bridges();
    });

    BenchmarkResult::new(
        op,
        CONTRACT,
        "List bridges — empty list (cold read miss)",
        insns,
        mem,
        1,
        0,
        true,
        get_budget(config, "bridge::list_bridges"),
        vec!["query".into(), "list".into(), "empty".into()],
    )
}

fn bench_list_bridges_populated(config: &RunConfig) -> BenchmarkResult {
    let op = "bridge::list_bridges_populated";
    let env = fresh_env();
    let (client, admin) = setup(&env);
    // Register 3 bridges
    client.register_bridge(&admin, &s(&env, "eth-mainnet"), &50u64, &1_000i128);
    client.register_bridge(&admin, &s(&env, "polygon"), &30u64, &500i128);
    client.register_bridge(&admin, &s(&env, "bsc"), &20u64, &200i128);

    let (insns, mem) = measure_instructions(&env, || {
        client.list_bridges();
    });

    BenchmarkResult::new(
        op,
        CONTRACT,
        "List bridges — 3 bridges (warm read, Vec deserialization)",
        insns,
        mem,
        1,
        0,
        false,
        get_budget(config, "bridge::list_bridges"),
        vec!["query".into(), "list".into(), "populated".into()],
    )
}

fn bench_get_admin(config: &RunConfig) -> BenchmarkResult {
    let op = "bridge::get_admin";
    let env = fresh_env();
    let (client, _) = setup(&env);

    let (insns, mem) = measure_instructions(&env, || {
        client.get_admin();
    });

    BenchmarkResult::new(
        op,
        CONTRACT,
        "Get admin address — single storage read",
        insns,
        mem,
        1,
        0,
        false,
        get_budget(config, op),
        vec!["query".into(), "admin".into()],
    )
}

// ─── Compute Fee ──────────────────────────────────────────────────────────────

fn bench_compute_fee_normal(config: &RunConfig) -> BenchmarkResult {
    let op = "bridge::compute_fee";
    let env = fresh_env();
    // Clone env before passing to measure_instructions to avoid double-borrow
    let env2 = env.clone();
    let (insns, mem) = measure_instructions(&env, || {
        BridgeContract::compute_fee(env2.clone(), 1_000_000i128, 50u64);
    });

    BenchmarkResult::new(
        op,
        CONTRACT,
        "Compute fee — pure arithmetic, no storage access",
        insns,
        mem,
        0,
        0,
        false,
        get_budget(config, op),
        vec!["compute".into(), "fee".into()],
    )
}

fn bench_compute_fee_zero_rate(config: &RunConfig) -> BenchmarkResult {
    let op = "bridge::compute_fee_zero_rate";
    let env = fresh_env();
    let env2 = env.clone();
    let (insns, mem) = measure_instructions(&env, || {
        BridgeContract::compute_fee(env2.clone(), 1_000_000i128, 0u64);
    });

    BenchmarkResult::new(
        op,
        CONTRACT,
        "Compute fee — zero rate (edge case: no fee path)",
        insns,
        mem,
        0,
        0,
        false,
        get_budget(config, "bridge::compute_fee"),
        vec!["compute".into(), "fee".into(), "zero_rate".into()],
    )
}

// ─── Storage pattern benchmarks ───────────────────────────────────────────────

fn bench_bridge_deposit_multiple_bridges_storage(config: &RunConfig) -> BenchmarkResult {
    let op = "bridge::bridge_deposit_multi_bridge_storage";
    let env = fresh_env();
    let (client, admin) = setup(&env);

    // Register 5 bridges
    for i in 0..5u32 {
        let id = format!("bridge-{}", i);
        client.register_bridge(&admin, &String::from_str(&env, &id), &50u64, &1_000i128);
    }

    // Measure deposit on the 5th bridge — bridge list is populated (warm-ish)
    let user = Address::generate(&env);
    let (insns, mem) = measure_instructions(&env, || {
        client.bridge_deposit(&user, &s(&env, "bridge-4"), &10_000i128);
    });

    BenchmarkResult::new(
        op,
        CONTRACT,
        "Bridge deposit with 5 registered bridges — storage write pattern",
        insns,
        mem,
        1,
        1,
        false,
        get_budget(config, "bridge::bridge_deposit"),
        vec![
            "deposit".into(),
            "storage_pattern".into(),
            "multi_bridge".into(),
        ],
    )
}

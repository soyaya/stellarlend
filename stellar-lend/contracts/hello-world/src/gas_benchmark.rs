use crate::{
    deposit::{AssetParams, DepositDataKey, Position},
    flash_loan::FlashLoanConfig,
    HelloContract, HelloContractClient,
};
use soroban_sdk::{testutils::Address as _, Address, Env};

extern crate std;
use std::{env as std_env, fs, path::PathBuf, string::String as StdString, vec::Vec as StdVec};

#[derive(Clone, Debug)]
struct GasSample {
    operation: &'static str,
    cpu_insns: i128,
    mem_bytes: i128,
    scenario: &'static str,
}

fn setup() -> (Env, HelloContractClient<'static>, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(HelloContract, ());
    let client = HelloContractClient::new(&env, &contract_id);
    let client: HelloContractClient<'static> = unsafe {
        // Test-only lifetime widening used throughout this crate tests.
        core::mem::transmute::<HelloContractClient<'_>, HelloContractClient<'static>>(client)
    };

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let liquidator = Address::generate(&env);
    let asset = Address::generate(&env);

    client.initialize(&admin);
    client.update_asset_config(
        &asset,
        &AssetParams {
            deposit_enabled: true,
            collateral_factor: 7_500,
            max_deposit: 1_000_000_000,
            borrow_fee_bps: 50,
        },
    );

    // Seed position and reserve state so write-heavy operations have meaningful work.
    env.as_contract(&contract_id, || {
        env.storage().persistent().set(
            &DepositDataKey::Position(user.clone()),
            &Position {
                collateral: 50_000,
                debt: 5_000,
                borrow_interest: 200,
                last_accrual_time: env.ledger().timestamp(),
            },
        );
        env.storage()
            .persistent()
            .set(&DepositDataKey::ProtocolReserve(Some(asset.clone())), &10_000i128);
    });

    (env, client, admin, user, liquidator, asset)
}

fn measure<F, R>(env: &Env, op: F) -> (i128, i128, R)
where
    F: FnOnce() -> R,
{
    env.cost_estimate().budget().reset_unlimited();
    let before_cpu = env.cost_estimate().budget().cpu_instruction_cost();
    let before_mem = env.cost_estimate().budget().memory_bytes_cost();

    let result = op();

    let after_cpu = env.cost_estimate().budget().cpu_instruction_cost();
    let after_mem = env.cost_estimate().budget().memory_bytes_cost();

    (
        (after_cpu.saturating_sub(before_cpu)) as i128,
        (after_mem.saturating_sub(before_mem)) as i128,
        result,
    )
}

fn push(samples: &mut StdVec<GasSample>, operation: &'static str, cpu: i128, mem: i128, scenario: &'static str) {
    samples.push(GasSample {
        operation,
        cpu_insns: cpu,
        mem_bytes: mem,
        scenario,
    });
}

fn write_report(samples: &[GasSample]) {
    let mut json = StdString::from("{\n  \"version\": 1,\n  \"contract\": \"hello-world\",\n  \"benchmarks\": [\n");
    for (idx, s) in samples.iter().enumerate() {
        let comma = if idx + 1 == samples.len() { "" } else { "," };
        json.push_str(&format!(
            "    {{\"operation\":\"{}\",\"scenario\":\"{}\",\"cpu_insns\":{},\"mem_bytes\":{}}}{}\n",
            s.operation, s.scenario, s.cpu_insns, s.mem_bytes, comma
        ));
    }
    json.push_str("  ]\n}\n");

    let mut workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    workspace_root.push("../..");

    let path = if let Ok(out_path) = std_env::var("GAS_BENCHMARK_OUTPUT") {
        let candidate = PathBuf::from(out_path);
        if candidate.is_absolute() {
            candidate
        } else {
            workspace_root.join(candidate)
        }
    } else {
        workspace_root.join("benchmarks/gas-current.json")
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(path, json).expect("failed to write gas benchmark report");
}

#[test]
fn benchmark_public_functions_and_storage_patterns() {
    let mut samples: StdVec<GasSample> = StdVec::new();

    // Measure initialize on a fresh contract instance.
    let init_env = Env::default();
    init_env.mock_all_auths();
    let init_contract_id = init_env.register(HelloContract, ());
    let init_client = HelloContractClient::new(&init_env, &init_contract_id);
    let init_admin = Address::generate(&init_env);
    let (cpu, mem, _) = measure(&init_env, || init_client.initialize(&init_admin));
    push(&mut samples, "initialize", cpu, mem, "write");

    let (env, client, admin, user, liquidator, asset) = setup();

    let (cpu, mem, _) = measure(&env, || client.hello());
    push(&mut samples, "hello", cpu, mem, "read_only");

    let vote_token = Address::generate(&env);
    let (cpu, mem, _) = measure(&env, || {
        client.gov_initialize(
            &admin,
            &vote_token,
            &Some(10u64),
            &Some(5u64),
            &Some(100u32),
            &Some(1_000i128),
            &Some(20u64),
            &Some(1_000i128),
        )
    });
    push(&mut samples, "gov_initialize", cpu, mem, "write");

    let (cpu, mem, _) = measure(&env, || client.transfer_admin(&admin, &admin));
    push(&mut samples, "transfer_admin", cpu, mem, "write");

    let (cpu, mem, _) = measure(&env, || client.deposit_collateral(&user, &Some(asset.clone()), &1_000));
    push(&mut samples, "deposit_collateral", cpu, mem, "write_cold");

    let (cpu, mem, _) = measure(&env, || client.deposit_collateral(&user, &Some(asset.clone()), &1_000));
    push(&mut samples, "deposit_collateral", cpu, mem, "write_warm");

    let (cpu, mem, _) =
        measure(&env, || client.try_set_risk_params(&admin, &Some(15_000), &None, &None, &None));
    push(&mut samples, "set_risk_params", cpu, mem, "write");

    let (cpu, mem, _) = measure(&env, || client.try_borrow_asset(&user, &Some(asset.clone()), &100));
    push(&mut samples, "borrow_asset", cpu, mem, "write");

    let (cpu, mem, _) = measure(&env, || client.try_repay_debt(&user, &Some(asset.clone()), &100));
    push(&mut samples, "repay_debt", cpu, mem, "write");

    let (cpu, mem, _) =
        measure(&env, || client.try_withdraw_collateral(&user, &Some(asset.clone()), &50));
    push(&mut samples, "withdraw_collateral", cpu, mem, "write");

    let (cpu, mem, _) = measure(&env, || {
        client.try_liquidate(
            &liquidator,
            &user,
            &Some(asset.clone()),
            &Some(asset.clone()),
            &10,
        )
    });
    push(&mut samples, "liquidate", cpu, mem, "write");

    let (cpu, mem, _) = measure(&env, || client.set_emergency_pause(&admin, &false));
    push(&mut samples, "set_emergency_pause", cpu, mem, "write");

    let callback = Address::generate(&env);
    let (cpu, mem, _) =
        measure(&env, || client.try_execute_flash_loan(&user, &asset, &200, &callback));
    push(&mut samples, "execute_flash_loan", cpu, mem, "write");

    let (cpu, mem, _) = measure(&env, || client.try_repay_flash_loan(&user, &asset, &200));
    push(&mut samples, "repay_flash_loan", cpu, mem, "write");

    let (cpu, mem, _) = measure(&env, || client.can_be_liquidated(&10_000, &9_000));
    push(&mut samples, "can_be_liquidated", cpu, mem, "read_only");

    let (cpu, mem, _) = measure(&env, || client.get_max_liquidatable_amount(&1_000));
    push(
        &mut samples,
        "get_max_liquidatable_amount",
        cpu,
        mem,
        "read_only",
    );

    let (cpu, mem, _) = measure(&env, || client.get_liquidation_incentive_amount(&1_000));
    push(
        &mut samples,
        "get_liquidation_incentive_amount",
        cpu,
        mem,
        "read_only",
    );

    let (cpu, mem, _) = measure(&env, || client.require_min_collateral_ratio(&10_000, &5_000));
    push(
        &mut samples,
        "require_min_collateral_ratio",
        cpu,
        mem,
        "read_only",
    );

    let treasury = Address::generate(&env);
    let (cpu, mem, _) = measure(&env, || client.set_treasury(&admin, &treasury));
    push(&mut samples, "set_treasury", cpu, mem, "write");

    let (cpu, mem, _) = measure(&env, || client.get_treasury());
    push(&mut samples, "get_treasury", cpu, mem, "read_only");

    let (cpu, mem, _) = measure(&env, || client.get_reserve_balance(&Some(asset.clone())));
    push(&mut samples, "get_reserve_balance", cpu, mem, "read_only");

    let recipient = Address::generate(&env);
    let (cpu, mem, _) = measure(&env, || {
        client.claim_reserves(&admin, &Some(asset.clone()), &recipient, &10)
    });
    push(&mut samples, "claim_reserves", cpu, mem, "write");

    let (cpu, mem, _) = measure(&env, || client.set_fee_config(&admin, &1_000, &500));
    push(&mut samples, "set_fee_config", cpu, mem, "write");

    let (cpu, mem, _) = measure(&env, || client.get_fee_config());
    push(&mut samples, "get_fee_config", cpu, mem, "read_only");

    let (cpu, mem, _) = measure(&env, || client.get_user_asset_collateral(&user, &asset));
    push(&mut samples, "get_user_asset_collateral", cpu, mem, "read_only");

    let (cpu, mem, _) = measure(&env, || client.get_user_asset_list(&user));
    push(&mut samples, "get_user_asset_list", cpu, mem, "read_only");

    let (cpu, mem, _) = measure(&env, || client.get_user_total_collateral_value(&user));
    push(
        &mut samples,
        "get_user_total_collateral_value",
        cpu,
        mem,
        "read_only",
    );

    let (cpu, mem, _) = measure(&env, || client.try_get_health_factor(&user));
    push(&mut samples, "get_health_factor", cpu, mem, "read_only");

    let (cpu, mem, _) = measure(&env, || client.try_get_user_position(&user));
    push(&mut samples, "get_user_position", cpu, mem, "read_cold");

    let (cpu, mem, _) = measure(&env, || client.try_get_user_position(&user));
    push(&mut samples, "get_user_position", cpu, mem, "read_warm");

    let (cpu, mem, _) = measure(&env, || {
        client.update_asset_config(
            &asset,
            &AssetParams {
                deposit_enabled: true,
                collateral_factor: 8_000,
                max_deposit: 2_000_000_000,
                borrow_fee_bps: 100,
            },
        )
    });
    push(&mut samples, "update_asset_config", cpu, mem, "write");

    let (cpu, mem, _) = measure(&env, || {
        client.configure_flash_loan(
            &admin,
            &FlashLoanConfig {
                fee_bps: 9,
                max_amount: 1_000_000,
                min_amount: 100,
            },
        )
    });
    push(&mut samples, "configure_flash_loan", cpu, mem, "write");

    write_report(&samples);
    assert!(
        samples.len() >= 30,
        "expected full benchmark coverage, got {} operations",
        samples.len()
    );
}

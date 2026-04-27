use crate::{deposit::DepositDataKey, HelloContract, HelloContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup() -> (Env, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(HelloContract, ());
    let client = HelloContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, admin, contract_id)
}

// ---- Treasury Address -------------------------------------------------------

#[test]
fn test_set_and_get_treasury() {
    let (env, admin, contract_id) = setup();
    let client = HelloContractClient::new(&env, &contract_id);
    let treasury = Address::generate(&env);

    assert_eq!(client.get_treasury(), None);
    client.set_treasury(&admin, &treasury);
    assert_eq!(client.get_treasury(), Some(treasury));
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_set_treasury_non_admin_rejected() {
    let (env, _admin, contract_id) = setup();
    let client = HelloContractClient::new(&env, &contract_id);
    let non_admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.set_treasury(&non_admin, &treasury);
}

// ---- Fee Configuration -----------------------------------------------------

#[test]
fn test_default_fee_config() {
    let (env, _admin, contract_id) = setup();
    let client = HelloContractClient::new(&env, &contract_id);
    let config = client.get_fee_config();
    // Default: 10% interest fee (1000 bps), 10% liquidation fee (1000 bps)
    assert_eq!(config.interest_fee_bps, 1000);
    assert_eq!(config.liquidation_fee_bps, 1000);
}

#[test]
fn test_set_fee_config() {
    let (env, admin, contract_id) = setup();
    let client = HelloContractClient::new(&env, &contract_id);
    client.set_fee_config(&admin, &500, &200);
    let config = client.get_fee_config();
    assert_eq!(config.interest_fee_bps, 500);
    assert_eq!(config.liquidation_fee_bps, 200);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_set_fee_config_non_admin_rejected() {
    let (env, _admin, contract_id) = setup();
    let client = HelloContractClient::new(&env, &contract_id);
    let non_admin = Address::generate(&env);
    client.set_fee_config(&non_admin, &500, &200);
}

#[test]
#[should_panic(expected = "Error(Contract, #26)")]
fn test_set_fee_config_interest_out_of_range() {
    let (env, admin, contract_id) = setup();
    let client = HelloContractClient::new(&env, &contract_id);
    client.set_fee_config(&admin, &10001, &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #26)")]
fn test_set_fee_config_liquidation_out_of_range() {
    let (env, admin, contract_id) = setup();
    let client = HelloContractClient::new(&env, &contract_id);
    client.set_fee_config(&admin, &0, &10001);
}

#[test]
fn test_set_fee_config_zero_fees_allowed() {
    let (env, admin, contract_id) = setup();
    let client = HelloContractClient::new(&env, &contract_id);
    client.set_fee_config(&admin, &0, &0);
    let config = client.get_fee_config();
    assert_eq!(config.interest_fee_bps, 0);
    assert_eq!(config.liquidation_fee_bps, 0);
}

#[test]
fn test_set_fee_config_max_fees_allowed() {
    let (env, admin, contract_id) = setup();
    let client = HelloContractClient::new(&env, &contract_id);
    client.set_fee_config(&admin, &10000, &10000);
    let config = client.get_fee_config();
    assert_eq!(config.interest_fee_bps, 10000);
    assert_eq!(config.liquidation_fee_bps, 10000);
}

#[test]
fn test_fee_config_update_is_persistent() {
    let (env, admin, contract_id) = setup();
    let client = HelloContractClient::new(&env, &contract_id);
    client.set_fee_config(&admin, &750, &300);
    // Re-read via a fresh client on the same contract
    let client2 = HelloContractClient::new(&env, &contract_id);
    let config = client2.get_fee_config();
    assert_eq!(config.interest_fee_bps, 750);
    assert_eq!(config.liquidation_fee_bps, 300);
}

// ---- Reserve Balance --------------------------------------------------------

#[test]
fn test_get_reserve_balance_default_zero() {
    let (env, _admin, contract_id) = setup();
    let client = HelloContractClient::new(&env, &contract_id);
    let asset = Address::generate(&env);
    assert_eq!(client.get_reserve_balance(&Some(asset)), 0);
    assert_eq!(client.get_reserve_balance(&None), 0);
}

#[test]
fn test_get_reserve_balance_reflects_storage() {
    let (env, _admin, contract_id) = setup();
    let asset = Address::generate(&env);

    env.as_contract(&contract_id, || {
        env.storage().persistent().set(
            &DepositDataKey::ProtocolReserve(Some(asset.clone())),
            &750i128,
        );
    });

    let client = HelloContractClient::new(&env, &contract_id);
    assert_eq!(client.get_reserve_balance(&Some(asset)), 750);
}

#[test]
fn test_multiple_assets_independent_reserves() {
    let (env, _admin, contract_id) = setup();
    let asset1 = Address::generate(&env);
    let asset2 = Address::generate(&env);

    env.as_contract(&contract_id, || {
        env.storage().persistent().set(
            &DepositDataKey::ProtocolReserve(Some(asset1.clone())),
            &100i128,
        );
        env.storage().persistent().set(
            &DepositDataKey::ProtocolReserve(Some(asset2.clone())),
            &200i128,
        );
    });

    let client = HelloContractClient::new(&env, &contract_id);
    assert_eq!(client.get_reserve_balance(&Some(asset1)), 100);
    assert_eq!(client.get_reserve_balance(&Some(asset2)), 200);
}

// ---- Claim Reserves ---------------------------------------------------------

#[test]
fn test_claim_reserves_updates_balance() {
    let (env, admin, contract_id) = setup();
    let asset = Address::generate(&env);
    let recipient = Address::generate(&env);

    env.as_contract(&contract_id, || {
        env.storage().persistent().set(
            &DepositDataKey::ProtocolReserve(Some(asset.clone())),
            &1000i128,
        );
    });

    let client = HelloContractClient::new(&env, &contract_id);
    client.claim_reserves(&admin, &Some(asset.clone()), &recipient, &400);
    assert_eq!(client.get_reserve_balance(&Some(asset)), 600);
}

#[test]
fn test_claim_full_reserve() {
    let (env, admin, contract_id) = setup();
    let asset = Address::generate(&env);
    let recipient = Address::generate(&env);

    env.as_contract(&contract_id, || {
        env.storage().persistent().set(
            &DepositDataKey::ProtocolReserve(Some(asset.clone())),
            &500i128,
        );
    });

    let client = HelloContractClient::new(&env, &contract_id);
    client.claim_reserves(&admin, &Some(asset.clone()), &recipient, &500);
    assert_eq!(client.get_reserve_balance(&Some(asset)), 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_claim_reserves_non_admin_rejected() {
    let (env, _admin, contract_id) = setup();
    let client = HelloContractClient::new(&env, &contract_id);
    let non_admin = Address::generate(&env);
    let asset = Address::generate(&env);

    // Seed reserve so the call would succeed if auth check passed
    env.as_contract(&contract_id, || {
        env.storage().persistent().set(
            &DepositDataKey::ProtocolReserve(Some(asset.clone())),
            &500i128,
        );
    });

    client.claim_reserves(&non_admin, &Some(asset), &non_admin, &100);
}

#[test]
#[should_panic(expected = "Error(Contract, #25)")]
fn test_claim_reserves_exceeds_balance() {
    let (env, admin, contract_id) = setup();
    let client = HelloContractClient::new(&env, &contract_id);
    let asset = Address::generate(&env);
    let recipient = Address::generate(&env);
    // Reserve is 0, claiming 100 should fail with InsufficientReserve (#3)
    client.claim_reserves(&admin, &Some(asset), &recipient, &100);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_claim_reserves_zero_amount() {
    let (env, admin, contract_id) = setup();
    let client = HelloContractClient::new(&env, &contract_id);
    let asset = Address::generate(&env);
    let recipient = Address::generate(&env);
    client.claim_reserves(&admin, &Some(asset), &recipient, &0);
}

// ---- Interest Fee Integration -----------------------------------------------

#[test]
fn test_repay_interest_credited_to_reserve_with_configurable_fee() {
    let (env, admin, contract_id) = setup();
    let client = HelloContractClient::new(&env, &contract_id);
    let user = Address::generate(&env);
    let asset = Address::generate(&env);

    // Set a 20% interest fee
    client.set_fee_config(&admin, &2000, &1000);

    // Set up position with 100 borrow_interest
    env.as_contract(&contract_id, || {
        env.storage().persistent().set(
            &DepositDataKey::Position(user.clone()),
            &crate::deposit::Position {
                collateral: 10000,
                debt: 1000,
                borrow_interest: 100,
                last_accrual_time: env.ledger().timestamp(),
            },
        );
    });

    // Repay 100 (covers all interest). Token call is skipped in test cfg.
    client.repay_debt(&user, &Some(asset.clone()), &100);

    // 20% of 100 interest paid = 20 goes to protocol reserve
    let reserve = client.get_reserve_balance(&Some(asset));
    assert_eq!(reserve, 20);
}

#[test]
fn test_repay_zero_interest_fee_no_reserve_added() {
    let (env, admin, contract_id) = setup();
    let client = HelloContractClient::new(&env, &contract_id);
    let user = Address::generate(&env);
    let asset = Address::generate(&env);

    // Disable interest fee
    client.set_fee_config(&admin, &0, &0);

    env.as_contract(&contract_id, || {
        env.storage().persistent().set(
            &DepositDataKey::Position(user.clone()),
            &crate::deposit::Position {
                collateral: 10000,
                debt: 1000,
                borrow_interest: 100,
                last_accrual_time: env.ledger().timestamp(),
            },
        );
    });

    client.repay_debt(&user, &Some(asset.clone()), &100);

    let reserve = client.get_reserve_balance(&Some(asset));
    assert_eq!(reserve, 0);
}

#[test]
fn test_repay_default_10_percent_interest_fee() {
    let (env, _admin, contract_id) = setup();
    let client = HelloContractClient::new(&env, &contract_id);
    let user = Address::generate(&env);
    let asset = Address::generate(&env);

    // Default fee config: 10% (1000 bps)
    env.as_contract(&contract_id, || {
        env.storage().persistent().set(
            &DepositDataKey::Position(user.clone()),
            &crate::deposit::Position {
                collateral: 10000,
                debt: 1000,
                borrow_interest: 100,
                last_accrual_time: env.ledger().timestamp(),
            },
        );
    });

    client.repay_debt(&user, &Some(asset.clone()), &100);

    // 10% of 100 interest = 10
    let reserve = client.get_reserve_balance(&Some(asset));
    assert_eq!(reserve, 10);
}

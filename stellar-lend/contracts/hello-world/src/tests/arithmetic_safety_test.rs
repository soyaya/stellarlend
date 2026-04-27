//! Arithmetic safety regression tests.
//!
//! These tests focus on ensuring protocol entrypoints error on overflow/underflow
//! rather than wrapping.

#![cfg(test)]

use crate::{HelloContract, HelloContractClient};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env};

fn setup(env: &Env) -> (Address, Address, HelloContractClient<'_>) {
    let contract_id = env.register(HelloContract, ());
    let client = HelloContractClient::new(env, &contract_id);
    let admin = Address::generate(env);
    client.initialize(&admin);
    (contract_id, admin, client)
}

#[test]
fn test_deposit_overflow_errors_not_wraps() {
    let env = Env::default();
    env.mock_all_auths();
    let (_cid, _admin, client) = setup(&env);
    let user = Address::generate(&env);

    // First deposit sets collateral near MAX.
    let big = i128::MAX;
    client.deposit_collateral(&user, &None, &big);

    // Any additional positive deposit must error (would overflow).
    let res = client.try_deposit_collateral(&user, &None, &1);
    assert!(res.is_err());
}

#[test]
fn test_withdraw_underflow_errors_not_wraps() {
    let env = Env::default();
    env.mock_all_auths();
    let (_cid, _admin, client) = setup(&env);
    let user = Address::generate(&env);

    client.deposit_collateral(&user, &None, &100);

    // Withdraw more than balance must error (would underflow).
    let res = client.try_withdraw_collateral(&user, &None, &101);
    assert!(res.is_err());
}


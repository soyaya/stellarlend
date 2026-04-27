//! Off-chain intent verification tests (nonce + expiry).

#![cfg(test)]

use crate::{HelloContract, HelloContractClient};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, Env, Symbol};

fn setup(env: &Env) -> (Address, Address, HelloContractClient<'_>) {
    let contract_id = env.register(HelloContract, ());
    let client = HelloContractClient::new(env, &contract_id);
    let admin = Address::generate(env);
    client.initialize(&admin);
    (contract_id, admin, client)
}

#[test]
fn test_intent_nonce_increments_and_blocks_replay() {
    let env = Env::default();
    env.mock_all_auths();
    let (_cid, _admin, client) = setup(&env);

    let relayer = Address::generate(&env);
    let user = Address::generate(&env);
    client.deposit_collateral(&user, &None, &1_000_000_000);

    env.ledger().with_mut(|li| li.timestamp = 100);
    let op = Symbol::new(&env, "borrow");
    assert_eq!(client.get_intent_nonce(&user, &op), 0);

    // First intent with nonce=0 succeeds.
    let expires = 200;
    client.borrow_asset_intent(&relayer, &user, &None, &1, &0, &expires);
    assert_eq!(client.get_intent_nonce(&user, &op), 1);

    // Replay same nonce should fail.
    let res = client.try_borrow_asset_intent(&relayer, &user, &None, &1, &0, &expires);
    assert!(res.is_err());
}

#[test]
fn test_intent_expiration_blocks_execution() {
    let env = Env::default();
    env.mock_all_auths();
    let (_cid, _admin, client) = setup(&env);

    let relayer = Address::generate(&env);
    let user = Address::generate(&env);
    client.deposit_collateral(&user, &None, &1_000_000_000);

    env.ledger().with_mut(|li| li.timestamp = 500);
    let expired_at = 499;
    let res = client.try_borrow_asset_intent(&relayer, &user, &None, &1, &0, &expired_at);
    assert!(res.is_err());
}


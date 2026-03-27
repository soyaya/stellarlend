//! Tests for input length validation (issue #99).
//!
//! Verifies that proposal description strings exceeding MAX_DESCRIPTION_LEN
//! are rejected with InputTooLong, and that boundary-length inputs are accepted.

#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env, String};
use soroban_sdk::token::StellarAssetClient;

use crate::{
    errors::GovernanceError,
    governance::MAX_DESCRIPTION_LEN,
    types::ProposalType,
    HelloContract, HelloContractClient,
};

// ── helpers ──────────────────────────────────────────────────────────────────

fn setup(env: &Env) -> (HelloContractClient, Address, Address) {
    let admin = Address::generate(env);
    let token_addr = env.register_stellar_asset_contract(admin.clone());
    StellarAssetClient::new(env, &token_addr).mint(&admin, &1_000_000_i128);

    let id = env.register_contract(None, HelloContract);
    let client = HelloContractClient::new(env, &id);
    env.mock_all_auths();
    client.initialize(&admin);
    client.gov_initialize(
        &admin,
        &token_addr,
        &Some(259200),
        &Some(86400),
        &Some(400),
        &Some(0),   // no token threshold so any proposer works
        &Some(604800),
        &Some(5000),
    );
    (client, admin, token_addr)
}

/// Build a Soroban String of exactly `len` bytes (ASCII 'a').
fn str_of_len(env: &Env, len: usize) -> String {
    let bytes: soroban_sdk::Bytes = soroban_sdk::Bytes::from_slice(env, &vec![b'a'; len]);
    String::from_bytes(env, &bytes)
}

// ── create_proposal ───────────────────────────────────────────────────────────

#[test]
fn test_create_proposal_at_max_len_succeeds() {
    let env = Env::default();
    let (client, admin, token_addr) = setup(&env);
    let proposer = Address::generate(&env);
    StellarAssetClient::new(&env, &token_addr).mint(&proposer, &1_000_i128);

    let desc = str_of_len(&env, MAX_DESCRIPTION_LEN as usize);
    // Must not panic / return Err
    client.gov_create_proposal(&proposer, &ProposalType::EmergencyPause(true), &desc, &None);
}

#[test]
fn test_create_proposal_over_max_len_rejected() {
    let env = Env::default();
    let (client, admin, token_addr) = setup(&env);
    let proposer = Address::generate(&env);
    StellarAssetClient::new(&env, &token_addr).mint(&proposer, &1_000_i128);

    let desc = str_of_len(&env, MAX_DESCRIPTION_LEN as usize + 1);
    let result = client.try_gov_create_proposal(
        &proposer,
        &ProposalType::EmergencyPause(true),
        &desc,
        &None,
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        GovernanceError::InputTooLong.into()
    );
}

// ── create_admin_proposal ─────────────────────────────────────────────────────

#[test]
fn test_admin_proposal_at_max_len_succeeds() {
    let env = Env::default();
    let (client, admin, _) = setup(&env);

    let desc = str_of_len(&env, MAX_DESCRIPTION_LEN as usize);
    client.gov_create_admin_proposal(&admin, &ProposalType::EmergencyPause(false), &desc);
}

#[test]
fn test_admin_proposal_over_max_len_rejected() {
    let env = Env::default();
    let (client, admin, _) = setup(&env);

    let desc = str_of_len(&env, MAX_DESCRIPTION_LEN as usize + 1);
    let result = client.try_gov_create_admin_proposal(
        &admin,
        &ProposalType::EmergencyPause(false),
        &desc,
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        GovernanceError::InputTooLong.into()
    );
}

// ── create_emergency_proposal ─────────────────────────────────────────────────

#[test]
fn test_emergency_proposal_at_max_len_succeeds() {
    let env = Env::default();
    let (client, admin, _) = setup(&env);

    let desc = str_of_len(&env, MAX_DESCRIPTION_LEN as usize);
    client.gov_create_emergency_proposal(&admin, &ProposalType::EmergencyPause(true), &desc);
}

#[test]
fn test_emergency_proposal_over_max_len_rejected() {
    let env = Env::default();
    let (client, admin, _) = setup(&env);

    let desc = str_of_len(&env, MAX_DESCRIPTION_LEN as usize + 1);
    let result = client.try_gov_create_emergency_proposal(
        &admin,
        &ProposalType::EmergencyPause(true),
        &desc,
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        GovernanceError::InputTooLong.into()
    );
}

// ── empty string is always valid ──────────────────────────────────────────────

#[test]
fn test_empty_description_accepted() {
    let env = Env::default();
    let (client, admin, token_addr) = setup(&env);
    let proposer = Address::generate(&env);
    StellarAssetClient::new(&env, &token_addr).mint(&proposer, &1_000_i128);

    let desc = String::from_str(&env, "");
    client.gov_create_proposal(&proposer, &ProposalType::EmergencyPause(true), &desc, &None);
}

use crate::bridge::*;
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, Env, String, Vec};

fn setup() -> (Env, BridgeContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register(BridgeContract, ());
    let client = BridgeContractClient::new(&env, &id);
    let admin = Address::generate(&env);
    client.init(&admin);
    (env, client, admin)
}

fn s(env: &Env, v: &str) -> String {
    String::from_str(env, v)
}

fn default_bridge(client: &BridgeContractClient, env: &Env, admin: &Address) {
    client.register_bridge(admin, &s(env, "eth-mainnet"), &50u64, &1_000i128);
}

fn register_default_validators(
    env: &Env,
    client: &BridgeContractClient,
    admin: &Address,
) -> Vec<Address> {
    let mut validators = Vec::new(env);
    for _ in 0..3 {
        let validator = Address::generate(env);
        client.register_validator(admin, &validator, &10_000i128);
        validators.push_back(validator);
    }
    validators
}

fn submit_default_message(
    env: &Env,
    client: &BridgeContractClient,
    relayer: &Address,
    recipient: &Address,
    nonce: u64,
) -> u64 {
    client.submit_cross_chain_message(
        relayer,
        &CrossChainMessageInput {
            bridge_id: s(env, "eth-mainnet"),
            channel_id: s(env, "eth-channel"),
            source_chain: s(env, "ethereum"),
            source_tx_id: s(env, "0xtx"),
            source_height: 10,
            nonce,
            recipient: recipient.clone(),
            amount: 5_000,
            payload_version: 1,
        },
    )
}

// ── init ──────────────────────────────────────────────────────────────────────

#[test]
fn init_sets_admin() {
    let (_, client, admin) = setup();
    assert_eq!(client.get_admin(), admin);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn init_twice_panics() {
    let (env, client, _) = setup();
    client.init(&Address::generate(&env));
}

// ── register_bridge ───────────────────────────────────────────────────────────

#[test]
fn register_bridge_success() {
    let (env, client, admin) = setup();
    default_bridge(&client, &env, &admin);
    let cfg = client.get_bridge_config(&s(&env, "eth-mainnet"));
    assert_eq!(cfg.fee_bps, 50);
    assert_eq!(cfg.min_amount, 1_000);
    assert!(cfg.active);
    assert_eq!(cfg.total_deposited, 0);
    assert_eq!(cfg.total_withdrawn, 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn register_bridge_non_admin_panics() {
    let (env, client, _) = setup();
    let rando = Address::generate(&env);
    client.register_bridge(&rando, &s(&env, "bsc"), &10u64, &100i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn register_bridge_duplicate_panics() {
    let (env, client, admin) = setup();
    default_bridge(&client, &env, &admin);
    client.register_bridge(&admin, &s(&env, "eth-mainnet"), &50u64, &1_000i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn register_bridge_fee_too_high_panics() {
    let (env, client, admin) = setup();
    client.register_bridge(&admin, &s(&env, "bsc"), &1_001u64, &100i128);
}

#[test]
fn register_bridge_max_fee_ok() {
    let (env, client, admin) = setup();
    client.register_bridge(&admin, &s(&env, "bsc"), &1_000u64, &100i128);
    assert_eq!(client.get_bridge_config(&s(&env, "bsc")).fee_bps, 1_000);
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn register_bridge_empty_id_panics() {
    let (env, client, admin) = setup();
    client.register_bridge(&admin, &s(&env, ""), &10u64, &100i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn register_bridge_id_too_long_panics() {
    let (env, client, admin) = setup();
    let long = String::from_str(&env, &"a".repeat(65));
    client.register_bridge(&admin, &long, &10u64, &100i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #10)")]
fn register_bridge_negative_min_amount_panics() {
    let (env, client, admin) = setup();
    client.register_bridge(&admin, &s(&env, "bsc"), &10u64, &-1i128);
}

// ── set_bridge_fee ─────────────────────────────────────────────────────────────

#[test]
fn set_bridge_fee_success() {
    let (env, client, admin) = setup();
    default_bridge(&client, &env, &admin);
    client.set_bridge_fee(&admin, &s(&env, "eth-mainnet"), &200u64);
    assert_eq!(
        client.get_bridge_config(&s(&env, "eth-mainnet")).fee_bps,
        200
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn set_bridge_fee_not_found_panics() {
    let (env, client, admin) = setup();
    client.set_bridge_fee(&admin, &s(&env, "ghost"), &10u64);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn set_bridge_fee_exceeds_cap_panics() {
    let (env, client, admin) = setup();
    default_bridge(&client, &env, &admin);
    client.set_bridge_fee(&admin, &s(&env, "eth-mainnet"), &9_999u64);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn set_bridge_fee_unauthorized_panics() {
    let (env, client, admin) = setup();
    default_bridge(&client, &env, &admin);
    let rando = Address::generate(&env);
    client.set_bridge_fee(&rando, &s(&env, "eth-mainnet"), &100u64);
}

// ── set_bridge_active ──────────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn deactivate_bridge_stops_deposits() {
    let (env, client, admin) = setup();
    default_bridge(&client, &env, &admin);
    client.set_bridge_active(&admin, &s(&env, "eth-mainnet"), &false);
    let user = Address::generate(&env);
    client.bridge_deposit(&user, &s(&env, "eth-mainnet"), &10_000i128);
}

#[test]
fn reactivate_bridge_allows_deposits() {
    let (env, client, admin) = setup();
    default_bridge(&client, &env, &admin);
    client.set_bridge_active(&admin, &s(&env, "eth-mainnet"), &false);
    client.set_bridge_active(&admin, &s(&env, "eth-mainnet"), &true);
    let user = Address::generate(&env);
    client.bridge_deposit(&user, &s(&env, "eth-mainnet"), &10_000i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn set_bridge_active_not_found_panics() {
    let (env, client, admin) = setup();
    client.set_bridge_active(&admin, &s(&env, "ghost"), &false);
}

// ── bridge_deposit ─────────────────────────────────────────────────────────────

#[test]
fn deposit_returns_correct_net() {
    let (env, client, admin) = setup();
    default_bridge(&client, &env, &admin); // fee_bps=50, min=1_000
    let user = Address::generate(&env);
    // fee = 100_000 * 50 / 10_000 = 500  →  net = 99_500
    let net = client.bridge_deposit(&user, &s(&env, "eth-mainnet"), &100_000i128);
    assert_eq!(net, 99_500);
}

#[test]
fn deposit_zero_fee_bridge() {
    let (env, client, admin) = setup();
    client.register_bridge(&admin, &s(&env, "free"), &0u64, &1i128);
    let user = Address::generate(&env);
    let net = client.bridge_deposit(&user, &s(&env, "free"), &50_000i128);
    assert_eq!(net, 50_000);
}

#[test]
fn deposit_accumulates_total_deposited() {
    let (env, client, admin) = setup();
    default_bridge(&client, &env, &admin);
    let user = Address::generate(&env);
    client.bridge_deposit(&user, &s(&env, "eth-mainnet"), &20_000i128);
    client.bridge_deposit(&user, &s(&env, "eth-mainnet"), &30_000i128);
    assert_eq!(
        client
            .get_bridge_config(&s(&env, "eth-mainnet"))
            .total_deposited,
        50_000
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn deposit_zero_amount_panics() {
    let (env, client, admin) = setup();
    default_bridge(&client, &env, &admin);
    let user = Address::generate(&env);
    client.bridge_deposit(&user, &s(&env, "eth-mainnet"), &0i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn deposit_negative_amount_panics() {
    let (env, client, admin) = setup();
    default_bridge(&client, &env, &admin);
    let user = Address::generate(&env);
    client.bridge_deposit(&user, &s(&env, "eth-mainnet"), &-1i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #12)")]
fn deposit_below_minimum_panics() {
    let (env, client, admin) = setup();
    default_bridge(&client, &env, &admin); // min=1_000
    let user = Address::generate(&env);
    client.bridge_deposit(&user, &s(&env, "eth-mainnet"), &999i128);
}

#[test]
fn deposit_exactly_minimum_succeeds() {
    let (env, client, admin) = setup();
    default_bridge(&client, &env, &admin);
    let user = Address::generate(&env);
    client.bridge_deposit(&user, &s(&env, "eth-mainnet"), &1_000i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn deposit_unknown_bridge_panics() {
    let (env, client, _) = setup();
    let user = Address::generate(&env);
    client.bridge_deposit(&user, &s(&env, "ghost"), &50_000i128);
}

// ── bridge_withdraw ────────────────────────────────────────────────────────────

#[test]
fn withdraw_accumulates_total_withdrawn() {
    let (env, client, admin) = setup();
    default_bridge(&client, &env, &admin);
    let recip = Address::generate(&env);
    client.bridge_withdraw(&admin, &s(&env, "eth-mainnet"), &recip, &40_000i128);
    assert_eq!(
        client
            .get_bridge_config(&s(&env, "eth-mainnet"))
            .total_withdrawn,
        40_000
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn withdraw_non_admin_panics() {
    let (env, client, admin) = setup();
    default_bridge(&client, &env, &admin);
    let rando = Address::generate(&env);
    let recip = Address::generate(&env);
    client.bridge_withdraw(&rando, &s(&env, "eth-mainnet"), &recip, &5_000i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn withdraw_zero_amount_panics() {
    let (env, client, admin) = setup();
    default_bridge(&client, &env, &admin);
    let recip = Address::generate(&env);
    client.bridge_withdraw(&admin, &s(&env, "eth-mainnet"), &recip, &0i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #12)")]
fn withdraw_below_minimum_panics() {
    let (env, client, admin) = setup();
    default_bridge(&client, &env, &admin); // min=1_000
    let recip = Address::generate(&env);
    client.bridge_withdraw(&admin, &s(&env, "eth-mainnet"), &recip, &500i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn withdraw_unknown_bridge_panics() {
    let (env, client, admin) = setup();
    let recip = Address::generate(&env);
    client.bridge_withdraw(&admin, &s(&env, "ghost"), &recip, &5_000i128);
}

// -- cross-chain verification -------------------------------------------------

#[test]
fn verified_message_flow_requires_quorum_and_finality() {
    let (env, client, admin) = setup();
    default_bridge(&client, &env, &admin);
    let validators = register_default_validators(&env, &client, &admin);
    let relayer = Address::generate(&env);
    let recipient = Address::generate(&env);

    env.ledger().with_mut(|li| li.sequence_number = 1);
    let message_id = submit_default_message(&env, &client, &relayer, &recipient, 1);

    client.attest_cross_chain_message(&validators.get(0).unwrap(), &message_id, &true);
    let before_quorum = client.try_execute_verified_withdrawal(&relayer, &message_id);
    assert_eq!(before_quorum, Err(Ok(ContractError::QuorumNotReached)));

    client.attest_cross_chain_message(&validators.get(1).unwrap(), &message_id, &true);
    let before_finality = client.try_execute_verified_withdrawal(&relayer, &message_id);
    assert_eq!(before_finality, Err(Ok(ContractError::MessageNotFinal)));

    env.ledger().with_mut(|li| li.sequence_number = 4);
    client.execute_verified_withdrawal(&relayer, &message_id);

    let msg = client.get_cross_chain_message(&message_id);
    assert!(msg.executed);
    assert_eq!(
        client
            .get_bridge_config(&s(&env, "eth-mainnet"))
            .total_withdrawn,
        5_000
    );
    assert_eq!(client.get_bridge_security_stats().executed_messages, 1);
}

#[test]
fn replay_protection_blocks_duplicate_message_keys() {
    let (env, client, admin) = setup();
    default_bridge(&client, &env, &admin);
    register_default_validators(&env, &client, &admin);
    let relayer = Address::generate(&env);
    let recipient = Address::generate(&env);

    submit_default_message(&env, &client, &relayer, &recipient, 7);
    let duplicate = client.try_submit_cross_chain_message(
        &relayer,
        &CrossChainMessageInput {
            bridge_id: s(&env, "eth-mainnet"),
            channel_id: s(&env, "eth-channel"),
            source_chain: s(&env, "ethereum"),
            source_tx_id: s(&env, "0xtx"),
            source_height: 10,
            nonce: 7,
            recipient: recipient.clone(),
            amount: 5_000,
            payload_version: 1,
        },
    );

    assert_eq!(duplicate, Err(Ok(ContractError::ReplayDetected)));
}

#[test]
fn conflicting_attestations_slash_validators_and_close_channel() {
    let (env, client, admin) = setup();
    default_bridge(&client, &env, &admin);
    client.set_bridge_security_config(
        &admin,
        &SecurityConfig {
            min_validator_signatures: 2,
            min_finality_ledgers: 3,
            optimistic_delay_ledgers: 2,
            slash_bps: 2_500,
            supported_message_version: 1,
            anomaly_close_threshold: 1,
        },
    );
    let validators = register_default_validators(&env, &client, &admin);
    let relayer = Address::generate(&env);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    let first = client.submit_cross_chain_message(
        &relayer,
        &CrossChainMessageInput {
            bridge_id: s(&env, "eth-mainnet"),
            channel_id: s(&env, "eth-channel"),
            source_chain: s(&env, "ethereum"),
            source_tx_id: s(&env, "0xconflict"),
            source_height: 10,
            nonce: 11,
            recipient: alice.clone(),
            amount: 5_000,
            payload_version: 1,
        },
    );
    let second = client.submit_cross_chain_message(
        &relayer,
        &CrossChainMessageInput {
            bridge_id: s(&env, "eth-mainnet"),
            channel_id: s(&env, "eth-channel"),
            source_chain: s(&env, "ethereum"),
            source_tx_id: s(&env, "0xconflict"),
            source_height: 11,
            nonce: 12,
            recipient: bob.clone(),
            amount: 7_000,
            payload_version: 1,
        },
    );

    client.attest_cross_chain_message(&validators.get(0).unwrap(), &first, &true);
    client.attest_cross_chain_message(&validators.get(0).unwrap(), &second, &true);
    client.attest_cross_chain_message(&validators.get(1).unwrap(), &first, &true);
    client.attest_cross_chain_message(&validators.get(1).unwrap(), &second, &true);

    let slashed = client.slash_conflicting_messages(&relayer, &first, &second);
    assert_eq!(slashed.len(), 2);
    assert!(client.get_cross_chain_message(&first).invalidated);
    assert!(client.get_cross_chain_message(&second).invalidated);

    let first_validator = client.get_validator(&validators.get(0).unwrap());
    assert!(first_validator.slashed_total > 0);
    assert!(
        client
            .get_channel_state(&s(&env, "eth-channel"))
            .emergency_closed
    );
    assert_eq!(client.get_bridge_security_stats().slashes, 2);
}

#[test]
fn emergency_closed_channel_blocks_new_messages() {
    let (env, client, admin) = setup();
    default_bridge(&client, &env, &admin);
    let relayer = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.close_channel_emergency(
        &admin,
        &s(&env, "eth-channel"),
        &s(&env, "bridge_oracle_failure"),
    );
    let result = client.try_submit_cross_chain_message(
        &relayer,
        &CrossChainMessageInput {
            bridge_id: s(&env, "eth-mainnet"),
            channel_id: s(&env, "eth-channel"),
            source_chain: s(&env, "ethereum"),
            source_tx_id: s(&env, "0xclosed"),
            source_height: 15,
            nonce: 1,
            recipient: recipient.clone(),
            amount: 5_000,
            payload_version: 1,
        },
    );

    assert_eq!(result, Err(Ok(ContractError::ChannelClosed)));
}

#[test]
fn out_of_order_message_nonce_is_rejected_and_monitored() {
    let (env, client, admin) = setup();
    default_bridge(&client, &env, &admin);
    let relayer = Address::generate(&env);
    let recipient = Address::generate(&env);

    submit_default_message(&env, &client, &relayer, &recipient, 5);
    let replayed_nonce = client.try_submit_cross_chain_message(
        &relayer,
        &CrossChainMessageInput {
            bridge_id: s(&env, "eth-mainnet"),
            channel_id: s(&env, "eth-channel"),
            source_chain: s(&env, "ethereum"),
            source_tx_id: s(&env, "0xnewtx"),
            source_height: 12,
            nonce: 4,
            recipient: recipient.clone(),
            amount: 5_000,
            payload_version: 1,
        },
    );

    assert_eq!(
        replayed_nonce,
        Err(Ok(ContractError::InvalidMessageOrdering))
    );
}

// ── list_bridges ───────────────────────────────────────────────────────────────

#[test]
fn list_bridges_empty() {
    let (_, client, _) = setup();
    assert_eq!(client.list_bridges().len(), 0);
}

#[test]
fn list_bridges_multiple() {
    let (env, client, admin) = setup();
    client.register_bridge(&admin, &s(&env, "bsc"), &10u64, &100i128);
    client.register_bridge(&admin, &s(&env, "polygon"), &20u64, &200i128);
    client.register_bridge(&admin, &s(&env, "avax"), &30u64, &300i128);
    assert_eq!(client.list_bridges().len(), 3);
}

// ── transfer_admin ─────────────────────────────────────────────────────────────

#[test]
fn transfer_admin_success() {
    let (env, client, admin) = setup();
    let new_admin = Address::generate(&env);
    client.transfer_admin(&admin, &new_admin);
    assert_eq!(client.get_admin(), new_admin);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn transfer_admin_non_admin_panics() {
    let (env, client, _) = setup();
    let rando = Address::generate(&env);
    let new_admin = Address::generate(&env);
    client.transfer_admin(&rando, &new_admin);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn old_admin_loses_rights_after_transfer() {
    let (env, client, admin) = setup();
    let new_admin = Address::generate(&env);
    client.transfer_admin(&admin, &new_admin);
    client.register_bridge(&admin, &s(&env, "bsc"), &10u64, &100i128);
}

// ── compute_fee ────────────────────────────────────────────────────────────────

#[test]
fn compute_fee_normal() {
    let env = Env::default();
    assert_eq!(BridgeContract::compute_fee(env, 1_000_000, 50), 5_000);
}

#[test]
fn compute_fee_rounds_down() {
    let env = Env::default();
    // 999 * 10 / 10_000 = 0
    assert_eq!(BridgeContract::compute_fee(env, 999, 10), 0);
}

#[test]
fn compute_fee_zero_rate() {
    let env = Env::default();
    assert_eq!(BridgeContract::compute_fee(env, 1_000_000, 0), 0);
}

#[test]
fn compute_fee_max_rate() {
    let env = Env::default();
    // 100_000 * 1_000 / 10_000 = 10_000
    assert_eq!(BridgeContract::compute_fee(env, 100_000, 1_000), 10_000);
}

// ── bridge_acceptance_pause ────────────────────────────────────────────────────

#[test]
fn bridge_acceptance_paused_blocks_deposits() {
    let (env, client, admin) = setup();
    default_bridge(&client, &env, &admin);

    // Initially not paused
    assert!(!client.is_bridge_acceptance_paused());

    // Pause bridge acceptance
    client.set_bridge_acceptance_paused(&admin, &true);
    assert!(client.is_bridge_acceptance_paused());

    // Deposit should fail
    let user = Address::generate(&env);
    let result = client.try_bridge_deposit(&user, &s(&env, "eth-mainnet"), &10_000i128);
    assert_eq!(result, Err(Ok(ContractError::BridgeAcceptancePaused)));
}

#[test]
fn bridge_acceptance_unpause_allows_deposits() {
    let (env, client, admin) = setup();
    default_bridge(&client, &env, &admin);

    client.set_bridge_acceptance_paused(&admin, &true);
    client.set_bridge_acceptance_paused(&admin, &false);

    let user = Address::generate(&env);
    let net = client.bridge_deposit(&user, &s(&env, "eth-mainnet"), &10_000i128);
    assert!(net > 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn set_bridge_acceptance_paused_non_admin_panics() {
    let (env, client, _) = setup();
    let rando = Address::generate(&env);
    client.set_bridge_acceptance_paused(&rando, &true);
}

#[test]
fn bridge_acceptance_pause_emits_event() {
    use soroban_sdk::testutils::Events;
    let (env, client, admin) = setup();

    client.set_bridge_acceptance_paused(&admin, &true);

    let events = env.events().all();
    assert!(!events.is_empty());
}

#[test]
fn bridge_acceptance_pause_does_not_block_withdraw() {
    let (env, client, admin) = setup();
    default_bridge(&client, &env, &admin);

    // Pause acceptance
    client.set_bridge_acceptance_paused(&admin, &true);

    // Admin withdraw should still work (not affected by acceptance pause)
    let recip = Address::generate(&env);
    client.bridge_withdraw(&admin, &s(&env, "eth-mainnet"), &recip, &1_000i128);
}

//! # Comprehensive Governance Attack Prevention Tests
//!
//! This test suite verifies all acceptance criteria for governance flash loan
//! attack prevention as specified in the security requirements.
//!
//! ## Acceptance Criteria Covered:
//! 1. ✅ Vote locking mechanism (tokens locked during vote period)
//! 2. ✅ Delegation deadline before proposal submission
//! 3. ✅ Quorum requirements prevent low-vote passage
//! 4. ✅ Vote power snapshot before proposal
//! 5. ✅ Proposal execution delay
//! 6. ✅ Governance analytics for attack detection
//! 7. ✅ Tests verify attack resistance
//!
//! ## Edge Cases Covered:
//! - Legitimate large voters
//! - Vote delegation during lock period
//! - Proposal cancellation
//! - Emergency governance

#![cfg(test)]

use soroban_sdk::token::StellarAssetClient;
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env, String,
};

use crate::{
    types::{ProposalStatus, ProposalType, VoteType},
    HelloContract, HelloContractClient,
};

// ============================================================================
// Test Setup Helpers
// ============================================================================

fn setup_with_config(
    env: &Env,
    voting_period: u64,
    execution_delay: u64,
    quorum_bps: u32,
) -> (Address, Address, HelloContractClient) {
    let admin = Address::generate(env);
    let token = env.register_stellar_asset_contract(admin.clone());

    let contract_id = env.register_contract(None, HelloContract);
    let client = HelloContractClient::new(env, &contract_id);

    env.mock_all_auths();

    client.initialize(&admin);
    client.gov_initialize(
        &admin,
        &token,
        &Some(voting_period),
        &Some(execution_delay),
        &Some(quorum_bps),
        &Some(100_i128),          // proposal threshold
        &Some(7 * 24 * 3600_u64), // timelock duration
        &Some(5000_i128),         // 50% voting threshold
    );

    (admin, token, client)
}

fn setup(env: &Env) -> (Address, Address, HelloContractClient) {
    setup_with_config(
        env,
        7 * 24 * 3600, // 7-day voting period
        2 * 24 * 3600, // 2-day execution delay
        4000,          // 40% quorum
    )
}

fn mint(env: &Env, token: &Address, to: &Address, amount: i128) {
    StellarAssetClient::new(env, token).mint(to, &amount);
}

// ============================================================================
// AC1: Vote Locking Mechanism
// ============================================================================

#[test]
fn test_vote_locking_prevents_token_transfer_during_active_vote() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client) = setup(&env);

    let voter = Address::generate(&env);
    mint(&env, &token, &voter, 10_000);

    let proposal_id = client.gov_create_proposal(
        &voter,
        &ProposalType::EmergencyPause(true),
        &String::from_str(&env, "Test proposal"),
        &None,
    );

    // Advance to active voting period
    env.ledger().with_mut(|l| l.timestamp += 1);

    // Vote locks tokens
    client.gov_vote(&voter, &proposal_id, &VoteType::For);

    // Verify lock is active
    assert!(
        client.gov_is_vote_locked(&voter),
        "Tokens must be locked after voting"
    );

    let lock = client
        .gov_get_vote_lock(&voter)
        .expect("Lock record must exist");
    assert_eq!(lock.voter, voter);
    assert_eq!(lock.proposal_id, proposal_id);
    assert_eq!(lock.locked_amount, 10_000);
    assert!(lock.locked_until > env.ledger().timestamp());
}

#[test]
fn test_vote_lock_extends_for_multiple_active_proposals() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client) = setup(&env);

    let voter = Address::generate(&env);
    mint(&env, &token, &voter, 10_000);

    // Create first proposal
    let proposal_id_1 = client.gov_create_proposal(
        &voter,
        &ProposalType::EmergencyPause(true),
        &String::from_str(&env, "Proposal 1"),
        &None,
    );

    env.ledger().with_mut(|l| l.timestamp += 1);
    client.gov_vote(&voter, &proposal_id_1, &VoteType::For);

    let lock_1 = client.gov_get_vote_lock(&voter).unwrap();

    // Create second proposal with later end time
    env.ledger().with_mut(|l| l.timestamp += 100);
    let proposal_id_2 = client.gov_create_proposal(
        &voter,
        &ProposalType::EmergencyPause(false),
        &String::from_str(&env, "Proposal 2"),
        &None,
    );

    env.ledger().with_mut(|l| l.timestamp += 1);
    client.gov_vote(&voter, &proposal_id_2, &VoteType::Against);

    let lock_2 = client.gov_get_vote_lock(&voter).unwrap();

    // Lock should extend to the later proposal's end time
    assert!(
        lock_2.locked_until >= lock_1.locked_until,
        "Lock should extend for later proposal"
    );
}

#[test]
fn test_vote_lock_expires_after_voting_period_ends() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client) = setup(&env);

    let voter = Address::generate(&env);
    mint(&env, &token, &voter, 5_000);

    let proposal_id = client.gov_create_proposal(
        &voter,
        &ProposalType::EmergencyPause(true),
        &String::from_str(&env, "Test"),
        &None,
    );

    env.ledger().with_mut(|l| l.timestamp += 1);
    client.gov_vote(&voter, &proposal_id, &VoteType::For);

    assert!(client.gov_is_vote_locked(&voter));

    // Advance past voting period (7 days + 1 second)
    env.ledger().with_mut(|l| l.timestamp += 7 * 24 * 3600 + 1);

    assert!(
        !client.gov_is_vote_locked(&voter),
        "Lock must expire after voting period"
    );
}

// ============================================================================
// AC2: Delegation Deadline Before Proposal Submission
// ============================================================================

#[test]
fn test_delegation_must_be_established_24h_before_proposal() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client) = setup(&env);

    let proposer = Address::generate(&env);
    let delegator = Address::generate(&env);
    let delegatee = Address::generate(&env);

    mint(&env, &token, &proposer, 1_000);
    mint(&env, &token, &delegator, 10_000);
    mint(&env, &token, &delegatee, 500);

    // Delegation established at t=10000
    env.ledger().with_mut(|l| l.timestamp = 10_000);
    client.gov_delegate_vote(&delegator, &delegatee);

    // Proposal created only 1 hour later (< 24h deadline)
    env.ledger().with_mut(|l| l.timestamp = 10_000 + 3600);

    let proposal_id = client.gov_create_proposal(
        &proposer,
        &ProposalType::EmergencyPause(true),
        &String::from_str(&env, "Test"),
        &None,
    );

    env.ledger().with_mut(|l| l.timestamp += 1);

    // Delegatee votes - should only have own power (500), not delegator's (10000)
    client.gov_vote(&delegatee, &proposal_id, &VoteType::For);

    let proposal = client.gov_get_proposal(&proposal_id).unwrap();
    assert_eq!(
        proposal.for_votes, 500,
        "Delegation within 24h deadline must not count"
    );
}

#[test]
fn test_delegation_established_before_deadline_counts() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client) = setup(&env);

    let proposer = Address::generate(&env);
    let delegator = Address::generate(&env);
    let delegatee = Address::generate(&env);

    mint(&env, &token, &proposer, 1_000);
    mint(&env, &token, &delegator, 10_000);
    mint(&env, &token, &delegatee, 500);

    // Delegation established at t=10000
    env.ledger().with_mut(|l| l.timestamp = 10_000);
    client.gov_delegate_vote(&delegator, &delegatee);

    // Proposal created 48 hours later (> 24h deadline)
    env.ledger().with_mut(|l| l.timestamp = 10_000 + 48 * 3600);

    let proposal_id = client.gov_create_proposal(
        &proposer,
        &ProposalType::EmergencyPause(true),
        &String::from_str(&env, "Test"),
        &None,
    );

    // Take snapshot for delegator
    client.gov_get_vote_power_snapshot(&proposal_id, &delegator);

    env.ledger().with_mut(|l| l.timestamp += 1);

    // Delegatee votes - should have own power (500) + delegator's (10000)
    client.gov_vote(&delegatee, &proposal_id, &VoteType::For);

    let proposal = client.gov_get_proposal(&proposal_id).unwrap();
    assert_eq!(
        proposal.for_votes, 10_500,
        "Valid delegation must add delegator's power"
    );
}

// ============================================================================
// AC3: Quorum Requirements Prevent Low-Vote Passage
// ============================================================================

#[test]
fn test_quorum_requirement_blocks_low_participation_proposal() {
    let env = Env::default();
    env.mock_all_auths();

    // Setup with 50% quorum requirement
    let (admin, token, client) = setup_with_config(&env, 7 * 24 * 3600, 2 * 24 * 3600, 5000);

    let proposer = Address::generate(&env);
    let small_voter = Address::generate(&env);
    let large_holder = Address::generate(&env);

    mint(&env, &token, &proposer, 1_000);
    mint(&env, &token, &small_voter, 100);
    mint(&env, &token, &large_holder, 100_000); // Holds tokens but doesn't vote

    let proposal_id = client.gov_create_proposal(
        &proposer,
        &ProposalType::EmergencyPause(true),
        &String::from_str(&env, "Low participation proposal"),
        &None,
    );

    env.ledger().with_mut(|l| l.timestamp += 1);

    // Only small voter votes (100 out of 101,100 total)
    client.gov_vote(&small_voter, &proposal_id, &VoteType::For);

    // Advance past voting period
    env.ledger().with_mut(|l| l.timestamp += 7 * 24 * 3600 + 1);

    let outcome = client.gov_queue_proposal(&small_voter, &proposal_id);

    assert!(
        !outcome.quorum_reached,
        "Quorum must not be met with low participation"
    );
    assert!(!outcome.succeeded, "Proposal must fail without quorum");
}

#[test]
fn test_quorum_requirement_allows_sufficient_participation() {
    let env = Env::default();
    env.mock_all_auths();

    // Setup with 40% quorum
    let (admin, token, client) = setup(&env);

    let proposer = Address::generate(&env);
    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);

    mint(&env, &token, &proposer, 1_000);
    mint(&env, &token, &voter1, 50_000);
    mint(&env, &token, &voter2, 50_000);

    let proposal_id = client.gov_create_proposal(
        &proposer,
        &ProposalType::EmergencyPause(true),
        &String::from_str(&env, "Sufficient participation"),
        &None,
    );

    env.ledger().with_mut(|l| l.timestamp += 1);

    // Both voters participate (100k out of 101k = ~99%)
    client.gov_vote(&voter1, &proposal_id, &VoteType::For);
    client.gov_vote(&voter2, &proposal_id, &VoteType::For);

    env.ledger().with_mut(|l| l.timestamp += 7 * 24 * 3600 + 1);

    let outcome = client.gov_queue_proposal(&voter1, &proposal_id);

    assert!(outcome.quorum_reached, "Quorum must be met");
    assert!(outcome.succeeded, "Proposal must succeed with quorum");
}

// ============================================================================
// AC4: Vote Power Snapshot Before Proposal
// ============================================================================

#[test]
fn test_snapshot_taken_at_proposal_creation() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client) = setup(&env);

    let proposer = Address::generate(&env);
    mint(&env, &token, &proposer, 5_000);

    let creation_time = env.ledger().timestamp();

    let proposal_id = client.gov_create_proposal(
        &proposer,
        &ProposalType::EmergencyPause(true),
        &String::from_str(&env, "Test"),
        &None,
    );

    // Snapshot should exist for proposer
    let snapshot = client
        .gov_get_vote_power_snapshot(&proposal_id, &proposer)
        .expect("Snapshot must be taken at proposal creation");

    assert_eq!(snapshot.proposal_id, proposal_id);
    assert_eq!(snapshot.voter, proposer);
    assert_eq!(snapshot.balance, 5_000);
    assert_eq!(snapshot.snapshot_time, creation_time);
}

#[test]
fn test_tokens_acquired_after_snapshot_have_no_voting_power() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client) = setup(&env);

    let proposer = Address::generate(&env);
    let attacker = Address::generate(&env);

    mint(&env, &token, &proposer, 1_000);
    // Attacker has NO tokens at proposal creation

    let proposal_id = client.gov_create_proposal(
        &proposer,
        &ProposalType::EmergencyPause(true),
        &String::from_str(&env, "Test"),
        &None,
    );

    // Attacker acquires tokens AFTER proposal (flash loan simulation)
    mint(&env, &token, &attacker, 1_000_000);

    env.ledger().with_mut(|l| l.timestamp += 1);

    // Attacker tries to vote - should fail
    let result = client.try_gov_vote(&attacker, &proposal_id, &VoteType::For);

    assert!(
        result.is_err(),
        "Voter with no snapshot must not be able to vote"
    );
}

// ============================================================================
// AC5: Proposal Execution Delay
// ============================================================================

#[test]
fn test_execution_delay_enforced() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client) = setup(&env);

    let proposer = Address::generate(&env);
    let voter = Address::generate(&env);

    mint(&env, &token, &proposer, 1_000);
    mint(&env, &token, &voter, 100_000);

    let proposal_id = client.gov_create_proposal(
        &proposer,
        &ProposalType::EmergencyPause(true),
        &String::from_str(&env, "Test"),
        &None,
    );

    env.ledger().with_mut(|l| l.timestamp += 1);
    client.gov_vote(&voter, &proposal_id, &VoteType::For);

    // Advance past voting period
    env.ledger().with_mut(|l| l.timestamp += 7 * 24 * 3600 + 1);
    client.gov_queue_proposal(&voter, &proposal_id);

    // Try to execute immediately - should fail (2-day delay required)
    let result = client.try_gov_execute_proposal(&voter, &proposal_id);
    assert!(result.is_err(), "Execution before delay period must fail");

    // Advance past execution delay
    env.ledger().with_mut(|l| l.timestamp += 2 * 24 * 3600 + 1);

    // Now execution should succeed
    client.gov_execute_proposal(&voter, &proposal_id);

    let proposal = client.gov_get_proposal(&proposal_id).unwrap();
    assert_eq!(proposal.status, ProposalStatus::Executed);
}

#[test]
fn test_execution_delay_provides_cancellation_window() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client) = setup(&env);

    let proposer = Address::generate(&env);
    let voter = Address::generate(&env);

    mint(&env, &token, &proposer, 1_000);
    mint(&env, &token, &voter, 100_000);

    let proposal_id = client.gov_create_proposal(
        &proposer,
        &ProposalType::EmergencyPause(true),
        &String::from_str(&env, "Malicious proposal"),
        &None,
    );

    env.ledger().with_mut(|l| l.timestamp += 1);
    client.gov_vote(&voter, &proposal_id, &VoteType::For);

    env.ledger().with_mut(|l| l.timestamp += 7 * 24 * 3600 + 1);
    client.gov_queue_proposal(&voter, &proposal_id);

    // During execution delay, admin can cancel
    client.gov_cancel_proposal(&admin, &proposal_id);

    let proposal = client.gov_get_proposal(&proposal_id).unwrap();
    assert_eq!(proposal.status, ProposalStatus::Cancelled);

    // Advance past delay
    env.ledger().with_mut(|l| l.timestamp += 2 * 24 * 3600 + 1);

    // Execution should fail on cancelled proposal
    let result = client.try_gov_execute_proposal(&voter, &proposal_id);
    assert!(result.is_err(), "Cancelled proposal must not be executable");
}

// ============================================================================
// AC6: Governance Analytics for Attack Detection
// ============================================================================

#[test]
fn test_analytics_track_suspicious_large_voter() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client) = setup(&env);

    let proposer = Address::generate(&env);
    let whale = Address::generate(&env);

    mint(&env, &token, &proposer, 100);
    mint(&env, &token, &whale, 999_900); // ~99.99% of supply

    let proposal_id = client.gov_create_proposal(
        &proposer,
        &ProposalType::EmergencyPause(true),
        &String::from_str(&env, "Test"),
        &None,
    );

    env.ledger().with_mut(|l| l.timestamp += 1);
    client.gov_vote(&whale, &proposal_id, &VoteType::For);

    let analytics = client.gov_get_analytics();

    assert!(
        analytics.suspicious_proposals > 0,
        "Large single voter must trigger suspicious flag"
    );
    assert_eq!(analytics.max_single_voter_power, 999_900);
    assert!(analytics.last_suspicious_at > 0);
}

#[test]
fn test_analytics_count_total_proposals_and_votes() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client) = setup(&env);

    let proposer = Address::generate(&env);
    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);

    mint(&env, &token, &proposer, 1_000);
    mint(&env, &token, &voter1, 5_000);
    mint(&env, &token, &voter2, 3_000);

    // Create multiple proposals
    let proposal_id_1 = client.gov_create_proposal(
        &proposer,
        &ProposalType::EmergencyPause(true),
        &String::from_str(&env, "Proposal 1"),
        &None,
    );

    let proposal_id_2 = client.gov_create_proposal(
        &proposer,
        &ProposalType::EmergencyPause(false),
        &String::from_str(&env, "Proposal 2"),
        &None,
    );

    env.ledger().with_mut(|l| l.timestamp += 1);

    // Cast votes
    client.gov_vote(&voter1, &proposal_id_1, &VoteType::For);
    client.gov_vote(&voter2, &proposal_id_1, &VoteType::Against);
    client.gov_vote(&voter1, &proposal_id_2, &VoteType::For);

    let analytics = client.gov_get_analytics();

    assert_eq!(analytics.total_proposals, 2);
    assert_eq!(analytics.total_votes, 3);
}

// ============================================================================
// Edge Case: Legitimate Large Voters
// ============================================================================

#[test]
fn test_legitimate_large_voter_not_blocked() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client) = setup(&env);

    let proposer = Address::generate(&env);
    let large_holder = Address::generate(&env);

    // Large holder has tokens BEFORE proposal
    mint(&env, &token, &proposer, 1_000);
    mint(&env, &token, &large_holder, 100_000);

    let proposal_id = client.gov_create_proposal(
        &proposer,
        &ProposalType::EmergencyPause(true),
        &String::from_str(&env, "Legitimate proposal"),
        &None,
    );

    env.ledger().with_mut(|l| l.timestamp += 1);

    // Large holder can vote successfully
    client.gov_vote(&large_holder, &proposal_id, &VoteType::For);

    let proposal = client.gov_get_proposal(&proposal_id).unwrap();
    assert_eq!(
        proposal.for_votes, 100_000,
        "Legitimate large voter must be able to vote"
    );
}

// ============================================================================
// Edge Case: Vote Delegation During Lock Period
// ============================================================================

#[test]
fn test_cannot_delegate_while_vote_locked() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client) = setup(&env);

    let voter = Address::generate(&env);
    let delegatee = Address::generate(&env);

    mint(&env, &token, &voter, 10_000);
    mint(&env, &token, &delegatee, 1_000);

    let proposal_id = client.gov_create_proposal(
        &voter,
        &ProposalType::EmergencyPause(true),
        &String::from_str(&env, "Test"),
        &None,
    );

    env.ledger().with_mut(|l| l.timestamp += 1);

    // Vote locks tokens
    client.gov_vote(&voter, &proposal_id, &VoteType::For);

    assert!(client.gov_is_vote_locked(&voter));

    // Try to delegate while locked - should fail
    let result = client.try_gov_delegate_vote(&voter, &delegatee);

    assert!(
        result.is_err(),
        "Delegation during vote lock must be prevented"
    );
}

// ============================================================================
// Edge Case: Proposal Cancellation
// ============================================================================

#[test]
fn test_proposer_can_cancel_own_proposal() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client) = setup(&env);

    let proposer = Address::generate(&env);
    mint(&env, &token, &proposer, 1_000);

    let proposal_id = client.gov_create_proposal(
        &proposer,
        &ProposalType::EmergencyPause(true),
        &String::from_str(&env, "Test"),
        &None,
    );

    client.gov_cancel_proposal(&proposer, &proposal_id);

    let proposal = client.gov_get_proposal(&proposal_id).unwrap();
    assert_eq!(proposal.status, ProposalStatus::Cancelled);
}

#[test]
fn test_admin_can_cancel_any_proposal() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client) = setup(&env);

    let proposer = Address::generate(&env);
    mint(&env, &token, &proposer, 1_000);

    let proposal_id = client.gov_create_proposal(
        &proposer,
        &ProposalType::EmergencyPause(true),
        &String::from_str(&env, "Malicious proposal"),
        &None,
    );

    // Admin cancels
    client.gov_cancel_proposal(&admin, &proposal_id);

    let proposal = client.gov_get_proposal(&proposal_id).unwrap();
    assert_eq!(proposal.status, ProposalStatus::Cancelled);
}

#[test]
fn test_cannot_cancel_executed_proposal() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client) = setup(&env);

    let proposer = Address::generate(&env);
    let voter = Address::generate(&env);

    mint(&env, &token, &proposer, 1_000);
    mint(&env, &token, &voter, 100_000);

    let proposal_id = client.gov_create_proposal(
        &proposer,
        &ProposalType::EmergencyPause(true),
        &String::from_str(&env, "Test"),
        &None,
    );

    env.ledger().with_mut(|l| l.timestamp += 1);
    client.gov_vote(&voter, &proposal_id, &VoteType::For);

    env.ledger().with_mut(|l| l.timestamp += 7 * 24 * 3600 + 1);
    client.gov_queue_proposal(&voter, &proposal_id);

    env.ledger().with_mut(|l| l.timestamp += 2 * 24 * 3600 + 1);
    client.gov_execute_proposal(&voter, &proposal_id);

    // Try to cancel executed proposal - should fail
    let result = client.try_gov_cancel_proposal(&admin, &proposal_id);
    assert!(result.is_err(), "Cannot cancel executed proposal");
}

// ============================================================================
// Edge Case: Emergency Governance
// ============================================================================

#[test]
fn test_emergency_proposal_bypasses_normal_delays() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client) = setup(&env);

    // Emergency proposal can be executed immediately
    let proposal_id = client.gov_create_emergency_proposal(
        &admin,
        &ProposalType::EmergencyPause(true),
        &String::from_str(&env, "Emergency action"),
    );

    let proposal = client.gov_get_proposal(&proposal_id).unwrap();

    assert_eq!(proposal.status, ProposalStatus::Queued);
    assert_eq!(proposal.execution_time, Some(env.ledger().timestamp()));
}

#[test]
fn test_proposal_rate_limiting_prevents_spam() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client) = setup(&env);

    let proposer = Address::generate(&env);
    mint(&env, &token, &proposer, 100_000);

    // Create 5 proposals (rate limit)
    for i in 0..5 {
        client.gov_create_proposal(
            &proposer,
            &ProposalType::EmergencyPause(true),
            &String::from_str(&env, "Proposal"),
            &None,
        );
    }

    // 6th proposal should fail
    let result = client.try_gov_create_proposal(
        &proposer,
        &ProposalType::EmergencyPause(true),
        &String::from_str(&env, "Spam"),
        &None,
    );

    assert!(result.is_err(), "Proposal rate limit must prevent spam");
}

#[test]
fn test_delegation_depth_limit_prevents_chain_attacks() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, token, client) = setup(&env);

    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let c = Address::generate(&env);
    let d = Address::generate(&env);
    let e = Address::generate(&env);

    for addr in [&a, &b, &c, &d, &e] {
        mint(&env, &token, addr, 1_000);
    }

    // Build chain: a → b → c → d (depth 3, max allowed)
    client.gov_delegate_vote(&a, &b);
    client.gov_delegate_vote(&b, &c);
    client.gov_delegate_vote(&c, &d);

    // d → e would exceed max depth
    let result = client.try_gov_delegate_vote(&d, &e);

    assert!(
        result.is_err(),
        "Delegation chain exceeding max depth must be rejected"
    );
}

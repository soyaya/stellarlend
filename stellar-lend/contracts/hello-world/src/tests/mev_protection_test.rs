use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, Env};

use crate::mev_protection::{
    create_commit, execution_hint, get_commit, get_ordering_stats, reveal_borrow, user_guidance,
    MevProtectionError, SensitiveOperation, TxOrderingHint,
};
use crate::HelloContract;

fn setup_contract(env: &Env) -> Address {
    env.register(HelloContract, ())
}

#[test]
fn test_commit_reveal_requires_delay() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = setup_contract(&env);
    let user = Address::generate(&env);
    let asset = Address::generate(&env);

    let commit_id = env.as_contract(&contract_id, || {
        create_commit(
            &env,
            user.clone(),
            SensitiveOperation::Borrow,
            Some(asset),
            None,
            None,
            500,
            100,
            TxOrderingHint::Default,
        )
        .unwrap()
    });

    let err = env
        .as_contract(&contract_id, || reveal_borrow(&env, user, commit_id))
        .unwrap_err();
    assert_eq!(err, MevProtectionError::CommitNotReady);
}

#[test]
fn test_commit_expires_after_window() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = setup_contract(&env);
    let user = Address::generate(&env);
    let asset = Address::generate(&env);

    let commit_id = env.as_contract(&contract_id, || {
        create_commit(
            &env,
            user.clone(),
            SensitiveOperation::Borrow,
            Some(asset),
            None,
            None,
            500,
            100,
            TxOrderingHint::PrivateMempool,
        )
        .unwrap()
    });

    env.ledger().with_mut(|li| li.timestamp = 301);

    let err = env
        .as_contract(&contract_id, || reveal_borrow(&env, user, commit_id))
        .unwrap_err();
    assert_eq!(err, MevProtectionError::CommitExpired);
}

#[test]
fn test_fee_cap_blocks_surge_execution() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = setup_contract(&env);
    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);
    let asset = Address::generate(&env);

    let first = env.as_contract(&contract_id, || {
        create_commit(
            &env,
            user_a.clone(),
            SensitiveOperation::Borrow,
            Some(asset.clone()),
            None,
            None,
            1_000,
            100,
            TxOrderingHint::Default,
        )
        .unwrap()
    });
    let second = env.as_contract(&contract_id, || {
        create_commit(
            &env,
            user_b.clone(),
            SensitiveOperation::Borrow,
            Some(asset),
            None,
            None,
            1_000,
            5,
            TxOrderingHint::Default,
        )
        .unwrap()
    });

    env.ledger().with_mut(|li| li.timestamp = 31);
    env.as_contract(&contract_id, || reveal_borrow(&env, user_a, first))
        .unwrap();
    env.ledger().with_mut(|li| li.timestamp = 32);

    let err = env
        .as_contract(&contract_id, || reveal_borrow(&env, user_b, second))
        .unwrap_err();
    assert_eq!(err, MevProtectionError::FeeCapExceeded);
}

#[test]
fn test_sandwich_pattern_updates_monitoring_stats() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = setup_contract(&env);
    let attacker = Address::generate(&env);
    let victim = Address::generate(&env);
    let asset = Address::generate(&env);

    let first = env.as_contract(&contract_id, || {
        create_commit(
            &env,
            attacker.clone(),
            SensitiveOperation::Borrow,
            Some(asset.clone()),
            None,
            None,
            2_000,
            100,
            TxOrderingHint::PrivateMempool,
        )
        .unwrap()
    });
    let middle = env.as_contract(&contract_id, || {
        create_commit(
            &env,
            victim.clone(),
            SensitiveOperation::Borrow,
            Some(asset.clone()),
            None,
            None,
            2_050,
            100,
            TxOrderingHint::Default,
        )
        .unwrap()
    });
    let last = env.as_contract(&contract_id, || {
        create_commit(
            &env,
            attacker.clone(),
            SensitiveOperation::Borrow,
            Some(asset),
            None,
            None,
            2_010,
            100,
            TxOrderingHint::BatchAuction,
        )
        .unwrap()
    });

    env.ledger().with_mut(|li| li.timestamp = 31);
    env.as_contract(&contract_id, || {
        reveal_borrow(&env, attacker.clone(), first)
    })
    .unwrap();
    env.ledger().with_mut(|li| li.timestamp = 32);
    env.as_contract(&contract_id, || reveal_borrow(&env, victim, middle))
        .unwrap();
    env.ledger().with_mut(|li| li.timestamp = 33);
    env.as_contract(&contract_id, || reveal_borrow(&env, attacker, last))
        .unwrap();

    let stats = env.as_contract(&contract_id, || get_ordering_stats(&env));
    assert!(stats.suspicious_sequences >= 2);
    assert!(stats.sandwich_alerts >= 1);
}

#[test]
fn test_guidance_hint_and_commit_lookup() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = setup_contract(&env);
    let user = Address::generate(&env);

    let hint = env.as_contract(&contract_id, || {
        execution_hint(&env, TxOrderingHint::Default)
    });
    assert_eq!(hint, TxOrderingHint::PrivateMempool);

    let msg = env.as_contract(&contract_id, || {
        user_guidance(&env, SensitiveOperation::Liquidate)
    });
    assert!(!msg.is_empty());

    let commit_id = env.as_contract(&contract_id, || {
        create_commit(
            &env,
            user.clone(),
            SensitiveOperation::Withdraw,
            None,
            None,
            None,
            100,
            100,
            TxOrderingHint::DelayedReveal,
        )
        .unwrap()
    });
    let commit = env
        .as_contract(&contract_id, || get_commit(&env, commit_id))
        .unwrap();
    assert_eq!(commit.owner, user);
}

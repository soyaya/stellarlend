//! Rate limiter integration tests.
//!
//! Validates that borrow/liquidate entrypoints enforce per-user and global-per-pool
//! limits, including burst/grace and admin bypass.

#![cfg(test)]

use crate::rate_limiter::RateLimitConfig;
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
fn test_borrow_rate_limited_per_user() {
    let env = Env::default();
    env.mock_all_auths();
    let (_cid, admin, client) = setup(&env);
    let user = Address::generate(&env);

    // Large collateral so we don't hit collateral ratio limits first.
    client.deposit_collateral(&user, &None, &1_000_000_000);

    // Tight limit: 2 calls per 60s, no burst.
    client.configure_rate_limit_operation(
        &admin,
        &Symbol::new(&env, "borrow"),
        &RateLimitConfig {
            window_seconds: 60,
            max_calls_per_window: 2,
            burst_calls: 0,
            grace_burst_calls: 0,
        },
    );

    env.ledger().with_mut(|li| li.timestamp = 1);
    assert!(client.borrow_asset(&user, &None, &1).is_ok());
    assert!(client.borrow_asset(&user, &None, &1).is_ok());

    // Third call in same window should be blocked.
    let res = client.try_borrow_asset(&user, &None, &1);
    assert!(res.is_err());
}

#[test]
fn test_borrow_global_pool_rate_limited() {
    let env = Env::default();
    env.mock_all_auths();
    let (_cid, admin, client) = setup(&env);
    let u1 = Address::generate(&env);
    let u2 = Address::generate(&env);

    client.deposit_collateral(&u1, &None, &1_000_000_000);
    client.deposit_collateral(&u2, &None, &1_000_000_000);

    // Global-per-pool limit uses the same config; set max=2 and observe u2 blocked.
    client.configure_rate_limit_operation(
        &admin,
        &Symbol::new(&env, "borrow"),
        &RateLimitConfig {
            window_seconds: 60,
            max_calls_per_window: 2,
            burst_calls: 0,
            grace_burst_calls: 0,
        },
    );

    env.ledger().with_mut(|li| li.timestamp = 1);
    assert!(client.borrow_asset(&u1, &None, &1).is_ok());
    assert!(client.borrow_asset(&u1, &None, &1).is_ok());

    // Global bucket should be empty now, so u2 fails even though its user bucket is fresh.
    let res = client.try_borrow_asset(&u2, &None, &1);
    assert!(res.is_err());
}

#[test]
fn test_grace_burst_allows_extra_borrows() {
    let env = Env::default();
    env.mock_all_auths();
    let (_cid, admin, client) = setup(&env);
    let user = Address::generate(&env);
    client.deposit_collateral(&user, &None, &1_000_000_000);

    client.configure_rate_limit_operation(
        &admin,
        &Symbol::new(&env, "borrow"),
        &RateLimitConfig {
            window_seconds: 60,
            max_calls_per_window: 1,
            burst_calls: 0,
            grace_burst_calls: 2,
        },
    );

    // Enable grace for user.
    client.set_user_rate_limit_grace(&admin, &user, &Symbol::new(&env, "borrow"), &true);

    env.ledger().with_mut(|li| li.timestamp = 1);
    assert!(client.borrow_asset(&user, &None, &1).is_ok());
    assert!(client.borrow_asset(&user, &None, &1).is_ok());
    assert!(client.borrow_asset(&user, &None, &1).is_ok());

    // Fourth should fail (1 + grace_burst_calls(2) = 3 capacity).
    let res = client.try_borrow_asset(&user, &None, &1);
    assert!(res.is_err());
}

#[test]
fn test_admin_bypass() {
    let env = Env::default();
    env.mock_all_auths();
    let (_cid, admin, client) = setup(&env);

    // Configure absurdly small limits.
    client.configure_rate_limit_operation(
        &admin,
        &Symbol::new(&env, "borrow"),
        &RateLimitConfig {
            window_seconds: 60,
            max_calls_per_window: 1,
            burst_calls: 0,
            grace_burst_calls: 0,
        },
    );

    // Admin borrow call should bypass limits (even if it would otherwise be rate limited).
    client.deposit_collateral(&admin, &None, &1_000_000_000);
    env.ledger().with_mut(|li| li.timestamp = 1);
    assert!(client.borrow_asset(&admin, &None, &1).is_ok());
    assert!(client.borrow_asset(&admin, &None, &1).is_ok());
}


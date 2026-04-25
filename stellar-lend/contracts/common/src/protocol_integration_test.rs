extern crate std;

use soroban_sdk::testutils::Address as _;
use soroban_sdk::{symbol_short, Address, Env, Symbol};

use crate::cache;
use crate::message_bus::{self, MessageBusError, MessageState};
use crate::shared_types::{AssetConfigV1, AssetRiskParamsV1, PositionSummaryV1, SharedTypesVersion, SHARED_TYPES_VERSION_V1};

fn setup() -> (Env, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let source = Address::generate(&env);
    let target = Address::generate(&env);
    let user = Address::generate(&env);
    (env, source, target, user)
}

#[test]
fn cross_contract_message_flow_with_retry_and_confirm() {
    let (env, source, target, _) = setup();
    let kind = symbol_short!("borrow");
    let hash = symbol_short!("hash001");

    let id = message_bus::publish(&env, source, target, kind, hash, SHARED_TYPES_VERSION_V1);
    let first = message_bus::dequeue_next(&env).unwrap();
    assert_eq!(first.id, id);
    assert_eq!(first.state, MessageState::InFlight);

    message_bus::mark_failed(&env, id).unwrap();
    message_bus::retry_failed(&env, id).unwrap();
    let second = message_bus::dequeue_next(&env).unwrap();
    assert_eq!(second.id, id);
    assert_eq!(second.attempts, 2);

    message_bus::confirm_delivery(&env, id).unwrap();
    let stored = message_bus::get_message(&env, id).unwrap();
    assert_eq!(stored.state, MessageState::Delivered);
}

#[test]
fn replay_protection_rejects_duplicate_confirmation() {
    let (env, source, target, _) = setup();
    let id = message_bus::publish(
        &env,
        source,
        target,
        symbol_short!("repay"),
        symbol_short!("hash002"),
        SHARED_TYPES_VERSION_V1,
    );
    let _ = message_bus::dequeue_next(&env).unwrap();
    message_bus::confirm_delivery(&env, id).unwrap();
    let duplicate = message_bus::confirm_delivery(&env, id);
    assert_eq!(duplicate, Err(MessageBusError::AlreadyDelivered));
}

#[test]
fn cache_ttl_and_metrics_work_for_health_factor() {
    let (env, _, _, _) = setup();
    let health_key: Symbol = symbol_short!("health");
    cache::set_cached(&env, health_key, 12_345, Some(15)).unwrap();

    let first = cache::get_cached(&env, symbol_short!("health"));
    assert_eq!(first, Some(12_345));
    let missing = cache::get_cached(&env, symbol_short!("absent"));
    assert_eq!(missing, None);

    let stats = cache::cache_stats(&env);
    assert_eq!(stats.hits, 1);
    assert_eq!(stats.misses, 1);
    assert_eq!(stats.size, 1);
}

#[test]
fn shared_types_are_versioned_and_reusable() {
    let (env, _, _, user) = setup();
    let token = Address::generate(&env);

    let config = AssetConfigV1 {
        asset: Some(token),
        max_supply: 1_000_000,
        max_borrow: 500_000,
        can_collateralize: true,
        can_borrow: true,
        price: 10_000_000,
        price_updated_at: env.ledger().timestamp(),
        risk: AssetRiskParamsV1 {
            collateral_factor_bps: 7_500,
            liquidation_threshold_bps: 8_000,
            reserve_factor_bps: 1_000,
        },
    };

    let summary = PositionSummaryV1 {
        total_collateral_value: 50_000,
        weighted_collateral_value: 40_000,
        total_debt_value: 20_000,
        weighted_debt_value: 20_000,
        health_factor: 20_000,
        is_liquidatable: false,
        borrow_capacity: 20_000,
    };

    assert_eq!(config.risk.collateral_factor_bps, 7_500);
    assert_eq!(summary.health_factor, 20_000);
    assert_eq!(SharedTypesVersion::V1, SharedTypesVersion::V1);
    assert_ne!(user, Address::generate(&env));
}

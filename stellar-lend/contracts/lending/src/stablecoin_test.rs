use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env,
};

#[contract]
pub struct CustomizableMockOracle;

#[contractimpl]
impl CustomizableMockOracle {
    pub fn price(env: Env, asset: Address) -> i128 {
        env.storage()
            .instance()
            .get(&asset)
            .unwrap_or(100_000_000) // Default $1.00
    }

    pub fn set_price(env: Env, asset: Address, price: i128) {
        env.storage().instance().set(&asset, &price);
    }
}

fn setup(
    env: &Env,
) -> (
    LendingContractClient<'_>,
    Address,
    Address,
    Address,
    Address,
    Address,
) {
    let contract_id = env.register(LendingContract, ());
    let client = LendingContractClient::new(env, &contract_id);

    let admin = Address::generate(env);
    let user = Address::generate(env);
    let asset = Address::generate(env);
    let collateral_asset = Address::generate(env);
    let oracle_id = env.register(CustomizableMockOracle, ());

    client.initialize(&admin, &1_000_000_000, &1000);
    client.set_oracle(&admin, &oracle_id);

    (client, admin, user, asset, collateral_asset, oracle_id)
}

#[test]
fn test_stability_fee_applied_when_depegged() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, user, asset, collateral_asset, oracle_id) = setup(&env);
    let oracle_client = CustomizableMockOracleClient::new(&env, &oracle_id);

    // Initial price $1.00
    oracle_client.set_price(&asset, &100_000_000);

    // Configure stablecoin: $1.00 peg, 1% threshold, 2% stability fee
    let config = StablecoinConfig {
        target_price: 100_000_000,
        peg_threshold_bps: 100, // 1%
        stability_fee_bps: 200,  // 2%
        emergency_threshold_bps: 1000, // 10%
    };
    client.set_stablecoin_config(&admin, &asset, &config);

    // Borrow 100,000 units
    env.ledger().with_mut(|li| li.timestamp = 1000);
    client.borrow(&user, &asset, &100_000, &collateral_asset, &200_000);

    // 1 year later, price is $0.985 (1.5% deviation > 1% threshold)
    oracle_client.set_price(&asset, &98_500_000);
    env.ledger().with_mut(|li| li.timestamp = 1000 + 31_536_000);

    let debt = client.get_user_debt(&user);
    // Base rate is 500 bps (5%), stability fee is 200 bps (2%)
    // Total interest = 7% of 100,000 = 7,000
    assert!(debt.interest_accrued >= 6900 && debt.interest_accrued <= 7100, "Interest accrued: {}", debt.interest_accrued);
}

#[test]
fn test_no_stability_fee_when_within_threshold() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, user, asset, collateral_asset, oracle_id) = setup(&env);
    let oracle_client = CustomizableMockOracleClient::new(&env, &oracle_id);

    // Configure stablecoin: $1.00 peg, 1% threshold
    let config = StablecoinConfig {
        target_price: 100_000_000,
        peg_threshold_bps: 100, // 1%
        stability_fee_bps: 200,
        emergency_threshold_bps: 1000,
    };
    client.set_stablecoin_config(&admin, &asset, &config);

    oracle_client.set_price(&asset, &100_000_000);
    env.ledger().with_mut(|li| li.timestamp = 1000);
    client.borrow(&user, &asset, &100_000, &collateral_asset, &200_000);

    // 1 year later, price is $0.995 (0.5% deviation < 1% threshold)
    oracle_client.set_price(&asset, &99_500_000);
    env.ledger().with_mut(|li| li.timestamp = 1000 + 31_536_000);

    let debt = client.get_user_debt(&user);
    // Only base rate 5% applied = 5,000
    assert!(debt.interest_accrued >= 4900 && debt.interest_accrued <= 5100, "Interest accrued: {}", debt.interest_accrued);
}

#[test]
fn test_get_set_stablecoin_config() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _user, asset, _collateral_asset, _oracle_id) = setup(&env);

    let config = StablecoinConfig {
        target_price: 100_000_000,
        peg_threshold_bps: 100,
        stability_fee_bps: 200,
        emergency_threshold_bps: 1000,
    };

    client.set_stablecoin_config(&admin, &asset, &config);
    let fetched = client.get_stablecoin_config(&asset).unwrap();
    assert_eq!(fetched.target_price, 100_000_000);
    assert_eq!(fetched.peg_threshold_bps, 100);
    assert_eq!(fetched.stability_fee_bps, 200);
}

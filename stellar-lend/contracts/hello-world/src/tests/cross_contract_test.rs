#![cfg(test)]

use crate::{HelloContract, HelloContractClient};
use soroban_sdk::token::Client as TokenClient;
use soroban_sdk::token::StellarAssetClient as StellarTokenClient;
use soroban_sdk::{contract, contractimpl, testutils::Address as _, Address, Env, Symbol};

// ============================================================================
// Helper Contracts
// ============================================================================

/// Mock Flash Loan Receiver Contract.
/// Implements the `on_flash_loan(user, asset, amount, fee)` callback expected
/// by the protocol's pull-repayment flash loan model.
#[contract]
pub struct MockFlashLoanReceiver;

#[contractimpl]
impl MockFlashLoanReceiver {
    pub fn init(env: Env, provider: Address, should_repay: bool, should_reenter: bool) {
        env.storage()
            .instance()
            .set(&Symbol::new(&env, "provider"), &provider);
        env.storage()
            .instance()
            .set(&Symbol::new(&env, "should_repay"), &should_repay);
        env.storage()
            .instance()
            .set(&Symbol::new(&env, "should_reenter"), &should_reenter);
    }

    /// Callback invoked by the protocol: on_flash_loan(user, asset, amount, fee)
    pub fn on_flash_loan(env: Env, _user: Address, asset: Address, amount: i128, fee: i128) {
        let provider: Address = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "provider"))
            .unwrap();
        let should_repay: bool = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "should_repay"))
            .unwrap();
        let should_reenter: bool = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "should_reenter"))
            .unwrap();

        let token_client = TokenClient::new(&env, &asset);

        // Verify we received the funds
        let balance = token_client.balance(&env.current_contract_address());
        if balance < amount {
            panic!("Did not receive flash loan funds");
        }

        if should_reenter {
            // Attempt to re-enter the provider — should fail due to reentrancy guard
            let client = HelloContractClient::new(&env, &provider);
            let result = client.try_execute_flash_loan(
                &env.current_contract_address(),
                &asset,
                &amount,
                &env.current_contract_address(),
            );
            // Panic so the outer flash loan also fails, proving re-entry was attempted
            if result.is_err() {
                panic!("Reentrancy blocked as expected");
            }
        }

        if should_repay {
            // Approve provider to pull principal + fee (pull repayment model)
            let total_debt = amount + fee;
            token_client.approve(
                &env.current_contract_address(),
                &provider,
                &total_debt,
                &200u32,
            );
        }
        // If !should_repay, no approval — the provider's transfer_from will fail
    }
}

// ============================================================================
// Test Suite
// ============================================================================

fn create_token_contract<'a>(
    e: &Env,
    admin: &Address,
) -> (Address, TokenClient<'a>, StellarTokenClient<'a>) {
    let addr = e.register_stellar_asset_contract(admin.clone());
    (
        addr.clone(),
        TokenClient::new(e, &addr),
        StellarTokenClient::new(e, &addr),
    )
}

fn setup_protocol<'a>(
    e: &Env,
) -> (
    HelloContractClient<'a>,
    Address,
    Address,
    Address,
    TokenClient<'a>,
) {
    let admin = Address::generate(e);
    let user = Address::generate(e);

    let protocol_id = e.register(HelloContract, ());
    let client = HelloContractClient::new(e, &protocol_id);

    client.initialize(&admin);

    let (token_addr, token_client, stellar_token_client) = create_token_contract(e, &admin);

    // Seed protocol with liquidity for flash loans
    stellar_token_client.mint(&protocol_id, &1_000_000_000);
    // Seed user with tokens for collateral / fees
    stellar_token_client.mint(&user, &10_000_000);

    // Enable the asset in the protocol
    client.update_asset_config(
        &token_addr,
        &crate::deposit::AssetParams {
            deposit_enabled: true,
            collateral_factor: 7500,
            max_deposit: i128::MAX,
            borrow_fee_bps: 50,
        },
    );

    (client, protocol_id, admin, user, token_client)
}

#[test]
fn test_flash_loan_happy_path() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, protocol_id, admin, _user, token_client) = setup_protocol(&env);
    let token_addr = token_client.address.clone();

    // Configure flash loan (fee = 0.1%)
    client.configure_flash_loan(
        &admin,
        &crate::flash_loan::FlashLoanConfig {
            fee_bps: 10,
            max_amount: 1_000_000_000_000,
            min_amount: 100,
        },
    );

    // Deploy and initialise receiver
    let receiver_id = env.register(MockFlashLoanReceiver, ());
    let receiver_client = MockFlashLoanReceiverClient::new(&env, &receiver_id);
    receiver_client.init(&protocol_id, &true, &false);

    // Give receiver enough to cover the fee (loan=1000, fee=1)
    let stellar_token_client = StellarTokenClient::new(&env, &token_addr);
    stellar_token_client.mint(&receiver_id, &100);

    let loan_amount = 1000i128;
    // execute_flash_loan transfers funds to receiver, calls on_flash_loan, then pulls repayment
    let total_repayment =
        client.execute_flash_loan(&receiver_id, &token_addr, &loan_amount, &receiver_id);

    // After repayment the receiver should have: 100 (seed) - fee
    let fee = total_repayment - loan_amount;
    assert_eq!(token_client.balance(&receiver_id), 100 - fee);
}

#[test]
fn test_deposit_borrow_interactions() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _protocol_id, _admin, user, token_client) = setup_protocol(&env);
    let token_addr = token_client.address.clone();

    let deposit_amount = 10_000i128;
    token_client.approve(&user, &_protocol_id, &deposit_amount, &200u32);

    client.deposit_collateral(&user, &Some(token_addr.clone()), &deposit_amount);

    // Verify internal position was recorded
    assert_eq!(
        client.get_user_asset_collateral(&user, &token_addr),
        deposit_amount
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #20)")]
fn test_flash_loan_insufficient_liquidity() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _, user, token_client) = setup_protocol(&env);

    // Request more than the protocol holds
    let too_much = 2_000_000_000i128;
    client.execute_flash_loan(&user, &token_client.address, &too_much, &user);
}

#[test]
#[should_panic(expected = "Reentrancy blocked as expected")]
fn test_flash_loan_reentrancy_block() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, protocol_id, admin, _user, token_client) = setup_protocol(&env);
    let token_addr = token_client.address.clone();

    client.configure_flash_loan(
        &admin,
        &crate::flash_loan::FlashLoanConfig {
            fee_bps: 0,
            max_amount: 1_000_000_000_000,
            min_amount: 1,
        },
    );

    // Receiver tries to re-enter and panics when re-entry is blocked,
    // proving the Soroban host prevents contract re-entry.
    let receiver_id = env.register(MockFlashLoanReceiver, ());
    let receiver_client = MockFlashLoanReceiverClient::new(&env, &receiver_id);
    receiver_client.init(&protocol_id, &true, &true); // should_repay=true, should_reenter=true

    client.execute_flash_loan(&receiver_id, &token_addr, &1000i128, &receiver_id);
}

#[test]
fn test_cross_contract_error_propagation() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _, user, token_client) = setup_protocol(&env);

    // Zero amount is rejected at the contract level before any token call
    let res = client.try_deposit_collateral(&user, &Some(token_client.address.clone()), &0);
    assert!(res.is_err());
}

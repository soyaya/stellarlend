use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, Symbol, Vec,
};

use stellarlend_amm::{
    AmmCallbackData, AmmContract, AmmContractClient, AmmProtocolConfig, AmmSettings,
    LiquidityParams, SwapParams, TokenPair,
};

use crate::encoding::{parse_actions, ActionBytes};

const MAX_ACTIONS: usize = 64;
const NUM_USERS: usize = 4;
const NUM_PROTOCOLS: usize = 3;

#[derive(Clone)]
struct AmmHarness {
    env: Env,
    admin: Address,
    contract_id: Address,
    users: [Address; NUM_USERS],
    protocols: [Address; NUM_PROTOCOLS],
}

impl AmmHarness {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().with_mut(|li| li.timestamp = 1);

        let contract_id = env.register(AmmContract {}, ());
        let contract = AmmContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let _ = contract.try_initialize_amm_settings(&admin, &100, &1000, &1);

        let users = [
            Address::generate(&env),
            Address::generate(&env),
            Address::generate(&env),
            Address::generate(&env),
        ];
        let protocols = [
            Address::generate(&env),
            Address::generate(&env),
            Address::generate(&env),
        ];

        Self {
            env,
            admin,
            contract_id,
            users,
            protocols,
        }
    }

    fn user(&self, idx: u8) -> Address {
        self.users[(idx as usize) % NUM_USERS].clone()
    }

    fn protocol(&self, idx: u8) -> Address {
        self.protocols[(idx as usize) % NUM_PROTOCOLS].clone()
    }

    fn supported_pairs(&self) -> Vec<TokenPair> {
        let mut pairs = Vec::new(&self.env);
        pairs.push_back(TokenPair {
            token_a: None,
            token_b: Some(Address::generate(&self.env)),
            pool_address: Address::generate(&self.env),
        });
        pairs
    }

    fn act(&self, contract: &AmmContractClient<'_>, action: ActionBytes) {
        match action.kind() % 7 {
            // 0: initialize settings
            0 => {
                let ds = (action.i64_a().unsigned_abs() as i128) % 10_000;
                let ms = (action.i64_b().unsigned_abs() as i128) % 10_000;
                let thr = (action.u64_tail() as i128) % 1_000_000;
                let _ = contract.try_initialize_amm_settings(&self.admin, &ds, &ms, &thr);
            }
            // 1: add protocol
            1 => {
                let p = self.protocol(action.asset_a());
                let fee = (action.i64_a().unsigned_abs() as i128) % 10_000;
                let min_swap = (action.i64_b().unsigned_abs() as i128) % 10_000;
                let max_swap = ((action.u64_tail() as i128) % 1_000_000_000).max(min_swap);
                let cfg = AmmProtocolConfig {
                    protocol_address: p.clone(),
                    protocol_name: Symbol::new(&self.env, "FuzzAMM"),
                    enabled: true,
                    fee_tier: fee,
                    min_swap_amount: min_swap,
                    max_swap_amount: max_swap,
                    supported_pairs: self.supported_pairs(),
                };
                let _ = contract.try_add_amm_protocol(&self.admin, &cfg);
            }
            // 2: update settings
            2 => {
                let ds = (action.i64_a().unsigned_abs() as i128) % 10_000;
                let ms = (action.i64_b().unsigned_abs() as i128) % 10_000;
                let settings = AmmSettings {
                    default_slippage: ds,
                    max_slippage: ms,
                    swap_enabled: (action.u32_param() & 1) == 1,
                    liquidity_enabled: (action.u32_param() & 2) == 2,
                    auto_swap_threshold: (action.u64_tail() as i128) % 1_000_000,
                };
                let _ = contract.try_update_amm_settings(&self.admin, &settings);
            }
            // 3: execute swap
            3 => {
                let user = self.user(action.user());
                let protocol = self.protocol(action.asset_a());
                let amount_in = i128::from(action.i64_a());
                let min_out = i128::from(action.i64_b());
                let slippage = (action.u32_param() as i128) % 10_000;
                let deadline = self
                    .env
                    .ledger()
                    .timestamp()
                    .saturating_add(action.u64_tail() % 60);
                let params = SwapParams {
                    protocol,
                    token_in: None,
                    token_out: Some(Address::generate(&self.env)),
                    amount_in,
                    min_amount_out: min_out,
                    slippage_tolerance: slippage,
                    deadline,
                };
                let _ = contract.try_execute_swap(&user, &params);
            }
            // 4: add liquidity
            4 => {
                let user = self.user(action.user());
                let protocol = self.protocol(action.asset_a());
                let deadline = self
                    .env
                    .ledger()
                    .timestamp()
                    .saturating_add(action.u64_tail() % 60);
                let params = LiquidityParams {
                    protocol,
                    token_a: None,
                    token_b: Some(Address::generate(&self.env)),
                    amount_a: i128::from(action.i64_a()),
                    amount_b: i128::from(action.i64_b()),
                    min_amount_a: 0,
                    min_amount_b: 0,
                    deadline,
                };
                let _ = contract.try_add_liquidity(&user, &params);
            }
            // 5: remove liquidity
            5 => {
                let user = self.user(action.user());
                let protocol = self.protocol(action.asset_a());
                let lp = i128::from(action.i64_a());
                let deadline = self
                    .env
                    .ledger()
                    .timestamp()
                    .saturating_add(action.u64_tail() % 60);
                let token_b = Some(Address::generate(&self.env));
                let _ = contract.try_remove_liquidity(
                    &user, &protocol, &None, &token_b, &lp, &0, &0, &deadline,
                );
            }
            // 6: callback validation (no-op in many cases but exercises nonce logic)
            _ => {
                let user = self.user(action.user());
                let caller = self.protocol(action.asset_a());
                let mut expected = Vec::new(&self.env);
                expected.push_back(i128::from(action.i64_a()));
                expected.push_back(i128::from(action.i64_b()));
                let deadline = self
                    .env
                    .ledger()
                    .timestamp()
                    .saturating_add(1 + (action.u64_tail() % 60));
                let cb = AmmCallbackData {
                    nonce: action.u64_tail(),
                    operation: Symbol::new(&self.env, "swap"),
                    user,
                    expected_amounts: expected,
                    deadline,
                };
                let _ = contract.try_validate_amm_callback(&caller, &cb);
            }
        }
    }
}

pub fn run(data: &[u8]) {
    let h = AmmHarness::new();
    let contract = AmmContractClient::new(&h.env, &h.contract_id);
    for action in parse_actions(data, MAX_ACTIONS) {
        h.act(&contract, action);
    }
}

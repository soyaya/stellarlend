use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, String as SorobanString,
};

use bridge::{BridgeContract, BridgeContractClient};

use crate::encoding::{parse_actions, ActionBytes};

const MAX_ACTIONS: usize = 64;
const NUM_USERS: usize = 4;

#[derive(Clone)]
struct BridgeHarness {
    env: Env,
    contract_id: Address,
    admin: Address,
    users: [Address; NUM_USERS],
}

impl BridgeHarness {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().with_mut(|li| li.timestamp = 1);

        let contract_id = env.register(BridgeContract {}, ());
        let client = BridgeContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let _ = client.try_init(&admin);

        let users = [
            Address::generate(&env),
            Address::generate(&env),
            Address::generate(&env),
            Address::generate(&env),
        ];

        Self {
            env,
            contract_id,
            admin,
            users,
        }
    }

    fn user(&self, idx: u8) -> Address {
        self.users[(idx as usize) % NUM_USERS].clone()
    }

    fn bridge_id(&self, action: &ActionBytes) -> SorobanString {
        const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_";
        let len = ((action.u32_param() % 16) + 1) as usize;
        let mut s = std::string::String::with_capacity(len);
        for i in 0..len {
            let b = action.0[8 + (i % 24)];
            s.push(CHARS[(b as usize) % CHARS.len()] as char);
        }
        SorobanString::from_str(&self.env, &s)
    }

    fn act(&self, client: &BridgeContractClient<'_>, action: ActionBytes) {
        match action.kind() % 10 {
            // 0: init (idempotency / auth checks)
            0 => {
                let _ = client.try_init(&self.admin);
            }
            // 1: register_bridge
            1 => {
                let id = self.bridge_id(&action);
                let fee_bps = (action.u64_tail() % 2000) as u64;
                let min_amount = i128::from(action.i64_a());
                let _ = client.try_register_bridge(&self.admin, &id, &fee_bps, &min_amount);
            }
            // 2: set_bridge_fee
            2 => {
                let id = self.bridge_id(&action);
                let fee_bps = (action.u64_tail() % 2000) as u64;
                let _ = client.try_set_bridge_fee(&self.admin, &id, &fee_bps);
            }
            // 3: set_bridge_active
            3 => {
                let id = self.bridge_id(&action);
                let active = (action.u32_param() & 1) == 1;
                let _ = client.try_set_bridge_active(&self.admin, &id, &active);
            }
            // 4: pause/unpause bridge acceptance
            4 => {
                let paused = (action.u32_param() & 1) == 1;
                let _ = client.try_set_bridge_acceptance_paused(&self.admin, &paused);
            }
            // 5: bridge_deposit
            5 => {
                let sender = self.user(action.user());
                let id = self.bridge_id(&action);
                let amount = i128::from(action.i64_a());
                let _ = client.try_bridge_deposit(&sender, &id, &amount);
            }
            // 6: bridge_withdraw
            6 => {
                let id = self.bridge_id(&action);
                let recipient = self.user(action.user());
                let amount = i128::from(action.i64_a());
                let _ = client.try_bridge_withdraw(&self.admin, &id, &recipient, &amount);
            }
            // 7: transfer_admin
            7 => {
                let new_admin = self.user(action.user());
                let _ = client.try_transfer_admin(&self.admin, &new_admin);
            }
            // 8: queries
            8 => {
                let _ = client.is_bridge_acceptance_paused();
                let _ = client.list_bridges();
                let _ = client.get_admin();

                let id = self.bridge_id(&action);
                let _ = client.try_get_bridge_config(&id);
            }
            // 9: advance time
            _ => {
                let delta = action.u64_tail() % 86_400;
                self.env
                    .ledger()
                    .with_mut(|li| li.timestamp = li.timestamp.saturating_add(delta));
            }
        }
    }
}

pub fn run(data: &[u8]) {
    let h = BridgeHarness::new();
    let client = BridgeContractClient::new(&h.env, &h.contract_id);
    for action in parse_actions(data, MAX_ACTIONS) {
        h.act(&client, action);
    }
}

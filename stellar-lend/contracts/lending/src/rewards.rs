use soroban_sdk::{contracttype, Address, Env};

const SCALAR: i128 = 1_000_000_000;

#[derive(Clone)]
#[contracttype]
pub enum RewardKey {
    GlobalIndex,
    LastUpdate,
    EmissionRate,
    TotalLiquidity,
    UserIndex(Address),
    UserAccrued(Address),
}

// ─────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────

fn get_i128(env: &Env, key: &RewardKey) -> i128 {
    env.storage().persistent().get(key).unwrap_or(0)
}

fn set_i128(env: &Env, key: &RewardKey, val: i128) {
    env.storage().persistent().set(key, &val);
}

// ─────────────────────────────────────────
// Core logic
// ─────────────────────────────────────────

pub fn update_global_index(env: &Env) {
    let now = env.ledger().timestamp() as i128;
    let last = get_i128(env, &RewardKey::LastUpdate);

    if now == last {
        return;
    }

    let emission = get_i128(env, &RewardKey::EmissionRate);
    let total_liquidity = get_i128(env, &RewardKey::TotalLiquidity);

    if total_liquidity == 0 {
        set_i128(env, &RewardKey::LastUpdate, now);
        return;
    }

    let delta = now - last;

    let mut index = get_i128(env, &RewardKey::GlobalIndex);

    index += (emission * delta as i128 * SCALAR) / total_liquidity;

    set_i128(env, &RewardKey::GlobalIndex, index);
    set_i128(env, &RewardKey::LastUpdate, now);
}

pub fn update_user(env: &Env, user: &Address, balance: i128) {
    update_global_index(env);

    let global_index = get_i128(env, &RewardKey::GlobalIndex);

    let user_index_key = RewardKey::UserIndex(user.clone());
    let user_index = get_i128(env, &user_index_key);

    let mut accrued = get_i128(env, &RewardKey::UserAccrued(user.clone()));

    let delta = global_index - user_index;

    accrued += (balance * delta) / SCALAR;

    set_i128(env, &user_index_key, global_index);
    set_i128(env, &RewardKey::UserAccrued(user.clone()), accrued);
}

pub fn claim(env: &Env, user: &Address) -> i128 {
    let rewards = get_i128(env, &RewardKey::UserAccrued(user.clone()));

    set_i128(env, &RewardKey::UserAccrued(user.clone()), 0);

    rewards
}

// ─────────────────────────────────────────
// Liquidity tracking
// ─────────────────────────────────────────

pub fn add_liquidity(env: &Env, amount: i128) {
    let total = get_i128(env, &RewardKey::TotalLiquidity);
    set_i128(env, &RewardKey::TotalLiquidity, total + amount);
}

pub fn remove_liquidity(env: &Env, amount: i128) {
    let total = get_i128(env, &RewardKey::TotalLiquidity);
    set_i128(env, &RewardKey::TotalLiquidity, total - amount);
}

// ─────────────────────────────────────────
// Admin
// ─────────────────────────────────────────

pub fn set_emission_rate(env: &Env, rate: i128) {
    set_i128(env, &RewardKey::EmissionRate, rate);
}

use soroban_sdk::{contracterror, contracttype, symbol_short, Env, Map, Symbol, Vec};

const CACHE_TTL_SECS: u64 = 30;
const CACHE_MAX_ENTRIES: u32 = 64;
const CACHE_VALUES: Symbol = symbol_short!("cachevals");
const CACHE_ORDER: Symbol = symbol_short!("cacheordr");
const CACHE_HITS: Symbol = symbol_short!("cachehits");
const CACHE_MISS: Symbol = symbol_short!("cachemiss");

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum CacheError {
    InvalidTtl = 1,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheEntry {
    pub value: i128,
    pub expires_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub size: u32,
}

fn now(env: &Env) -> u64 {
    env.ledger().timestamp()
}

fn touch_key(env: &Env, key: Symbol) {
    let mut order: Vec<Symbol> = env
        .storage()
        .persistent()
        .get(&CACHE_ORDER)
        .unwrap_or(Vec::new(env));
    let mut idx = 0;
    while idx < order.len() {
        if order.get(idx).unwrap() == key {
            order.remove(idx);
            break;
        }
        idx += 1;
    }
    order.push_back(key);
    env.storage().persistent().set(&CACHE_ORDER, &order);
}

fn evict_lru_if_needed(env: &Env, values: &mut Map<Symbol, CacheEntry>) {
    let mut order: Vec<Symbol> = env
        .storage()
        .persistent()
        .get(&CACHE_ORDER)
        .unwrap_or(Vec::new(env));
    while values.len() > CACHE_MAX_ENTRIES {
        if order.is_empty() {
            break;
        }
        let lru_key = order.get(0).unwrap();
        order.remove(0);
        values.remove(lru_key);
    }
    env.storage().persistent().set(&CACHE_ORDER, &order);
}

pub fn invalidate(env: &Env, key: Symbol) {
    let mut values: Map<Symbol, CacheEntry> = env
        .storage()
        .persistent()
        .get(&CACHE_VALUES)
        .unwrap_or(Map::new(env));
    values.remove(key.clone());
    env.storage().persistent().set(&CACHE_VALUES, &values);
    let mut order: Vec<Symbol> = env
        .storage()
        .persistent()
        .get(&CACHE_ORDER)
        .unwrap_or(Vec::new(env));
    let mut idx = 0;
    while idx < order.len() {
        if order.get(idx).unwrap() == key {
            order.remove(idx);
            break;
        }
        idx += 1;
    }
    env.storage().persistent().set(&CACHE_ORDER, &order);
}

pub fn set_cached(env: &Env, key: Symbol, value: i128, ttl_secs: Option<u64>) -> Result<(), CacheError> {
    let ttl = ttl_secs.unwrap_or(CACHE_TTL_SECS);
    if ttl == 0 {
        return Err(CacheError::InvalidTtl);
    }
    let mut values: Map<Symbol, CacheEntry> = env
        .storage()
        .persistent()
        .get(&CACHE_VALUES)
        .unwrap_or(Map::new(env));
    values.set(
        key.clone(),
        CacheEntry {
            value,
            expires_at: now(env) + ttl,
        },
    );
    evict_lru_if_needed(env, &mut values);
    env.storage().persistent().set(&CACHE_VALUES, &values);
    touch_key(env, key);
    Ok(())
}

pub fn get_cached(env: &Env, key: Symbol) -> Option<i128> {
    let mut values: Map<Symbol, CacheEntry> = env
        .storage()
        .persistent()
        .get(&CACHE_VALUES)
        .unwrap_or(Map::new(env));
    if let Some(entry) = values.get(key.clone()) {
        if now(env) <= entry.expires_at {
            let hits = env.storage().persistent().get(&CACHE_HITS).unwrap_or(0u64) + 1;
            env.storage().persistent().set(&CACHE_HITS, &hits);
            touch_key(env, key);
            return Some(entry.value);
        }
        values.remove(key);
        env.storage().persistent().set(&CACHE_VALUES, &values);
    }
    let misses = env.storage().persistent().get(&CACHE_MISS).unwrap_or(0u64) + 1;
    env.storage().persistent().set(&CACHE_MISS, &misses);
    None
}

pub fn cache_stats(env: &Env) -> CacheStats {
    let values: Map<Symbol, CacheEntry> = env
        .storage()
        .persistent()
        .get(&CACHE_VALUES)
        .unwrap_or(Map::new(env));
    CacheStats {
        hits: env.storage().persistent().get(&CACHE_HITS).unwrap_or(0u64),
        misses: env.storage().persistent().get(&CACHE_MISS).unwrap_or(0u64),
        size: values.len(),
    }
}

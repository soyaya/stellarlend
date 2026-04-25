use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Map, Symbol, Vec};

const NEXT_ID_KEY: Symbol = symbol_short!("mb_nextid");
const QUEUE_KEY: Symbol = symbol_short!("mb_queue");
const MESSAGES_KEY: Symbol = symbol_short!("mb_msgs");
const DELIVERED_KEY: Symbol = symbol_short!("mb_done");
const MAX_RETRIES: u32 = 3;

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MessageBusError {
    MessageNotFound = 1,
    AlreadyDelivered = 2,
    RetryLimitExceeded = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MessageState {
    Queued,
    InFlight,
    Delivered,
    Failed,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BusMessage {
    pub id: u64,
    pub version: u32,
    pub source: Address,
    pub target: Address,
    pub kind: Symbol,
    pub payload_hash: Symbol,
    pub created_at: u64,
    pub attempts: u32,
    pub state: MessageState,
}

fn next_id(env: &Env) -> u64 {
    let id = env.storage().persistent().get(&NEXT_ID_KEY).unwrap_or(1u64);
    env.storage().persistent().set(&NEXT_ID_KEY, &(id + 1));
    id
}

pub fn publish(
    env: &Env,
    source: Address,
    target: Address,
    kind: Symbol,
    payload_hash: Symbol,
    version: u32,
) -> u64 {
    source.require_auth();
    let id = next_id(env);
    let msg = BusMessage {
        id,
        version,
        source,
        target,
        kind,
        payload_hash,
        created_at: env.ledger().timestamp(),
        attempts: 0,
        state: MessageState::Queued,
    };

    let mut messages: Map<u64, BusMessage> = env
        .storage()
        .persistent()
        .get(&MESSAGES_KEY)
        .unwrap_or(Map::new(env));
    messages.set(id, msg);
    env.storage().persistent().set(&MESSAGES_KEY, &messages);

    let mut queue: Vec<u64> = env
        .storage()
        .persistent()
        .get(&QUEUE_KEY)
        .unwrap_or(Vec::new(env));
    queue.push_back(id);
    env.storage().persistent().set(&QUEUE_KEY, &queue);
    id
}

pub fn dequeue_next(env: &Env) -> Option<BusMessage> {
    let mut queue: Vec<u64> = env
        .storage()
        .persistent()
        .get(&QUEUE_KEY)
        .unwrap_or(Vec::new(env));
    if queue.is_empty() {
        return None;
    }
    let id = queue.get(0).unwrap();
    queue.remove(0);
    env.storage().persistent().set(&QUEUE_KEY, &queue);

    let mut messages: Map<u64, BusMessage> = env
        .storage()
        .persistent()
        .get(&MESSAGES_KEY)
        .unwrap_or(Map::new(env));
    if let Some(mut msg) = messages.get(id) {
        msg.state = MessageState::InFlight;
        msg.attempts += 1;
        messages.set(id, msg.clone());
        env.storage().persistent().set(&MESSAGES_KEY, &messages);
        return Some(msg);
    }
    None
}

pub fn confirm_delivery(env: &Env, id: u64) -> Result<(), MessageBusError> {
    let mut messages: Map<u64, BusMessage> = env
        .storage()
        .persistent()
        .get(&MESSAGES_KEY)
        .unwrap_or(Map::new(env));
    let mut delivered: Map<u64, bool> = env
        .storage()
        .persistent()
        .get(&DELIVERED_KEY)
        .unwrap_or(Map::new(env));

    if delivered.get(id).unwrap_or(false) {
        return Err(MessageBusError::AlreadyDelivered);
    }

    let mut msg = messages.get(id).ok_or(MessageBusError::MessageNotFound)?;
    msg.state = MessageState::Delivered;
    messages.set(id, msg);
    delivered.set(id, true);

    env.storage().persistent().set(&MESSAGES_KEY, &messages);
    env.storage().persistent().set(&DELIVERED_KEY, &delivered);
    Ok(())
}

pub fn mark_failed(env: &Env, id: u64) -> Result<(), MessageBusError> {
    let mut messages: Map<u64, BusMessage> = env
        .storage()
        .persistent()
        .get(&MESSAGES_KEY)
        .unwrap_or(Map::new(env));
    let mut msg = messages.get(id).ok_or(MessageBusError::MessageNotFound)?;
    if msg.attempts >= MAX_RETRIES {
        return Err(MessageBusError::RetryLimitExceeded);
    }
    msg.state = MessageState::Failed;
    messages.set(id, msg);
    env.storage().persistent().set(&MESSAGES_KEY, &messages);
    Ok(())
}

pub fn retry_failed(env: &Env, id: u64) -> Result<(), MessageBusError> {
    let messages: Map<u64, BusMessage> = env
        .storage()
        .persistent()
        .get(&MESSAGES_KEY)
        .unwrap_or(Map::new(env));
    let msg = messages.get(id).ok_or(MessageBusError::MessageNotFound)?;
    if msg.attempts >= MAX_RETRIES {
        return Err(MessageBusError::RetryLimitExceeded);
    }
    let mut queue: Vec<u64> = env
        .storage()
        .persistent()
        .get(&QUEUE_KEY)
        .unwrap_or(Vec::new(env));
    queue.push_back(id);
    env.storage().persistent().set(&QUEUE_KEY, &queue);
    Ok(())
}

pub fn get_message(env: &Env, id: u64) -> Option<BusMessage> {
    let messages: Map<u64, BusMessage> = env
        .storage()
        .persistent()
        .get(&MESSAGES_KEY)
        .unwrap_or(Map::new(env));
    messages.get(id)
}

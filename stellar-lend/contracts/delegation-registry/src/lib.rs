#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env};

#[contract]
pub struct DelegationRegistry;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum DelegationError {
    Unauthorized = 1,
    InvalidExpiry = 2,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Permission {
    Deposit = 1,
    Withdraw = 2,
    Borrow = 4,
    Repay = 8,
    DepositCollateral = 16,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Delegation {
    pub permissions: u32,
    pub expiry: u64,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Delegation(Address, Address),
}

fn now(env: &Env) -> u64 {
    env.ledger().timestamp()
}

fn delegation_valid(env: &Env, d: &Delegation, required_permission: u32) -> bool {
    if d.expiry != 0 && d.expiry <= now(env) {
        return false;
    }
    (d.permissions & required_permission) != 0
}

#[contractimpl]
impl DelegationRegistry {
    pub fn grant(
        env: Env,
        delegator: Address,
        delegate: Address,
        permissions: u32,
        expiry: u64,
    ) -> Result<(), DelegationError> {
        delegator.require_auth();
        if expiry != 0 && expiry <= now(&env) {
            return Err(DelegationError::InvalidExpiry);
        }
        let d = Delegation { permissions, expiry };
        env.storage()
            .persistent()
            .set(&DataKey::Delegation(delegator, delegate), &d);
        Ok(())
    }

    pub fn revoke(env: Env, delegator: Address, delegate: Address) -> Result<(), DelegationError> {
        delegator.require_auth();
        env.storage()
            .persistent()
            .remove(&DataKey::Delegation(delegator, delegate));
        Ok(())
    }

    pub fn get(env: Env, delegator: Address, delegate: Address) -> Option<Delegation> {
        env.storage()
            .persistent()
            .get(&DataKey::Delegation(delegator, delegate))
    }

    pub fn validate(
        env: Env,
        delegator: Address,
        delegate: Address,
        permission: u32,
    ) -> bool {
        let d: Option<Delegation> = Self::get(env.clone(), delegator, delegate);
        match d {
            Some(d) => delegation_valid(&env, &d, permission),
            None => false,
        }
    }
}

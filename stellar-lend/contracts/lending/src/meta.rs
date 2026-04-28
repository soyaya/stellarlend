use soroban_sdk::{contracterror, contracttype, Address, Env, IntoVal, Symbol, Vec};

use crate::{borrow, deposit, pause::PauseType, withdraw};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MetaTxError {
    Unauthorized = 1,
    Expired = 2,
    InvalidNonce = 3,
    DelegationMissing = 4,
    DelegationExpired = 5,
    PermissionDenied = 6,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Action {
    Deposit = 1,
    Withdraw = 2,
    Borrow = 3,
    Repay = 4,
    DepositCollateral = 5,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Call {
    pub action: Action,
    pub asset: Address,
    pub amount: i128,
    pub collateral_asset: Option<Address>,
    pub collateral_amount: Option<i128>,
}

#[contracttype]
#[derive(Clone)]
pub enum MetaDataKey {
    DelegationRegistry,
    Nonce(Address),
}

pub fn set_delegation_registry(env: &Env, registry: Address) {
    env.storage()
        .persistent()
        .set(&MetaDataKey::DelegationRegistry, &registry);
}

pub fn get_delegation_registry(env: &Env) -> Option<Address> {
    env.storage().persistent().get(&MetaDataKey::DelegationRegistry)
}

fn get_nonce(env: &Env, delegator: &Address) -> u64 {
    env.storage()
        .persistent()
        .get(&MetaDataKey::Nonce(delegator.clone()))
        .unwrap_or(0)
}

fn set_nonce(env: &Env, delegator: &Address, nonce: u64) {
    env.storage()
        .persistent()
        .set(&MetaDataKey::Nonce(delegator.clone()), &nonce);
}

fn validate_delegation(
    env: &Env,
    registry: &Address,
    delegator: &Address,
    delegate: &Address,
    action: Action,
) -> Result<(), MetaTxError> {
    let permission = match action {
        Action::Deposit => 1u32,
        Action::Withdraw => 2u32,
        Action::Borrow => 4u32,
        Action::Repay => 8u32,
        Action::DepositCollateral => 16u32,
    };

    let valid: bool = env.invoke_contract(
        registry,
        &Symbol::new(env, "validate"),
        Vec::from_array(
            env,
            [
                delegator.clone().into_val(env),
                delegate.clone().into_val(env),
                (permission as u32).into_val(env),
            ],
        ),
    );

    if !valid {
        return Err(MetaTxError::PermissionDenied);
    }

    Ok(())
}

pub fn execute_delegated(
    env: &Env,
    delegator: Address,
    delegate: Address,
    nonce: u64,
    deadline: u64,
    calls: Vec<Call>,
) -> Result<(), MetaTxError> {
    delegate.require_auth();

    if deadline != 0 && env.ledger().timestamp() > deadline {
        return Err(MetaTxError::Expired);
    }

    let current = get_nonce(env, &delegator);
    if nonce != current {
        return Err(MetaTxError::InvalidNonce);
    }
    set_nonce(env, &delegator, current + 1);

    let registry = get_delegation_registry(env).ok_or(MetaTxError::DelegationMissing)?;

    for c in calls.iter() {
        validate_delegation(env, &registry, &delegator, &delegate, c.action)?;

        match c.action {
            Action::Deposit => {
                if crate::pause::is_paused(env, PauseType::Deposit) {
                    return Err(MetaTxError::Unauthorized);
                }
                deposit::deposit_with_auth(env, delegator.clone(), c.asset.clone(), c.amount, false)
                    .map_err(|_| MetaTxError::Unauthorized)?;
            }
            Action::Withdraw => {
                if crate::pause::is_paused(env, PauseType::Withdraw) {
                    return Err(MetaTxError::Unauthorized);
                }
                withdraw::withdraw_with_auth(env, delegator.clone(), c.asset.clone(), c.amount, false)
                    .map_err(|_| MetaTxError::Unauthorized)?;
            }
            Action::Borrow => {
                if crate::pause::is_paused(env, PauseType::Borrow) {
                    return Err(MetaTxError::Unauthorized);
                }
                let ca = c.collateral_asset.clone().ok_or(MetaTxError::Unauthorized)?;
                let camt = c.collateral_amount.ok_or(MetaTxError::Unauthorized)?;
                borrow::borrow_trusted(env, delegator.clone(), c.asset.clone(), c.amount, ca, camt)
                    .map_err(|_| MetaTxError::Unauthorized)?;
            }
            Action::Repay => {
                if crate::pause::is_paused(env, PauseType::Repay) {
                    return Err(MetaTxError::Unauthorized);
                }
                borrow::repay(env, delegator.clone(), c.asset.clone(), c.amount)
                    .map_err(|_| MetaTxError::Unauthorized)?;
            }
            Action::DepositCollateral => {
                if crate::pause::is_paused(env, PauseType::Deposit) {
                    return Err(MetaTxError::Unauthorized);
                }
                borrow::deposit(env, delegator.clone(), c.asset.clone(), c.amount)
                    .map_err(|_| MetaTxError::Unauthorized)?;
            }
        }
    }

    Ok(())
}

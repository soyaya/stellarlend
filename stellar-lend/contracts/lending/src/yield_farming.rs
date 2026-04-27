use soroban_sdk::{contracterror, contracttype, Address, Env, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum YieldError {
    VaultNotFound = 1,
    InsufficientFunds = 2,
    IntegrationFailed = 3,
    RiskTooHigh = 4,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct YieldSource {
    pub protocol_id: Symbol,
    pub address: Address,
    pub risk_score: u32,
    pub apy_bps: u32,
    pub active: bool,
}

pub fn deposit_to_yield_source(
    env: &Env,
    admin: Address,
    amount: i128,
    source: YieldSource,
) -> Result<i128, YieldError> {
    admin.require_auth();

    if !source.active {
        return Err(YieldError::IntegrationFailed);
    }

    // Safety check on external integrations
    if source.risk_score > 80 {
        return Err(YieldError::RiskTooHigh);
    }

    // In a full implementation, we'd invoke the external contract here:
    // env.invoke_contract(&source.address, &Symbol::new(env, "deposit"), ...);

    #[allow(deprecated)]
    env.events().publish(
        (Symbol::new(env, "yield_deposit"), source.protocol_id),
        amount,
    );
    Ok(amount)
}

pub fn compound_yield(env: &Env, source: YieldSource) -> Result<i128, YieldError> {
    if !source.active {
        return Err(YieldError::IntegrationFailed);
    }

    // Simulate claiming rewards and compounding
    let compounded_amount = 500i128; // mock yield calculated based on source.apy_bps
    #[allow(deprecated)]
    env.events().publish(
        (Symbol::new(env, "yield_compounded"), source.protocol_id),
        compounded_amount,
    );
    Ok(compounded_amount)
}

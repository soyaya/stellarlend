use soroban_sdk::{contracttype, Address};

pub const SHARED_TYPES_VERSION_V1: u32 = 1;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SharedTypesVersion {
    V1,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetRiskParamsV1 {
    pub collateral_factor_bps: i128,
    pub liquidation_threshold_bps: i128,
    pub reserve_factor_bps: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetConfigV1 {
    pub asset: Option<Address>,
    pub max_supply: i128,
    pub max_borrow: i128,
    pub can_collateralize: bool,
    pub can_borrow: bool,
    pub price: i128,
    pub price_updated_at: u64,
    pub risk: AssetRiskParamsV1,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PositionV1 {
    pub user: Address,
    pub asset: Option<Address>,
    pub collateral: i128,
    pub debt_principal: i128,
    pub accrued_interest: i128,
    pub last_updated: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PositionSummaryV1 {
    pub total_collateral_value: i128,
    pub weighted_collateral_value: i128,
    pub total_debt_value: i128,
    pub weighted_debt_value: i128,
    pub health_factor: i128,
    pub is_liquidatable: bool,
    pub borrow_capacity: i128,
}

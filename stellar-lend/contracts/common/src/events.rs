#![allow(unused)]

use soroban_sdk::{contractevent, Address, Env};

// Minimal event set required by `upgrade.rs`.
// These are emitted by publishing the struct instance (Soroban SDK pattern).

#[contractevent]
#[derive(Clone, Debug)]
pub struct UpgradeInitEvent {
    pub admin: Address,
    pub required_approvals: u32,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct UpgradeApproverAddedEvent {
    pub caller: Address,
    pub approver: Address,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct UpgradeApproverRemovedEvent {
    pub caller: Address,
    pub approver: Address,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct UpgradeProposedEvent {
    pub caller: Address,
    pub id: u64,
    pub new_version: u32,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct UpgradeApprovalRecordedEvent {
    pub caller: Address,
    pub proposal_id: u64,
    pub approval_count: u32,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct UpgradeExecutedEvent {
    pub caller: Address,
    pub proposal_id: u64,
    pub new_version: u32,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct UpgradeRollbackEvent {
    pub caller: Address,
    pub proposal_id: u64,
    pub prev_version: u32,
}

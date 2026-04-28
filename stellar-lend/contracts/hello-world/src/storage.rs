use soroban_sdk::{contracttype, Address, Vec};

#[derive(Clone)]
#[contracttype]
pub enum GovernanceDataKey {
    Admin,
    Config,
    NextProposalId,
    MultisigConfig,
    MultisigAdmins,
    MultisigThreshold,
    GuardianConfig,
    Guardians,
    GuardianThreshold,

    Proposal(u64),
    Vote(u64, Address),
    VotePowerSnapshot(u64, Address),
    VoteLock(Address),
    DelegationRecord(Address),
    ProposalWindowStart(Address),
    ProposalCreationCount(Address),
    GovernanceAnalytics,
    ProposalApprovals(u64),
    UserProposals(Address, u64),

    ProposalSimulationCache(u64),
    ParameterOptimizationCache,

    RecoveryRequest,
    RecoveryApprovals,

    // Timelock keys
    TimelockConfig,
    NextTimelockId,
    TimelockOperation(u64),
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    // Credit scoring keys
    CreditScore(Address),
    
    // Circuit breaker keys
    CircuitBreakerConfig,
    CircuitBreakerState,
    CircuitBreakerWhitelist,
}

#[derive(Clone)]
#[contracttype]
pub struct GuardianConfig {
    pub guardians: Vec<Address>,
    pub threshold: u32,
}

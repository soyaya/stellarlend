//! Contract events for the lending workspace (`LendingContract`, `DataStore`, `UpgradeManager`).
//!
//! # Indexer / off-chain consumers
//!
//! Each event is emitted via the Soroban `contractevent` macro and the generated `publish` helper
//! on the event value, which routes to `Events::publish_event` (not the deprecated `Events::publish`).
//!
//! ## Topic layout
//!
//! - **Lending (main contract)**  
//!   - `borrow_event`, `repay_event`, `withdraw_event`, `flash_loan_event`: first topic is the
//!     event type name in snake_case (Soroban default).  
//!   - `pause_event`: defined on [`crate::pause::PauseEvent`] in the pause module (shares
//!     [`crate::pause::PauseType`] with storage).  
//!   - **Vault vs borrow collateral adds** both use static topic `deposit_event` (see
//!     [`VaultDepositEvent`] and [`BorrowCollateralDepositEvent`]); payloads differ: vault deposits
//!     include `new_balance`; borrow collateral deposits do not.
//!
//! - **Data store contract** — static prefixes: `ds_init`, `writer`, `ds_save`, `ds_bkup`,
//!   `ds_rest`, `ds_migr`, followed by any `#[topic]` fields in struct order.
//!
//! - **Upgrade manager** — static prefixes: `up_init`, `up_apadd`, `up_prop`, `up_appr`, `up_exec`,
//!   `up_roll`, plus `#[topic]` fields as before.

use soroban_sdk::{contractevent, Address, String};

// ─── Lending (LendingContract) ─────────────────────────────────────────────

#[contractevent]
#[derive(Clone, Debug)]
pub struct BorrowEvent {
    pub user: Address,
    pub asset: Address,
    pub amount: i128,
    pub collateral: i128,
    pub timestamp: u64,
}

/// Collateral added to a borrow position (same static topic as vault deposits; distinguish by payload).
#[contractevent(topics = ["deposit_event"])]
#[derive(Clone, Debug)]
pub struct BorrowCollateralDepositEvent {
    pub user: Address,
    pub asset: Address,
    pub amount: i128,
    pub timestamp: u64,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct RepayEvent {
    pub user: Address,
    pub asset: Address,
    pub amount: i128,
    pub timestamp: u64,
}

/// Vault / pool deposit (same static topic as [`BorrowCollateralDepositEvent`]; includes `new_balance`).
#[contractevent(topics = ["deposit_event"])]
#[derive(Clone, Debug)]
pub struct VaultDepositEvent {
    pub user: Address,
    pub asset: Address,
    pub amount: i128,
    pub new_balance: i128,
    pub timestamp: u64,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct WithdrawEvent {
    pub user: Address,
    pub asset: Address,
    pub amount: i128,
    pub remaining_balance: i128,
    pub timestamp: u64,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct FlashLoanEvent {
    pub receiver: Address,
    pub asset: Address,
    pub amount: i128,
    pub fee: i128,
    pub timestamp: u64,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct PegDeviationEvent {
    pub asset: Address,
    pub price: i128,
    pub target_price: i128,
    pub deviation_bps: i128,
    pub timestamp: u64,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct StabilityFeeAppliedEvent {
    pub asset: Address,
    pub fee_bps: i128,
    pub timestamp: u64,
}

// ─── Data store contract ────────────────────────────────────────────────────

#[contractevent(topics = ["ds_init"], data_format = "single-value")]
#[derive(Clone, Debug)]
pub struct DataStoreInitEvent {
    #[topic]
    pub admin: Address,
}

#[contractevent(topics = ["writer"], data_format = "single-value")]
#[derive(Clone, Debug)]
pub struct DataStoreWriterChangeEvent {
    #[topic]
    pub caller: Address,
    #[topic]
    pub writer: Address,
}

#[contractevent(topics = ["ds_save"], data_format = "single-value")]
#[derive(Clone, Debug)]
pub struct DataStoreSaveEvent {
    #[topic]
    pub caller: Address,
    #[topic]
    pub key: String,
    pub value_len: u32,
}

#[contractevent(topics = ["ds_bkup"], data_format = "single-value")]
#[derive(Clone, Debug)]
pub struct DataStoreBackupEvent {
    #[topic]
    pub caller: Address,
    #[topic]
    pub backup_name: String,
    pub key_count: u32,
}

#[contractevent(topics = ["ds_rest"], data_format = "single-value")]
#[derive(Clone, Debug)]
pub struct DataStoreRestoreEvent {
    #[topic]
    pub caller: Address,
    #[topic]
    pub backup_name: String,
    pub entry_count: u32,
}

#[contractevent(topics = ["ds_migr"], data_format = "single-value")]
#[derive(Clone, Debug)]
pub struct DataStoreMigrateEvent {
    #[topic]
    pub caller: Address,
    #[topic]
    pub new_version: u32,
    pub memo: Option<String>,
}

// ─── Upgrade manager contract ──────────────────────────────────────────────

#[contractevent(topics = ["up_init"], data_format = "single-value")]
#[derive(Clone, Debug)]
pub struct UpgradeInitEvent {
    #[topic]
    pub admin: Address,
    pub required_approvals: u32,
}

#[contractevent(topics = ["up_apadd"], data_format = "single-value")]
#[derive(Clone, Debug)]
pub struct UpgradeApproverAddedEvent {
    #[topic]
    pub caller: Address,
    #[topic]
    pub approver: Address,
}

#[contractevent(topics = ["up_prop"], data_format = "single-value")]
#[derive(Clone, Debug)]
pub struct UpgradeProposedEvent {
    #[topic]
    pub caller: Address,
    #[topic]
    pub id: u64,
    pub new_version: u32,
}

#[contractevent(topics = ["up_appr"], data_format = "single-value")]
#[derive(Clone, Debug)]
pub struct UpgradeApprovalRecordedEvent {
    #[topic]
    pub caller: Address,
    #[topic]
    pub proposal_id: u64,
    pub approval_count: u32,
}

#[contractevent(topics = ["up_exec"], data_format = "single-value")]
#[derive(Clone, Debug)]
pub struct UpgradeExecutedEvent {
    #[topic]
    pub caller: Address,
    #[topic]
    pub proposal_id: u64,
    pub new_version: u32,
}

#[contractevent(topics = ["up_roll"], data_format = "single-value")]
#[derive(Clone, Debug)]
pub struct UpgradeRollbackEvent {
    #[topic]
    pub caller: Address,
    #[topic]
    pub proposal_id: u64,
    pub prev_version: u32,
}

// Lending uses the shared upgrade manager from `stellarlend-common`.
//
// Keeping this module as a thin re-export avoids code duplication while allowing
// existing tests and downstream tooling to refer to `crate::upgrade::*`.

pub use stellarlend_common::upgrade::{
    UpgradeError, UpgradeManager, UpgradeManagerClient, UpgradeStage, UpgradeStatus,
};

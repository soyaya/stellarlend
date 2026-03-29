# Smart Contract Upgrade Mechanism

This document explains the upgrade flow that exists in the repository today, what it guarantees, and what it does not yet automate.

## Current Reality

There are two important facts to keep in view:

1. `stellar-lend/contracts/hello-world` does not currently expose dedicated upgrade entrypoints from `HelloContract`.
2. `stellar-lend/contracts/lending/src/upgrade.rs` implements an `UpgradeManager`, but it does not call `env.deployer().update_current_contract_wasm(...)`.

That means the current upgrade manager is a governance and metadata tracker, not a fully automated self-upgrading contract.

What it does today:

- stores the current WASM hash and current version
- stores upgrade proposals
- tracks approvers and approval thresholds
- records executed and rolled-back proposal metadata
- emits upgrade lifecycle events for auditability

What it does not do today:

- swap contract code on-chain by itself
- migrate storage automatically
- roll back live contract code automatically

Operational consequence:

- a real production upgrade still requires a separate contract-code update step outside this manager
- a real rollback still requires a separate contract-code rollback step outside this manager

## Contracts and Files

- Upgrade manager implementation: `stellar-lend/contracts/lending/src/upgrade.rs`
- Upgrade events: `stellar-lend/contracts/lending/src/events.rs`
- Upgrade tests: `stellar-lend/contracts/lending/src/upgrade_test.rs`
- Storage compatibility notes: `docs/storage.md`

## Roles and Permissions

The upgrade manager distinguishes two roles.

### Admin

The admin can:

- initialize the manager
- add approvers
- propose upgrades
- roll back executed proposals

### Approver

An approver can:

- approve upgrade proposals
- execute approved proposals

Notes:

- the admin is automatically the first approver at initialization
- proposal execution requires the configured approval threshold
- rollback is admin-only

## Stored Upgrade State

The manager persists:

- `CurrentWasmHash`
- `CurrentVersion`
- `RequiredApprovals`
- approver list
- per-proposal metadata:
  - proposal id
  - proposer
  - new WASM hash
  - target version
  - approval list
  - stage
  - previous WASM hash
  - previous version

Stages are:

- `Proposed`
- `Approved`
- `Executed`
- `RolledBack`

## Upgrade Lifecycle in Code

### 1. Initialize

`init(admin, current_wasm_hash, required_approvals)`

- requires admin auth
- stores the initial hash
- sets current version to `0`
- validates that `required_approvals > 0`
- emits `("up_init", admin)` with `required_approvals`

### 2. Add Approvers

`add_approver(caller, approver)`

- admin only
- idempotent if the approver already exists
- emits `("up_apadd", caller, approver)`

### 3. Propose

`upgrade_propose(caller, new_wasm_hash, new_version)`

- admin only
- rejects versions less than or equal to the current version
- proposer auto-approves
- proposal starts in `Approved` if the threshold is already met, otherwise `Proposed`
- emits `("up_prop", caller, id)` with `new_version`

### 4. Approve

`upgrade_approve(caller, proposal_id)`

- approver only
- rejects duplicate approvals
- moves the proposal to `Approved` once the threshold is met
- emits `("up_appr", caller, proposal_id)` with `approval_count`

### 5. Execute

`upgrade_execute(caller, proposal_id)`

- approver only
- requires the threshold to be met
- captures the previous hash and version
- updates the tracked current hash and version
- marks the proposal as `Executed`
- emits `("up_exec", caller, proposal_id)` with `new_version`

Important:

- this step updates upgrade-manager metadata only
- it does not change live contract code

### 6. Roll Back

`upgrade_rollback(caller, proposal_id)`

- admin only
- only valid after `Executed`
- restores the tracked previous hash and version
- marks the proposal as `RolledBack`
- emits `("up_roll", caller, proposal_id)` with `prev_version`

Important:

- this step restores upgrade-manager metadata only
- it does not roll back live contract code

## What Data Is Preserved Across a Real WASM Upgrade

Soroban preserves storage when contract code is updated, but only if your new code remains storage-compatible.

The current storage guidance in `docs/storage.md` is the source of truth. The key safety rules are:

1. Append enum variants. Do not reorder or delete existing `contracttype` variants used as keys.
2. Keep struct field order stable. Do not reorder or remove stored fields.
3. Keep storage key definitions identical across versions.
4. Treat storage migrations as explicit admin operations, not as implicit side effects.

Examples of state that should survive a compatible code update:

- admin address and roles
- user collateral and debt positions
- protocol analytics
- oracle price feeds and caches
- risk parameters
- treasury and reserve balances
- governance configuration and proposals

## Planned Upgrade Runbook

Use this for a normal, non-emergency release.

### Phase 1: Prepare the Release

1. Build the candidate WASM from a clean commit.
2. Review storage compatibility against `docs/storage.md`.
3. If the release changes storage layout, write a dedicated migration entrypoint and test it separately.
4. Compute and record the new WASM hash.
5. Prepare a release note with:
   - target version
   - target WASM hash
   - expected state changes
   - migration steps, if any
   - rollback trigger conditions

### Phase 2: Rehearse on Testnet First

1. Deploy the current version to testnet.
2. Seed realistic state:
   - deposits
   - borrows
   - repayments
   - treasury state
   - governance state, if relevant
3. Exercise the new build against that state.
4. Run the migration, if one exists.
5. Verify:
   - user balances unchanged unless intended
   - reserve and treasury balances consistent
   - analytics getters still decode
   - no storage keys were orphaned unexpectedly
   - event decoders still work
6. Record the exact testnet contract IDs, ledger range, and observed upgrade events.

### Phase 3: Obtain Governance Approval

1. Initialize the upgrade manager if it is not already initialized.
2. Ensure the approver set reflects the current governance roster.
3. Call `upgrade_propose(admin, new_wasm_hash, new_version)`.
4. Collect approvals until the threshold is met.
5. Confirm `upgrade_status(proposal_id).stage == Approved`.

### Phase 4: Execute the Real Mainnet Upgrade

Because the current manager does not swap code, execution is a two-part operational procedure:

1. Perform the actual contract-code update using separate deployment tooling or an admin-only updater contract that calls `update_current_contract_wasm`.
2. Only after the code update succeeds, call `upgrade_execute(caller, proposal_id)` so the manager's tracked hash and version match reality.

Why this order is safer:

- if you call `upgrade_execute` first and the actual code update fails, your metadata is ahead of reality
- calling it after a confirmed code update keeps the audit trail aligned with the live contract

### Phase 5: Post-Upgrade Validation

Immediately verify:

1. read-only getters return expected values
2. at least one deposit or borrow or repay path still succeeds on test funds
3. emitted events decode with the existing indexer
4. protocol stats and analytics still populate
5. treasury and reserve balances remain intact
6. upgrade manager now reports the new version and hash

## Emergency Upgrade Runbook

There is no separate emergency-upgrade entrypoint in the current code. Emergency upgrades use the same manager with a faster governance process around it.

Recommended emergency process:

1. Freeze or pause risky protocol paths if the incident requires it.
2. Build the minimal fix release.
3. Run the smallest possible testnet reproduction of the incident and verify the fix.
4. Open an emergency proposal with the new WASM hash and version.
5. Collect threshold approvals from the pre-designated incident approvers.
6. Perform the external code update.
7. Immediately record the execution in the manager with `upgrade_execute`.
8. Intensify post-upgrade monitoring and keep rollback materials ready.

Emergency policy recommendations:

- keep the approver list current before incidents happen
- predefine who can approve under incident conditions
- document which pause switches should be activated first
- never skip testnet entirely unless the incident makes that impossible and leadership has explicitly accepted the risk

## Rollback Procedure

Rollback is also a two-part process because the manager does not change live code by itself.

### When to Roll Back

Possible rollback triggers:

- failed post-upgrade invariants
- storage corruption symptoms
- decoder or indexing breakage on critical events
- incorrect reserve, treasury, or accounting behavior
- severe availability regression

### Rollback Steps

1. Identify the executed proposal to roll back.
2. Read its stored `prev_wasm_hash` and `prev_version`.
3. Perform the actual contract-code rollback using separate deployment tooling or an admin-only updater contract.
4. After the code rollback succeeds, call `upgrade_rollback(admin, proposal_id)`.
5. Re-run the same validation checklist used after the forward upgrade.

Important:

- do not call `upgrade_rollback` before the real code rollback is confirmed
- otherwise the manager's metadata will again diverge from the live contract

## Data Migration Considerations

Not every upgrade needs a migration. Add a migration only when storage compatibility is not strict append-only.

You likely need a migration when:

- a storage-key enum changes
- a stored struct changes field order
- old data must be copied into new keys
- a value type changes in a non-backward-compatible way

Migration design rules:

1. Make migrations explicit and admin-only.
2. Make them idempotent when possible.
3. Separate code deployment from migration execution in runbooks.
4. Add tests that start from pre-upgrade state and assert post-upgrade state exactly.
5. Emit or record enough metadata to prove the migration ran once and completed.

Recommended migration verification:

- snapshot a representative pre-upgrade state
- run the migration once
- verify all old keys either still decode or were intentionally replaced
- verify a second migration attempt is rejected or becomes a no-op

## Testnet Verification Checklist

Use this checklist before any mainnet upgrade.

- current production state model is reproduced on testnet
- candidate WASM hash is recorded
- proposal threshold and approver list are correct
- upgrade proposal reaches `Approved`
- actual code update succeeds on testnet
- upgrade manager `upgrade_execute` is called after the successful update
- read-only getters return expected values
- at least one write path succeeds after upgrade
- event consumers decode new events correctly
- rollback path is tested on testnet as well

## Upgrade Events

The upgrade manager emits these events for audit trails and indexing:

| Topic tuple | Value | Meaning |
| --- | --- | --- |
| `("up_init", admin)` | `required_approvals` | manager initialized |
| `("up_apadd", caller, approver)` | single-value event | approver added |
| `("up_prop", caller, id)` | `new_version` | proposal opened |
| `("up_appr", caller, proposal_id)` | `approval_count` | approval recorded |
| `("up_exec", caller, proposal_id)` | `new_version` | metadata marked executed |
| `("up_roll", caller, proposal_id)` | `prev_version` | metadata marked rolled back |

## Recommended Next Hardening Step

If you want the repository to support truly end-to-end governed upgrades, add a dedicated admin-only or threshold-gated contract function that:

1. verifies the proposal is approved
2. calls `env.deployer().update_current_contract_wasm(new_wasm_hash)`
3. records the execution metadata in the same transaction or immediately after it

Until that exists, treat the current upgrade manager as an approval ledger plus version tracker.

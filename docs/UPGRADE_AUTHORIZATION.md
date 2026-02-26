# Upgrade Authorization and Key Rotation

## Scope

This document describes how upgrade authorization works for contracts using
`stellarlend_common::upgrade::UpgradeManager` and how to safely rotate upgrade keys.

## Authorization model

- `upgrade_init(admin, current_wasm_hash, required_approvals)` initializes upgrade state.
- `upgrade_propose(caller, new_wasm_hash, new_version)` is `admin` only.
- `upgrade_add_approver(caller, approver)` is `admin` only.
- `upgrade_remove_approver(caller, approver)` is `admin` only.
- `upgrade_approve(caller, proposal_id)` is restricted to the configured approver set.
- `upgrade_execute(caller, proposal_id)` is restricted to the configured approver set.
- `upgrade_rollback(caller, proposal_id)` is `admin` only.

All mutating authorization paths call `require_auth()` on the provided caller.

## Key rotation procedure

Safe rotation for an approver key:

1. Add a replacement key: `upgrade_add_approver(admin, new_key)`.
2. Verify the new key can approve and execute a proposal.
3. Revoke the old key: `upgrade_remove_approver(admin, old_key)`.
4. Confirm old key is rejected for `upgrade_approve` and `upgrade_execute`.

`upgrade_remove_approver` enforces threshold safety:

- It rejects removals that would leave no approvers.
- It rejects removals that would leave fewer approvers than `required_approvals`.

This prevents accidental permanent lockout during rotation.

## Invalid upgrade attempts covered by tests

- Unauthorized address attempts to add/remove approvers.
- Unauthorized address attempts to approve or execute upgrades.
- Duplicate approvals from the same key.
- Execute attempts before threshold approval is reached.
- Invalid version proposals (`new_version <= current_version`).
- Unsafe key removal that violates threshold constraints.

## Security assumptions

- Admin key custody is out of scope of contract logic and must be handled operationally.
- Approver keys should be distinct from the admin key where possible.
- `required_approvals` should reflect operational risk tolerance (single-key vs multi-key).
- In production, route admin operations through governance/multisig processes to avoid
  single-operator risk.

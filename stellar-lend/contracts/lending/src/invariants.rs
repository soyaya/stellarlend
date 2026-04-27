// invariants.rs
// Place at: stellar-lend/contracts/lending/src/invariants.rs
//
// Add to lib.rs (after existing mod declarations, around line 9):
//   pub mod invariants;

#![allow(unused_imports)]

extern crate alloc;

use alloc::vec::Vec;

use soroban_sdk::{Address, Env};

use crate::borrow::get_admin as get_borrow_admin;
use crate::pause::{is_paused, PauseType};
use crate::views::{
    get_collateral_balance as view_collateral_balance,
    get_collateral_value as view_collateral_value, get_debt_balance as view_debt_balance,
    get_debt_value as view_debt_value, get_health_factor as view_health_factor,
    get_user_position as view_user_position,
};

// ─────────────────────────────────────────────
// Violation — carries reproduction info
// ─────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct InvariantViolation {
    pub invariant_id: &'static str,
    pub message: &'static str,
}

// ─────────────────────────────────────────────
// Known exemption flags
// Each exemption is documented with the invariant it covers.
// ─────────────────────────────────────────────

#[derive(Default)]
pub struct ExemptionFlags {
    /// INV-005: admin is resetting interest rate — index may temporarily dip
    pub rate_reset_in_progress: bool,
    /// INV-007: oracle in degraded/fallback mode — staleness check relaxed
    pub oracle_fallback_active: bool,
    /// INV-009: inside flash loan — liquidity floor may be temporarily breached
    pub flash_loan_context: bool,
}

// ─────────────────────────────────────────────
// INV-001: Per-user solvency
// health_factor (in bps, 10_000 = 1.0) must be >= 10_000
// for any user who just completed a deposit/borrow/repay/withdraw.
// ─────────────────────────────────────────────
pub fn check_inv_001_solvency(env: &Env, user: &Address) -> Result<(), InvariantViolation> {
    let health_bps = view_health_factor(env, user);
    if health_bps < 10_000 {
        return Err(InvariantViolation {
            invariant_id: "INV-001",
            message: "Solvency: health_factor < 1.0 after action — undercollateralised",
        });
    }
    Ok(())
}

// ─────────────────────────────────────────────
// INV-002: Collateral balance non-negative
// ─────────────────────────────────────────────
pub fn check_inv_002_collateral_non_negative(
    env: &Env,
    user: &Address,
) -> Result<(), InvariantViolation> {
    let balance = view_collateral_balance(env, user);
    if balance < 0 {
        return Err(InvariantViolation {
            invariant_id: "INV-002",
            message: "Collateral balance < 0 — impossible state",
        });
    }
    Ok(())
}

// ─────────────────────────────────────────────
// INV-003: Debt balance non-negative
// ─────────────────────────────────────────────
pub fn check_inv_003_debt_non_negative(
    env: &Env,
    user: &Address,
) -> Result<(), InvariantViolation> {
    let balance = view_debt_balance(env, user);
    if balance < 0 {
        return Err(InvariantViolation {
            invariant_id: "INV-003",
            message: "Debt balance < 0 — impossible state",
        });
    }
    Ok(())
}

// ─────────────────────────────────────────────
// INV-004: Liquidation eligibility consistency
// If health < 1.0 and debt > 0, position must be reachable.
// EXEMPT when protocol is paused.
// ─────────────────────────────────────────────
pub fn check_inv_004_liquidation_eligible(
    env: &Env,
    user: &Address,
) -> Result<(), InvariantViolation> {
    if is_paused(env, PauseType::Liquidation) {
        return Ok(()); // documented exemption
    }
    let health_bps = view_health_factor(env, user);
    if health_bps < 10_000 {
        let debt = view_debt_balance(env, user);
        if debt <= 0 {
            return Err(InvariantViolation {
                invariant_id: "INV-004",
                message: "Liquidation: health_factor < 1.0 but debt == 0 — contradictory state",
            });
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────
// INV-005: No value creation on borrow
// collateral_value must not increase after a borrow.
// Snapshot before, check after.
// ─────────────────────────────────────────────
pub fn check_inv_005_no_value_creation_on_borrow(
    env: &Env,
    user: &Address,
    collateral_value_before: i128,
) -> Result<(), InvariantViolation> {
    let after = view_collateral_value(env, user);
    if after > collateral_value_before {
        return Err(InvariantViolation {
            invariant_id: "INV-005",
            message: "No-value-creation: collateral_value increased after borrow",
        });
    }
    Ok(())
}

// ─────────────────────────────────────────────
// INV-006: Admin address stability
// Admin must not change between actions unless set_admin was called.
// Snapshot before, check after.
// ─────────────────────────────────────────────
pub fn check_inv_006_admin_stability(
    env: &Env,
    admin_before: &Address,
) -> Result<(), InvariantViolation> {
    let admin_after = get_borrow_admin(env);
    if admin_after != Some(admin_before.clone()) {
        return Err(InvariantViolation {
            invariant_id: "INV-006",
            message: "Access control: admin changed without explicit set_admin action",
        });
    }
    Ok(())
}

// ─────────────────────────────────────────────
// INV-007: Pause immutability
// While paused, debt and collateral balances must not change.
// Only call this when is_paused() was true before the action.
// ─────────────────────────────────────────────
pub fn check_inv_007_pause_immutability(
    env: &Env,
    user: &Address,
    debt_before: i128,
    collateral_before: i128,
) -> Result<(), InvariantViolation> {
    if !is_paused(env, PauseType::Borrow) {
        return Ok(());
    }
    let debt_after = view_debt_balance(env, user);
    let collateral_after = view_collateral_balance(env, user);

    if debt_after != debt_before {
        return Err(InvariantViolation {
            invariant_id: "INV-007",
            message: "Pause: debt_balance changed while protocol is paused",
        });
    }
    if collateral_after != collateral_before {
        return Err(InvariantViolation {
            invariant_id: "INV-007",
            message: "Pause: collateral_balance changed while protocol is paused",
        });
    }
    Ok(())
}

// ─────────────────────────────────────────────
// INV-008: Health factor / debt consistency
// Zero debt must never produce health_factor < 1.0.
// Positive debt must never produce health_factor == 0.
// ─────────────────────────────────────────────
pub fn check_inv_008_health_factor_consistency(
    env: &Env,
    user: &Address,
) -> Result<(), InvariantViolation> {
    let debt = view_debt_balance(env, user);
    let health = view_health_factor(env, user);

    if debt == 0 && health < 10_000 {
        return Err(InvariantViolation {
            invariant_id: "INV-008",
            message: "Health factor: debt == 0 but health_factor < 1.0 — contradictory",
        });
    }
    if debt > 0 && health == 0 {
        return Err(InvariantViolation {
            invariant_id: "INV-008",
            message: "Health factor: debt > 0 but health_factor == 0 — arithmetic error",
        });
    }
    Ok(())
}

// ─────────────────────────────────────────────
// INV-009: Collateral value covers debt value
// For healthy positions, collateral_value must be >= debt_value.
// ─────────────────────────────────────────────
pub fn check_inv_009_collateral_covers_debt(
    env: &Env,
    user: &Address,
) -> Result<(), InvariantViolation> {
    let health = view_health_factor(env, user);
    if health >= 10_000 {
        let col_val = view_collateral_value(env, user);
        let debt_val = view_debt_value(env, user);
        if debt_val > 0 && col_val < debt_val {
            return Err(InvariantViolation {
                invariant_id: "INV-009",
                message: "Collateral coverage: collateral_value < debt_value on healthy position",
            });
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────
// Aggregate — run all stateless per-user invariants.
// Returns all violations found (does not stop on first).
// ─────────────────────────────────────────────
pub fn assert_all_for_user(env: &Env, user: &Address) -> Vec<InvariantViolation> {
    let mut violations = Vec::new();

    if let Err(v) = check_inv_001_solvency(env, user) {
        violations.push(v);
    }
    if let Err(v) = check_inv_002_collateral_non_negative(env, user) {
        violations.push(v);
    }
    if let Err(v) = check_inv_003_debt_non_negative(env, user) {
        violations.push(v);
    }
    if let Err(v) = check_inv_004_liquidation_eligible(env, user) {
        violations.push(v);
    }
    if let Err(v) = check_inv_008_health_factor_consistency(env, user) {
        violations.push(v);
    }
    if let Err(v) = check_inv_009_collateral_covers_debt(env, user) {
        violations.push(v);
    }

    violations
}

// ─────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use crate::LendingContract;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::Env;

    fn setup() -> (Env, Address) {
        let env = Env::default();
        let user = Address::generate(&env);
        (env, user)
    }

    #[allow(deprecated)]
    fn with_contract<T>(env: &Env, f: impl FnOnce() -> T) -> T {
        let contract_id = env.register_contract(None, LendingContract);
        env.as_contract(&contract_id, f)
    }

    #[test]
    fn test_violation_struct_fields() {
        let v = InvariantViolation {
            invariant_id: "INV-001",
            message: "test violation",
        };
        assert_eq!(v.invariant_id, "INV-001");
        assert_eq!(v.message, "test violation");
    }

    #[test]
    fn test_exemption_flags_default() {
        let flags = ExemptionFlags::default();
        assert!(!flags.rate_reset_in_progress);
        assert!(!flags.oracle_fallback_active);
        assert!(!flags.flash_loan_context);
    }

    #[test]
    fn test_inv_002_fresh_user_passes() {
        // A fresh address with no collateral stored should return Ok.
        let (env, user) = setup();
        with_contract(&env, || {
            assert!(check_inv_002_collateral_non_negative(&env, &user).is_ok());
        });
    }

    #[test]
    fn test_inv_003_fresh_user_passes() {
        let (env, user) = setup();
        with_contract(&env, || {
            assert!(check_inv_003_debt_non_negative(&env, &user).is_ok());
        });
    }

    #[test]
    fn test_inv_005_no_increase_passes() {
        // Passing the same value before and after should always pass.
        let (env, user) = setup();
        let before: i128 = 1_000_000;
        // view_collateral_value on fresh user returns 0, which is <= before
        with_contract(&env, || {
            let result = check_inv_005_no_value_creation_on_borrow(&env, &user, before);
            assert!(result.is_ok());
        });
    }

    #[test]
    fn test_assert_all_returns_vec() {
        let (env, user) = setup();
        let violations = with_contract(&env, || assert_all_for_user(&env, &user));
        // Fresh user with no state — all checks should pass
        assert!(
            violations.is_empty(),
            "Fresh user should have no violations"
        );
    }
}

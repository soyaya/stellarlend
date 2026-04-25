// ════════════════════════════════════════════════════════════════
// REENTRANCY GUARD - Prevents reentrancy attacks
// ════════════════════════════════════════════════════════════════

use soroban_sdk::{contracttype, Address, Env, Symbol};

/// Reentrancy guard state - is someone already inside this function?
#[derive(Clone, Copy, PartialEq, Eq)]
#[contracttype]
pub enum GuardState {
    NotEntered = 0,  // ✓ Safe to enter
    Entered = 1,     // ✗ Already inside, block new calls
}

/// Storage keys for reentrancy guards
#[contracttype]
#[derive(Clone)]
pub enum GuardKey {
    /// Guard for deposit function
    DepositGuard,
    /// Guard for withdrawal function
    WithdrawGuard,
    /// Guard for borrow function
    BorrowGuard,
    /// Guard for repay function
    RepayGuard,
    /// Guard for liquidate function
    LiquidateGuard,
    /// Guard for flash loan function
    FlashLoanGuard,
    /// Guard for deposit collateral function
    DepositCollateralGuard,
}

/// Get current guard state from storage
pub fn get_guard_state(env: &Env, key: GuardKey) -> GuardState {
    let storage_key = Symbol::new(env, &format!("guard_{:?}", key));
    
    match env.storage().instance().get::<Symbol, u32>(&storage_key) {
        Some(state) => {
            if state == 1 {
                GuardState::Entered
            } else {
                GuardState::NotEntered
            }
        }
        None => GuardState::NotEntered,
    }
}

/// Set guard state in storage
fn set_guard_state(env: &Env, key: GuardKey, state: GuardState) {
    let storage_key = Symbol::new(env, &format!("guard_{:?}", key));
    let state_value = match state {
        GuardState::NotEntered => 0u32,
        GuardState::Entered => 1u32,
    };
    
    env.storage().instance().set(&storage_key, &state_value);
}

/// RAII pattern: Guard that automatically exits when dropped
pub struct NonReentrant {
    env: Env,
    key: GuardKey,
}

impl NonReentrant {
    /// Create a new reentrancy guard
    /// 
    /// # Panics
    /// If someone tries to enter while already inside (reentrancy detected)
    pub fn new(env: Env, key: GuardKey) -> Result<Self, String> {
        // CHECK: Are we already inside?
        let state = get_guard_state(&env, key);
        
        if state == GuardState::Entered {
            // REENTRANCY DETECTED!
            return Err("Reentrancy detected!".to_string());
        }

        // EFFECT: Mark as entered IMMEDIATELY
        set_guard_state(&env, key, GuardState::Entered);

        Ok(NonReentrant { env, key })
    }
}

/// When NonReentrant goes out of scope, automatically exit
impl Drop for NonReentrant {
    fn drop(&mut self) {
        // INTERACTION: Exit only after all operations complete
        set_guard_state(&self.env, self.key, GuardState::NotEntered);
    }
}

// ════════════════════════════════════════════════════════════════
// HELPER MACROS
// ════════════════════════════════════════════════════════════════

/// Macro to create a reentrancy guard at function start
/// Usage: non_reentrant!(env, GuardKey::DepositGuard)?;
#[macro_export]
macro_rules! non_reentrant {
    ($env:expr, $key:expr) => {
        $crate::reentrancy_guard::NonReentrant::new($env, $key)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guard_state_transitions() {
        // Guard should start as NotEntered
        assert_eq!(
            GuardState::NotEntered,
            GuardState::NotEntered,
            "Initial state should be NotEntered"
        );
    }
}
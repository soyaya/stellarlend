use crate::reentrancy_guard::{GuardKey, NonReentrant};  // ← ADD

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum BorrowError {
    // ... existing errors ...
    Reentrancy = 20,  // ← ADD
}

pub fn borrow(
    env: &Env,
    user: Address,
    asset: Address,
    amount: i128,
    collateral_asset: Address,
    collateral_amount: i128,
) -> Result<(), BorrowError> {
    // 🛡️ REENTRANCY GUARD
    let _guard = NonReentrant::new(env.clone(), GuardKey::BorrowGuard)
        .map_err(|_| BorrowError::Reentrancy)?;

    // ✓ CHECK
    user.require_auth();
    
    if amount <= 0 || collateral_amount <= 0 {
        return Err(BorrowError::InvalidAmount);
    }

    // ✓ EFFECT - Update debt and collateral records
    let mut debt = get_user_debt(env, &user);
    debt.principal = debt.principal
        .checked_add(amount)
        .ok_or(BorrowError::Overflow)?;
    
    save_user_debt(env, &user, &debt);

    // ✓ INTERACTION - Only then transfer
    let token_client = token::Client::new(env, &asset);
    token_client.transfer(&env.current_contract_address(), &user, &amount);

    Ok(())
}

pub fn repay(
    env: &Env,
    user: Address,
    asset: Address,
    amount: i128,
) -> Result<(), BorrowError> {
    // 🛡️ REENTRANCY GUARD
    let _guard = NonReentrant::new(env.clone(), GuardKey::RepayGuard)
        .map_err(|_| BorrowError::Reentrancy)?;

    // ✓ CHECK
    user.require_auth();
    
    if amount <= 0 {
        return Err(BorrowError::InvalidAmount);
    }

    let mut debt = get_user_debt(env, &user);
    if debt.principal < amount {
        return Err(BorrowError::InvalidAmount);
    }

    // ✓ EFFECT - Reduce debt FIRST
    debt.principal = debt.principal
        .checked_sub(amount)
        .ok_or(BorrowError::Overflow)?;
    
    save_user_debt(env, &user, &debt);

    // ✓ INTERACTION - Collect payment LAST
    let token_client = token::Client::new(env, &asset);
    token_client.transfer(&user, &env.current_contract_address(), &amount);

    Ok(())
}
use crate::reentrancy_guard::{GuardKey, NonReentrant};  // ← ADD

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum FlashLoanError {
    InvalidAmount = 1,
    InsufficientRepayment = 2,
    Unauthorized = 3,
    InvalidFee = 4,
    CallbackFailed = 5,
    Reentrancy = 6,
    FlashLoanPaused = 7,
}

pub fn flash_loan(
    env: &Env,
    receiver: Address,
    asset: Address,
    amount: i128,
    params: Bytes,
) -> Result<(), FlashLoanError> {
    // 🛡️ REENTRANCY GUARD - IMPORTANT for flash loans!
    let _guard = NonReentrant::new(env.clone(), GuardKey::FlashLoanGuard)
        .map_err(|_| FlashLoanError::Reentrancy)?;

    // ✓ CHECK
    if is_paused(env, PauseType::FlashLoan) {
        return Err(FlashLoanError::FlashLoanPaused);
    }

    if amount <= 0 {
        return Err(FlashLoanError::InvalidAmount);
    }

    let fee = calculate_fee(env, amount);

    // Record initial balance
    let token_client = token::Client::new(env, &asset);
    let initial_balance = token_client.balance(&env.current_contract_address());

    // ✓ EFFECT - Record loan
    let loan_record = FlashLoanRecord {
        receiver: receiver.clone(),
        asset: asset.clone(),
        amount,
        fee,
        timestamp: env.ledger().timestamp(),
    };
    
    save_flash_loan_record(env, &loan_record);

    // ✓ INTERACTION - Transfer funds to receiver
    token_client.transfer(&env.current_contract_address(), &receiver, &amount);

    // Execute callback (this is where reentrancy could happen!)
    // But our guard prevents it!
    let callback_result: bool = env.invoke_contract(
        &receiver,
        &Symbol::new(env, "on_flash_loan"),
        (
            env.current_contract_address(),
            asset.clone(),
            amount,
            fee,
            params,
        )
            .into_val(env),
    );

    if !callback_result {
        return Err(FlashLoanError::CallbackFailed);
    }

    // Verify repayment
    let final_balance = token_client.balance(&env.current_contract_address());

    if final_balance < initial_balance + fee {
        return Err(FlashLoanError::InsufficientRepayment);
    }

    FlashLoanEvent {
        receiver,
        asset,
        amount,
        fee,
        timestamp: env.ledger().timestamp(),
    }
    .publish(env);

    Ok(())
}
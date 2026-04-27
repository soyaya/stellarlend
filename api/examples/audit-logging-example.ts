/**
 * Example demonstrating audit logging for financial operations
 * 
 * This example shows how to use the enhanced submit endpoint with audit logging
 * for all 4 lending operations: deposit, borrow, repay, withdraw
 */

interface SubmitRequest {
  signedXdr: string;
  operation?: 'deposit' | 'borrow' | 'repay' | 'withdraw';
  userAddress?: string;
  amount?: string;
  assetAddress?: string;
}

// Example usage for each lending operation with audit logging
const examples = {
  // Deposit operation with full audit data
  deposit: {
    signedXdr: "AAAAAgAAAABgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA...",
    operation: 'deposit' as const,
    userAddress: 'GDZZJ3UPZZCKY5DBH6ZGMPMRORRBG4ECIORASBUAXPPNCL4SYRHNLYU2',
    amount: '1000000', // 0.01 XLM in stroops
    assetAddress: 'GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAH2U' // Optional
  },

  // Borrow operation with full audit data
  borrow: {
    signedXdr: "AAAAAgAAAABgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA...",
    operation: 'borrow' as const,
    userAddress: 'GDZZJ3UPZZCKY5DBH6ZGMPMRORRBG4ECIORASBUAXPPNCL4SYRHNLYU2',
    amount: '500000', // 0.005 XLM in stroops
    assetAddress: 'GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAH2U'
  },

  // Repay operation with full audit data
  repay: {
    signedXdr: "AAAAAgAAAABgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA...",
    operation: 'repay' as const,
    userAddress: 'GDZZJ3UPZZCKY5DBH6ZGMPMRORRBG4ECIORASBUAXPPNCL4SYRHNLYU2',
    amount: '250000', // 0.0025 XLM in stroops
    assetAddress: 'GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAH2U'
  },

  // Withdraw operation with full audit data
  withdraw: {
    signedXdr: "AAAAAgAAAABgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA...",
    operation: 'withdraw' as const,
    userAddress: 'GDZZJ3UPZZCKY5DBH6ZGMPMRORRBG4ECIORASBUAXPPNCL4SYRHNLYU2',
    amount: '750000', // 0.0075 XLM in stroops
    assetAddress: 'GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAH2U'
  },

  // Minimal request (audit data will be redacted)
  minimal: {
    signedXdr: "AAAAAgAAAABgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA..."
  }
};

/**
 * Function to submit transaction with audit logging
 */
async function submitTransactionWithAudit(request: SubmitRequest): Promise<any> {
  try {
    const response = await fetch('http://localhost:3000/api/lending/submit', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(request),
    });

    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }

    return await response.json();
  } catch (error) {
    console.error('Error submitting transaction:', error);
    throw error;
  }
}

/**
 * Example usage
 */
async function demonstrateAuditLogging() {
  console.log('=== StellarLend Audit Logging Examples ===\n');

  // Example 1: Deposit with full audit data
  console.log('1. Deposit with full audit data:');
  try {
    const result = await submitTransactionWithAudit(examples.deposit);
    console.log('✅ Success:', result);
    console.log('📝 Audit log entry created with full details\n');
  } catch (error) {
    console.log('❌ Error:', error);
  }

  // Example 2: Borrow with full audit data
  console.log('2. Borrow with full audit data:');
  try {
    const result = await submitTransactionWithAudit(examples.borrow);
    console.log('✅ Success:', result);
    console.log('📝 Audit log entry created with full details\n');
  } catch (error) {
    console.log('❌ Error:', error);
  }

  // Example 3: Minimal request (audit data redacted)
  console.log('3. Minimal request (audit data redacted):');
  try {
    const result = await submitTransactionWithAudit(examples.minimal);
    console.log('✅ Success:', result);
    console.log('📝 Audit log entry created with redacted data\n');
  } catch (error) {
    console.log('❌ Error:', error);
  }
}

/**
 * Expected audit log format:
 * 
 * logger.info('AUDIT', {
 *   action: 'DEPOSIT',           // Operation type (uppercase)
 *   userAddress: 'GDZZJ...',     // User's Stellar address
 *   amount: '1000000',           // Amount in stroops
 *   assetAddress: 'GAAAAA...',   // Asset contract address
 *   txHash: 'abc123...',         // Transaction hash
 *   timestamp: '2024-01-01T...', // ISO timestamp
 *   ip: '192.168.1.1',          // Client IP address
 *   status: 'success',           // Transaction status
 *   ledger: 12345               // Ledger number
 * });
 */

export { demonstrateAuditLogging, examples, submitTransactionWithAudit };

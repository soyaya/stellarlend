/**
 * StellarLend API Usage Examples
 *
 * This file demonstrates how to interact with the StellarLend API
 * for common lending operations.
 */

import axios, { AxiosError } from 'axios';

const API_BASE_URL = process.env.API_BASE_URL || 'http://localhost:3000/api';

type LendingOperation = 'deposit' | 'borrow' | 'repay' | 'withdraw';

interface PrepareResponse {
  unsignedXdr: string;
  operation: LendingOperation;
  expiresAt: string;
}

interface TransactionResponse {
  success: boolean;
  transactionHash?: string;
  status: 'pending' | 'success' | 'failed';
  ledger?: number;
  message?: string;
  error?: string;
}

/**
 * Check API health status
 */
async function checkHealth(): Promise<void> {
  try {
    const response = await axios.get(`${API_BASE_URL}/health`);
    console.log('Health Check:', response.data);
    
    if (response.data.status === 'healthy') {
      console.log('✅ All services are operational');
    } else {
      console.log('⚠️ Some services are down:', response.data.services);
    }
  } catch (error) {
    console.error('❌ Health check failed:', error);
  }
}

/**
 * Request an unsigned XDR from the API
 */
async function prepareTransaction(
  operation: LendingOperation,
  userAddress: string,
  amount: string,
  assetAddress?: string
): Promise<PrepareResponse> {
  try {
    console.log(`\n📝 Preparing ${operation} transaction for ${amount} stroops...`);

    const response = await axios.get<PrepareResponse>(
      `${API_BASE_URL}/lending/prepare/${operation}`,
      {
        params: {
          userAddress,
          assetAddress,
          amount,
        },
      }
    );

    console.log('✅ Unsigned transaction prepared');
    console.log(`   Operation: ${response.data.operation}`);
    console.log(`   Expires At: ${response.data.expiresAt}`);
    console.log(`   XDR Preview: ${response.data.unsignedXdr.slice(0, 20)}...`);

    return response.data;
  } catch (error) {
    handleError(`Prepare ${operation}`, error);
    throw error;
  }
}

/**
 * Submit a client-signed XDR back to the API
 */
async function submitSignedTransaction(
  signedXdr: string
): Promise<TransactionResponse> {
  try {
    console.log('\n🚀 Submitting signed transaction...');

    const response = await axios.post<TransactionResponse>(
      `${API_BASE_URL}/lending/submit`,
      {
        signedXdr,
      }
    );

    if (response.data.success) {
      console.log('✅ Transaction successful!');
      console.log(`   Transaction Hash: ${response.data.transactionHash}`);
      console.log(`   Ledger: ${response.data.ledger}`);
    } else {
      console.log('❌ Transaction failed:', response.data.error);
    }

    return response.data;
  } catch (error) {
    handleError('Submit', error);
    throw error;
  }
}

/**
 * Handle API errors
 */
function handleError(operation: string, error: unknown): void {
  if (axios.isAxiosError(error)) {
    const axiosError = error as AxiosError<{ error: string }>;
    if (axiosError.response) {
      console.error(`❌ ${operation} failed:`, axiosError.response.data.error);
      console.error(`   Status: ${axiosError.response.status}`);
    } else if (axiosError.request) {
      console.error(`❌ ${operation} failed: No response from server`);
    } else {
      console.error(`❌ ${operation} failed:`, axiosError.message);
    }
  } else {
    console.error(`❌ ${operation} failed:`, error);
  }
}

/**
 * Placeholder signing hook for local wallet integration.
 * Replace this with Freighter, WalletKit, or another signer in real usage.
 */
async function signTransactionLocally(unsignedXdr: string): Promise<string> {
  console.log('\n🔐 Sign the XDR locally in your wallet before submitting it');
  console.log(`   Unsigned XDR: ${unsignedXdr}`);
  throw new Error('Implement local signing before using this example');
}

/**
 * Complete lending lifecycle example
 */
async function completeLendingCycle(): Promise<void> {
  console.log('='.repeat(60));
  console.log('StellarLend API - Complete Lending Cycle Example');
  console.log('='.repeat(60));

  // Replace with your actual testnet public address
  const USER_ADDRESS = 'GXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX';
  const OPERATIONS: Array<{ operation: LendingOperation; amount: string }> = [
    { operation: 'deposit', amount: '100000000' },
    { operation: 'borrow', amount: '50000000' },
    { operation: 'repay', amount: '55000000' },
    { operation: 'withdraw', amount: '50000000' },
  ];

  try {
    // 1. Check health
    await checkHealth();

    for (const { operation, amount } of OPERATIONS) {
      const prepared = await prepareTransaction(operation, USER_ADDRESS, amount);
      const signedXdr = await signTransactionLocally(prepared.unsignedXdr);
      await submitSignedTransaction(signedXdr);

      // Wait a bit for transaction to settle before the next step
      await new Promise((resolve) => setTimeout(resolve, 5000));
    }

    console.log('\n' + '='.repeat(60));
    console.log('✅ Complete lending cycle finished successfully!');
    console.log('='.repeat(60));
  } catch (error) {
    console.log('\n' + '='.repeat(60));
    console.log('❌ Lending cycle failed');
    console.log('='.repeat(60));
  }
}

/**
 * Error handling examples
 */
async function errorHandlingExamples(): Promise<void> {
  console.log('\n' + '='.repeat(60));
  console.log('Error Handling Examples');
  console.log('='.repeat(60));

  const USER_ADDRESS = 'GXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX';

  // Example 1: Invalid amount (zero)
  try {
    console.log('\n1. Testing zero amount (should fail)...');
    await prepareTransaction('deposit', USER_ADDRESS, '0');
  } catch (error) {
    console.log('   Expected error caught ✓');
  }

  // Example 2: Invalid address
  try {
    console.log('\n2. Testing invalid address (should fail)...');
    await prepareTransaction('deposit', 'invalid_address', '1000000');
  } catch (error) {
    console.log('   Expected error caught ✓');
  }

  // Example 3: Missing signed XDR
  try {
    console.log('\n3. Testing missing signed XDR (should fail)...');
    await axios.post(`${API_BASE_URL}/lending/submit`, {
      // signedXdr missing
    });
  } catch (error) {
    console.log('   Expected error caught ✓');
  }

  console.log('\n' + '='.repeat(60));
}

// Run examples if executed directly
if (require.main === module) {
  const args = process.argv.slice(2);
  
  if (args.includes('--health')) {
    checkHealth();
  } else if (args.includes('--errors')) {
    errorHandlingExamples();
  } else if (args.includes('--cycle')) {
    completeLendingCycle();
  } else {
    console.log('Usage:');
    console.log('  ts-node examples/usage.ts --health   # Check API health');
    console.log('  ts-node examples/usage.ts --errors   # Test error handling');
    console.log('  ts-node examples/usage.ts --cycle    # Run complete cycle');
  }
}

export {
  checkHealth,
  prepareTransaction,
  submitSignedTransaction,
  signTransactionLocally,
  completeLendingCycle,
  errorHandlingExamples,
};

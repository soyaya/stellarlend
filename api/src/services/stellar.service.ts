import {
  TransactionBuilder,
  Contract,
  xdr,
  Address,
  nativeToScVal,
  Account,
  BASE_FEE,
} from '@stellar/stellar-sdk';
import { Server as SorobanServer } from '@stellar/stellar-sdk/rpc';
import axios from 'axios';
import { config } from '../config';
import logger from '../utils/logger';
import { InternalServerError } from '../utils/errors';
import { TransactionResponse, LendingOperation, TransactionHistoryQuery, TransactionHistoryResponse, TransactionHistoryItem } from '../types';

const CONTRACT_METHODS: Record<LendingOperation, string> = {
  deposit: 'deposit_collateral',
  borrow: 'borrow_asset',
  repay: 'repay_debt',
  withdraw: 'withdraw_collateral',
};

// Timeout generous enough for client-side signing (5 minutes)
const TX_TIMEOUT_SECONDS = 300;

export class StellarService {
  private horizonUrl: string;
  private sorobanRpcUrl: string;
  private networkPassphrase: string;
  private contractId: string;
  private sorobanServer: SorobanServer;

  constructor() {
    this.horizonUrl = config.stellar.horizonUrl;
    this.sorobanRpcUrl = config.stellar.sorobanRpcUrl;
    this.networkPassphrase = config.stellar.networkPassphrase;
    this.contractId = config.stellar.contractId;
    this.sorobanServer = new SorobanServer(this.sorobanRpcUrl);
  }

  async getAccount(address: string): Promise<Account> {
    try {
      const response = await axios.get(`${this.horizonUrl}/accounts/${address}`);
      const data = response.data as { id: string; sequence: string };
      return new Account(data.id, data.sequence);
    } catch (error) {
      logger.error('Failed to fetch account:', error);
      throw new InternalServerError('Failed to fetch account information');
    }
  }

  private async buildTransaction(
    operation: LendingOperation,
    userAddress: string,
    assetAddress: string | undefined,
    amount: string
  ): Promise<string> {
    const account = await this.getAccount(userAddress);
    const contract = new Contract(this.contractId);

    const params = [
      new Address(userAddress).toScVal(),
      assetAddress ? new Address(assetAddress).toScVal() : xdr.ScVal.scvVoid(),
      nativeToScVal(BigInt(amount), { type: 'i128' }),
    ];

    const tx = new TransactionBuilder(account, {
      fee: BASE_FEE,
      networkPassphrase: this.networkPassphrase,
    })
      .addOperation(contract.call(CONTRACT_METHODS[operation], ...params))
      .setTimeout(TX_TIMEOUT_SECONDS)
      .build();

    const preparedTx = await this.sorobanServer.prepareTransaction(tx);
    return preparedTx.toXDR();
  }

  async buildUnsignedTransaction(
    operation: LendingOperation,
    userAddress: string,
    assetAddress: string | undefined,
    amount: string
  ): Promise<string> {
    try {
      return await this.buildTransaction(operation, userAddress, assetAddress, amount);
    } catch (error) {
      logger.error(`Failed to build unsigned ${operation} transaction:`, error);
      throw new InternalServerError(`Failed to build ${operation} transaction`);
    }
  }

  async submitTransaction(txXdr: string): Promise<TransactionResponse> {
    const {
      request: { maxRetries, retryInitialDelayMs, retryMaxDelayMs, timeout },
    } = config;

    for (let attempt = 0; attempt <= maxRetries; attempt++) {
      try {
        const response = await axios.post(
          `${this.horizonUrl}/transactions`,
          { tx: txXdr },
          { timeout }
        );
        // Horizon and other RPCs can return slightly different shapes; the only
        // reliable indicator we validate here is `successful` when present.
        const data = response.data as any;
        const successful: unknown = data?.successful;
        const transactionHash: string | undefined =
          data?.hash ?? data?.transaction_hash ?? data?.transactionHash;
        const ledger: number | undefined = data?.ledger ?? data?.ledger_index ?? data?.ledgerIndex;

        if (successful === false) {
          return {
            success: false,
            transactionHash,
            status: 'failed',
            error: 'Transaction failed on-chain',
            message: 'Provider reported on-chain failure despite successful HTTP submission',
            ledger,
            details: data,
          };
        }

        return {
          success: true,
          transactionHash,
          status: 'success',
          ledger,
        };
      } catch (error: any) {
        const status = error?.response?.status as number | undefined;
        const isClientError = typeof status === 'number' && status >= 400 && status < 500;
        const isRetryable =
          // Network error (no response) is retryable
          !error?.response ||
          // 5xx server errors are retryable
          (typeof status === 'number' && status >= 500);

        // Immediately fail on non-retryable 4xx errors
        if (isClientError && status !== 429) {
          logger.error('Transaction submission failed (non-retryable):', error);
          return {
            success: false,
            status: 'failed',
            error: error.response?.data?.extras?.result_codes || error.message,
          };
        }

        // If we've exhausted retries or it's not retryable, return failure
        if (attempt === maxRetries || !isRetryable) {
          logger.error('Transaction submission failed (final):', error);
          return {
            success: false,
            status: 'failed',
            error: error.response?.data?.extras?.result_codes || error.message,
          };
        }

        // Exponential backoff with cap
        const backoff = Math.min(retryInitialDelayMs * Math.pow(2, attempt), retryMaxDelayMs);
        logger.warn(
          `Submit transaction attempt ${attempt + 1} failed${
            status ? ` (status ${status})` : ''
          }. Retrying in ${backoff} ms...`
        );
        await new Promise((resolve) => setTimeout(resolve, backoff));
      }
    }

    // Fallback — should be unreachable because loop returns
    return {
      success: false,
      status: 'failed',
      error: 'Unknown submission error',
    };
  }

  async monitorTransaction(
    txHash: string,
    timeoutMs = 30000,
    abortSignal?: AbortSignal
  ): Promise<TransactionResponse> {
    const startTime = Date.now();
    let delay = 500;
    const maxDelay = 5000;

    while (Date.now() - startTime < timeoutMs) {
      if (abortSignal?.aborted) {
        return {
          success: false,
          transactionHash: txHash,
          status: 'cancelled',
          message: 'Transaction monitoring cancelled',
        };
      }
      try {
        const response = await axios.get(`${this.horizonUrl}/transactions/${txHash}`);
        const data = response.data as { successful: boolean; ledger: number };
        if (data.successful) {
          return {
            success: true,
            transactionHash: txHash,
            status: 'success',
            ledger: data.ledger,
          };
        }
        return {
          success: false,
          transactionHash: txHash,
          status: 'failed',
          error: 'Transaction failed',
        };
      } catch (error: any) {
        if (error.response?.status === 404) {
          // Wait for delay or until aborted
          await new Promise((resolve) => {
            const timeout = setTimeout(resolve, delay);
            if (abortSignal) {
              abortSignal.addEventListener(
                'abort',
                () => {
                  clearTimeout(timeout);
                  resolve(undefined);
                },
                { once: true }
              );
            }
          });
          if (abortSignal?.aborted) {
            return {
              success: false,
              transactionHash: txHash,
              status: 'cancelled',
              message: 'Transaction monitoring cancelled',
            };
          }
          delay = Math.min(delay * 2, maxDelay);
          continue;
        }
        logger.error('Error monitoring transaction:', error);
        throw new InternalServerError('Failed to monitor transaction');
      }
    }

    return {
      success: false,
      transactionHash: txHash,
      status: 'pending',
      message: 'Transaction monitoring timeout',
    };
  }

  async healthCheck(): Promise<{ horizon: boolean; sorobanRpc: boolean }> {
    const results = { horizon: false, sorobanRpc: false };

    try {
      await axios.get(`${this.horizonUrl}/`);
      results.horizon = true;
    } catch (error) {
      logger.error('Horizon health check failed:', error);
    }

    try {
      await this.sorobanServer.getHealth();
      results.sorobanRpc = true;
    } catch (error) {
      logger.error('Soroban RPC health check failed:', error);
    }

    return results;
  }

  async getTransactionHistory(query: TransactionHistoryQuery): Promise<TransactionHistoryResponse> {
    try {
      const { userAddress, limit = 10, cursor } = query;
      
      // Validate Stellar address format
      if (!this.isValidStellarAddress(userAddress)) {
        throw new InternalServerError('Invalid Stellar address format');
      }

      // Build Horizon API URL for transactions
      let url = `${this.horizonUrl}/accounts/${userAddress}/transactions?limit=${limit}&order=desc`;
      if (cursor) {
        url += `&cursor=${cursor}`;
      }

      const response = await axios.get(url);
      const transactions = response.data._embedded.records;

      // Filter and map transactions related to lending contract
      const lendingTransactions = await this.filterLendingTransactions(transactions);

      // Extract pagination info
      const pagination = {
        cursor: response.data._links.next ? response.data._links.next.href.split('cursor=')[1] : undefined,
        hasNextPage: !!response.data._links.next,
        limit: parseInt(response.data.limit) || limit,
      };

      return {
        transactions: lendingTransactions,
        pagination,
      };
    } catch (error) {
      logger.error('Failed to fetch transaction history:', error);
      throw new InternalServerError('Failed to fetch transaction history');
    }
  }

  private async filterLendingTransactions(transactions: any[]): Promise<TransactionHistoryItem[]> {
    const lendingTransactions: TransactionHistoryItem[] = [];

    for (const tx of transactions) {
      // Check if transaction involves our lending contract
      if (this.isLendingTransaction(tx)) {
        const item = await this.mapToTransactionHistoryItem(tx);
        if (item) {
          lendingTransactions.push(item);
        }
      }
    }

    return lendingTransactions;
  }

  private isLendingTransaction(transaction: any): boolean {
    try {
      // Check if transaction has operations that interact with our contract
      if (!transaction.operations || !Array.isArray(transaction.operations)) {
        return false;
      }

      return transaction.operations.some((op: any) => 
        op.type === 'invoke_contract_function' && 
        op.contract_id === this.contractId
      );
    } catch {
      return false;
    }
  }

  private async mapToTransactionHistoryItem(transaction: any): Promise<TransactionHistoryItem | null> {
    try {
      // Extract operation details
      const lendingOp = transaction.operations.find((op: any) => 
        op.type === 'invoke_contract_function' && 
        op.contract_id === this.contractId
      );

      if (!lendingOp) {
        return null;
      }

      // Map function name to operation type
      const operationType = this.mapFunctionToOperation(lendingOp.function_name);
      if (!operationType) {
        return null;
      }

      // Extract amount from function parameters
      const amount = this.extractAmountFromParams(lendingOp.function_parameters);

      return {
        transactionHash: transaction.hash,
        type: operationType,
        amount: amount || '0',
        assetAddress: this.extractAssetFromParams(lendingOp.function_parameters),
        timestamp: transaction.created_at,
        status: transaction.successful ? 'success' : 'failed',
        ledger: transaction.ledger,
        memo: transaction.memo || undefined,
      };
    } catch (error) {
      logger.error('Failed to map transaction to history item:', error);
      return null;
    }
  }

  private mapFunctionToOperation(functionName: string): LendingOperation | null {
    const functionToOperation: Record<string, LendingOperation> = {
      'deposit_collateral': 'deposit',
      'borrow_asset': 'borrow',
      'repay_debt': 'repay',
      'withdraw_collateral': 'withdraw',
    };

    return functionToOperation[functionName] || null;
  }

  private extractAmountFromParams(params: any[]): string {
    try {
      // Look for amount parameter (typically the third parameter)
      if (params && params.length >= 3) {
        const amountParam = params[2];
        if (amountParam && amountParam.value) {
          return amountParam.value.toString();
        }
      }
      return '0';
    } catch {
      return '0';
    }
  }

  private extractAssetFromParams(params: any[]): string | undefined {
    try {
      // Look for asset address parameter (typically the second parameter)
      if (params && params.length >= 2) {
        const assetParam = params[1];
        if (assetParam && assetParam.value) {
          return assetParam.value;
        }
      }
      return undefined;
    } catch {
      return undefined;
    }
  }

  private isValidStellarAddress(address: string): boolean {
    try {
      // Basic Stellar address validation (G followed by 56 alphanumeric characters)
      return /^G[A-Z0-9]{56}$/.test(address);
    } catch {
      return false;
    }
  }
}

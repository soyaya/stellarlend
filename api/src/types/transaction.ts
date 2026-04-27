import type { LendingOperation } from './index';

export type TransactionStepStatus =
  | 'pending'
  | 'approved'
  | 'rejected'
  | 'executing'
  | 'completed'
  | 'failed'
  | 'expired';

export type MultiStepTransactionStatus =
  | 'building'
  | 'pending_approval'
  | 'executing'
  | 'completed'
  | 'failed'
  | 'expired';

export interface TransactionStep {
  stepId: string;
  index: number;
  operation: LendingOperation;
  userAddress: string;
  amount: string;
  assetAddress?: string;
  status: TransactionStepStatus;
  unsignedXdr?: string;
  signedXdr?: string;
  txHash?: string;
  error?: string;
  createdAt: string;
  updatedAt: string;
}

export interface MultiStepTransaction {
  txId: string;
  userAddress: string;
  description: string;
  steps: TransactionStep[];
  currentStepIndex: number;
  status: MultiStepTransactionStatus;
  metadata: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
  expiresAt: string;
}

export interface CreateTransactionRequest {
  userAddress: string;
  description?: string;
  steps: Array<{
    operation: LendingOperation;
    amount: string;
    assetAddress?: string;
  }>;
  ttlSeconds?: number;
}

export interface ApproveStepRequest {
  txId: string;
  stepId: string;
  signedXdr: string;
}

export interface RejectStepRequest {
  txId: string;
  stepId: string;
  reason?: string;
}

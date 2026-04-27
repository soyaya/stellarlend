import {
  generateCommitment,
  generateRangeProof,
  verifyRangeProof,
  generateTransferProof,
  generateNonce,
  openCommitment,
  type AmountCommitment,
  type RangeProof,
  type PrivateTransferProof,
} from '../utils/zkProof';
import logger from '../utils/logger';

const MAX_AMOUNT = BigInt('1000000000000000'); // 1 billion tokens in stroops

export interface CommitRequest {
  amount: string;
}

export interface CommitResponse {
  commitment: AmountCommitment;
  nonce: string;
}

export interface RangeProofRequest {
  amount: string;
  min?: string;
  max?: string;
  nonce: string;
}

export interface VerifyRangeProofRequest {
  proof: RangeProof;
  nonce: string;
  amount: string;
}

export interface TransferProofRequest {
  senderAmount: string;
  recipientAmount: string;
  fee: string;
}

export interface VerifyTransferRequest {
  proof: PrivateTransferProof;
  senderAmount: string;
  recipientAmount: string;
  fee: string;
  senderNonce: string;
  recipientNonce: string;
}

export class ZkProofService {
  commit(req: CommitRequest): CommitResponse {
    const amount = BigInt(req.amount);
    if (amount < BigInt(0)) {
      throw Object.assign(new Error('Amount must be non-negative'), { status: 400 });
    }
    const nonce = generateNonce();
    const commitment = generateCommitment(amount, nonce);
    logger.info('Commitment generated');
    return { commitment, nonce };
  }

  openCommitment(commitment: AmountCommitment, amount: string): boolean {
    return openCommitment(commitment, BigInt(amount));
  }

  rangeProof(req: RangeProofRequest): RangeProof {
    const amount = BigInt(req.amount);
    const min = BigInt(req.min ?? '0');
    const max = BigInt(req.max ?? MAX_AMOUNT.toString());

    if (min > max) {
      throw Object.assign(new Error('min must be <= max'), { status: 400 });
    }

    const proof = generateRangeProof(amount, min, max, req.nonce);
    logger.info('Range proof generated', { min: min.toString(), max: max.toString() });
    return proof;
  }

  verifyRange(req: VerifyRangeProofRequest): { valid: boolean } {
    const valid = verifyRangeProof(req.proof, req.nonce, BigInt(req.amount));
    return { valid };
  }

  transferProof(req: TransferProofRequest): PrivateTransferProof {
    const senderAmount = BigInt(req.senderAmount);
    const recipientAmount = BigInt(req.recipientAmount);
    const fee = BigInt(req.fee ?? '0');

    if (senderAmount <= BigInt(0)) {
      throw Object.assign(new Error('Sender amount must be positive'), { status: 400 });
    }

    const proof = generateTransferProof(senderAmount, recipientAmount, fee, MAX_AMOUNT);
    logger.info('Transfer proof generated');
    return proof;
  }
}

export const zkProofService = new ZkProofService();

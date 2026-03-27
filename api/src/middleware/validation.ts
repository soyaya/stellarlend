import { body, param, validationResult, check } from 'express-validator';
import { Request, Response, NextFunction } from 'express';
import { ValidationError } from '../utils/errors';
import { StrKey } from '@stellar/stellar-sdk';

const VALID_OPERATIONS = ['deposit', 'borrow', 'repay', 'withdraw'];

export const validateRequest = (req: Request, res: Response, next: NextFunction) => {
  const errors = validationResult(req);
  if (!errors.isEmpty()) {
    const errorMessages = errors
      .array()
      .map((err) => err.msg)
      .join(', ');
    throw new ValidationError(errorMessages);
  }
  next();
};

export const amountValidation = [
  check('amount')
    .notEmpty()
    .withMessage('Amount is required')
    .custom((value) => {
      const errMsg = 'Amount must be a valid positive integer';

      try {
        const str = String(value).trim();

        // Strict integer check: reject floats, scientific notation, empty, etc.
        if (!/^\+?\d+$/.test(str)) {
          throw new Error(errMsg);
        }

        const amount = BigInt(str);
        if (amount <= 0n) {
          throw new Error(errMsg);
        }

        // Ensure it fits into signed i128 which is what the contract expects.
        const maxI128 = (1n << 127n) - 1n;
        if (amount > maxI128) {
          throw new Error(errMsg);
        }

        return true;
      } catch {
        throw new Error(errMsg);
      }
    }),
];

/**
 * Factory function to create lending validation middleware
 * Allows for future customization per operation if needed
 */
const createLendingValidation = () => [
  param('operation')
    .isIn(VALID_OPERATIONS)
    .withMessage(`Operation must be one of: ${VALID_OPERATIONS.join(', ')}`),
  check('userAddress')
    .notEmpty()
    .withMessage('User address is required')
    .custom((value) => {
      if (!StrKey.isValidEd25519PublicKey(value)) {
        throw new Error('Invalid Stellar address');
      }
      return true;
    }),
  ...amountValidation,
  check('assetAddress').optional().isString().notEmpty().withMessage('Asset address is required'),
  validateRequest,
];

export const prepareValidation = createLendingValidation();

export const submitValidation = [
  body('signedXdr').isString().notEmpty().withMessage('signedXdr is required'),
  body('operation').optional().isIn(VALID_OPERATIONS).withMessage(`Operation must be one of: ${VALID_OPERATIONS.join(', ')}`),
  body('userAddress').optional().custom((value) => {
    if (value && !StrKey.isValidEd25519PublicKey(value)) {
      throw new Error('Invalid Stellar address');
    }
    return true;
  }),
  body('amount').optional().custom((value) => {
    if (!value) return true;
    
    const errMsg = 'Amount must be a valid positive integer';
    try {
      const str = String(value).trim();
      if (!/^\+?\d+$/.test(str)) {
        throw new Error(errMsg);
      }
      const amount = BigInt(str);
      if (amount <= 0n) {
        throw new Error(errMsg);
      }
      const maxI128 = (1n << 127n) - 1n;
      if (amount > maxI128) {
        throw new Error(errMsg);
      }
      return true;
    } catch {
      throw new Error(errMsg);
    }
  }),
  body('assetAddress').optional().isString().notEmpty().withMessage('Asset address must be a string'),
  validateRequest,
];

// Kept for backward compatibility — deprecated, will be removed in v2
export const depositValidation = createLendingValidation();
export const borrowValidation = createLendingValidation();
export const repayValidation = createLendingValidation();
export const withdrawValidation = createLendingValidation();

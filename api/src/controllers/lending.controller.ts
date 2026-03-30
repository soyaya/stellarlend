import { Request, Response, NextFunction } from 'express';
import { StellarService } from '../services/stellar.service';
import {
  LendingOperation,
  PrepareResponse,
  SubmitRequest,
  ProtocolStatsResponse,
} from '../types';
import { config } from '../config';
import logger from '../utils/logger';

export const prepare = async (req: Request, res: Response, next: NextFunction) => {
  try {
    const operation = req.params.operation as LendingOperation;
    const { userAddress, assetAddress, amount } = { ...req.query, ...req.body } as any;

    logger.info('Preparing unsigned transaction', { operation, userAddress, amount });

    const stellarService = new StellarService();
    const unsignedXdr = await stellarService.buildUnsignedTransaction(
      operation,
      userAddress,
      assetAddress,
      amount
    );

    const expiresAt = new Date(Date.now() + 5 * 60 * 1000).toISOString();

    const response: PrepareResponse = { unsignedXdr, operation, expiresAt };
    return res.status(200).json(response);
  } catch (error) {
    next(error);
  }
};

export const submit = async (req: Request, res: Response, next: NextFunction) => {
  try {
    const { signedXdr, operation, userAddress, amount, assetAddress }: SubmitRequest = req.body;

    logger.info('Submitting signed transaction');

    const stellarService = new StellarService();
    const result = await stellarService.submitTransaction(signedXdr);

    if (result.success && result.transactionHash) {
      const monitorResult = await stellarService.monitorTransaction(result.transactionHash);
      
      // Create audit log entry with operation details
      const auditLogData = {
        action: operation ? operation.toUpperCase() : 'TRANSACTION_EXECUTED',
        userAddress: userAddress || 'REDACTED',
        amount: amount || 'REDACTED',
        assetAddress: assetAddress || 'REDACTED',
        txHash: result.transactionHash,
        timestamp: new Date().toISOString(),
        ip: req.ip,
        status: monitorResult.status,
        ledger: monitorResult.ledger
      };

      logger.info('AUDIT', auditLogData);
      
      return res.status(200).json(monitorResult);
    }

    return res.status(400).json(result);
  } catch (error) {
    next(error);
  }
};

export const getTransactionHistory = async (req: Request, res: Response, next: NextFunction) => {
  try {
    const { userAddress } = req.params;
    const query = { ...req.query, userAddress };
    const stellarService = new StellarService();
    const result = await stellarService.getTransactionHistory(query as any);

    return res.status(200).json(result);
  } catch (error) {
    next(error);
  }
};

export const healthCheck = async (req: Request, res: Response, next: NextFunction) => {
  try {
    const stellarService = new StellarService();
    const services = await stellarService.healthCheck();
    const isHealthy = services.horizon && services.sorobanRpc;

    res.status(isHealthy ? 200 : 503).json({
      status: isHealthy ? 'healthy' : 'unhealthy',
      timestamp: new Date().toISOString(),
      services,
    });
  } catch (error) {
    next(error);
  }
};

export const protocolStats = async (_req: Request, res: Response, next: NextFunction) => {
  try {
    const stellarService = new StellarService();
    const stats: ProtocolStatsResponse = await stellarService.getProtocolStats();

    res.setHeader(
      'Cache-Control',
      `public, max-age=${Math.floor(config.cache.protocolStatsTtlMs / 1000)}`
    );

    return res.status(200).json(stats);
  } catch (error) {
    next(error);
  }
};

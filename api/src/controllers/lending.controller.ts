import { Request, Response, NextFunction } from 'express';
import { StellarService } from '../services/stellar.service';
import {
  LendingOperation,
  PrepareResponse,
  SubmitRequest,
  ProtocolStatsResponse,
  TransactionHistoryQuery,
  TransactionHistoryResponse,
} from '../types';
import { config } from '../config';
import logger from '../utils/logger';
import { emergencyPauseService } from '../services/emergencyPause.service';
import { redisCacheService } from '../services/redisCache.service';
import { auditLogService } from '../services/auditLog.service';
import {
  assignRole,
  getCurrentRoleAssignments,
  getRbacAuditContext,
  scheduleRevocation,
  type Role,
} from '../middleware/rbac';

function mapHealthResponse(services: { horizon: boolean; sorobanRpc: boolean }) {
  const isHealthy = services.horizon && services.sorobanRpc;

  return {
    isHealthy,
    readinessStatus: isHealthy ? 'ok' : 'error',
    horizon: services.horizon ? 'up' : 'down',
    soroban: services.sorobanRpc ? 'up' : 'down',
  };
}

export const prepare = async (req: Request, res: Response, next: NextFunction) => {
  try {
    if (emergencyPauseService.isPaused().paused) {
      return res.status(503).json({
        success: false,
        error: 'Protocol is paused',
        reason: emergencyPauseService.isPaused().reason,
      });
    }

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
    const pauseState = emergencyPauseService.isPaused();
    if (pauseState.paused) {
      if (operation === 'withdraw' && userAddress && amount) {
        emergencyPauseService.queueWithdrawal({ userAddress, assetAddress, amount });
        return res.status(202).json({
          success: false,
          status: 'queued',
          reason: pauseState.reason,
          message: 'Withdrawal queued while protocol is paused',
        });
      }
      return res.status(503).json({
        success: false,
        error: 'Protocol is paused',
        reason: pauseState.reason,
      });
    }

    logger.info('Submitting signed transaction');

    const stellarService = new StellarService();
    const result = await stellarService.submitTransaction(signedXdr);

    if (result.success && result.transactionHash) {
      emergencyPauseService.recordSuccess();
      const monitorResult = await stellarService.monitorTransaction(result.transactionHash);

      auditLogService.record({
        action: operation ? (operation.toUpperCase() as any) : 'TRANSACTION_EXECUTED',
        actor: userAddress || 'REDACTED',
        status: monitorResult.status as any,
        txHash: result.transactionHash,
        ledger: monitorResult.ledger,
        amount: amount || 'REDACTED',
        assetAddress: assetAddress || 'REDACTED',
        ip: req.ip,
      });

      await redisCacheService.delByPrefix('stellarlend:position:');
      await redisCacheService.delByPrefix('stellarlend:pool:');
      await redisCacheService.delByPrefix('stellarlend:protocol:');

      return res.status(200).json(monitorResult);
    }

    emergencyPauseService.recordFailure();
    return res.status(400).json(result);
  } catch (error) {
    emergencyPauseService.recordFailure();
    next(error);
  }
};

export const getPauseStatus = (_req: Request, res: Response) => {
  return res.status(200).json({
    ...emergencyPauseService.isPaused(),
    queuedWithdrawals: emergencyPauseService.getWithdrawalQueue(),
    cacheMetrics: redisCacheService.getMetrics(),
  });
};

export const setManualPause = (req: Request, res: Response) => {
  const before = emergencyPauseService.isPaused();
  emergencyPauseService.pause('manual');
  auditLogService.record({
    action: 'PROTOCOL_PAUSED',
    actor: req.ip ?? 'SYSTEM',
    status: 'success',
    ip: req.ip,
    beforeState: { paused: before.paused, reason: before.reason },
    afterState: { paused: true, reason: 'manual' },
  });
  return res.status(200).json({ paused: true, reason: 'manual' });
};

export const resumeProtocol = (req: Request, res: Response) => {
  const before = emergencyPauseService.isPaused();
  const queuedWithdrawals = emergencyPauseService.drainWithdrawalQueue();
  emergencyPauseService.resume();
  auditLogService.record({
    action: 'PROTOCOL_RESUMED',
    actor: req.ip ?? 'SYSTEM',
    status: 'success',
    ip: req.ip,
    beforeState: { paused: before.paused, reason: before.reason },
    afterState: { paused: false, queuedWithdrawalsReleased: queuedWithdrawals.length },
  });
  return res.status(200).json({
    paused: false,
    resumed: true,
    queuedWithdrawalsReleased: queuedWithdrawals.length,
  });
};

export const assignAccessRole = (req: Request, res: Response) => {
  const { actor, role } = getRbacAuditContext(req);
  const { targetAddress, targetRole } = req.body as {
    targetAddress: string;
    targetRole: Role;
  };
  assignRole(role, targetAddress, targetRole);
  auditLogService.record({
    action: 'ROLE_ASSIGNED',
    actor,
    status: 'success',
    ip: req.ip,
    afterState: { targetAddress, targetRole, assignedBy: actor },
  });
  return res.status(200).json({
    success: true,
    assignedBy: actor,
    targetAddress,
    targetRole,
  });
};

export const revokeAccessRole = (req: Request, res: Response) => {
  const { actor, role } = getRbacAuditContext(req);
  const { targetAddress, targetRole, coolOffMs } = req.body as {
    targetAddress: string;
    targetRole: Role;
    coolOffMs?: number;
  };
  scheduleRevocation(actor, role, targetAddress, targetRole, coolOffMs ?? 3_600_000);
  auditLogService.record({
    action: 'ROLE_REVOKED',
    actor,
    status: 'pending',
    ip: req.ip,
    afterState: { targetAddress, targetRole, coolOffMs: coolOffMs ?? 3_600_000, revokedBy: actor },
  });
  return res.status(202).json({
    success: true,
    targetAddress,
    targetRole,
    coolOffMs: coolOffMs ?? 3_600_000,
  });
};

export const listRoleAssignments = (_req: Request, res: Response) => {
  return res.status(200).json(getCurrentRoleAssignments());
};

export const healthCheck = async (req: Request, res: Response, next: NextFunction) => {
  try {
    const stellarService = new StellarService();
    const services = await stellarService.healthCheck();
    const { isHealthy } = mapHealthResponse(services);

    res.status(isHealthy ? 200 : 503).json({
      status: isHealthy ? 'healthy' : 'unhealthy',
      timestamp: new Date().toISOString(),
      services,
    });
  } catch (error) {
    next(error);
  }
};

export const livenessCheck = (_req: Request, res: Response) => {
  res.status(200).json({ status: 'ok' });
};

export const readinessCheck = async (_req: Request, res: Response, next: NextFunction) => {
  try {
    const stellarService = new StellarService();
    const services = await stellarService.healthCheck();
    const { isHealthy, readinessStatus, horizon, soroban } = mapHealthResponse(services);

    res.status(isHealthy ? 200 : 503).json({
      status: readinessStatus,
      horizon,
      soroban,
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

export const getTransactionHistory = async (
  req: Request,
  res: Response,
  next: NextFunction
) => {
  try {
    const stellarService = new StellarService();
    const query: TransactionHistoryQuery = {
      userAddress: req.params.userAddress,
      limit: req.query.limit ? Number(req.query.limit) : undefined,
      cursor: typeof req.query.cursor === 'string' ? req.query.cursor : undefined,
    };

    const history: TransactionHistoryResponse = await stellarService.getTransactionHistory(query);
    return res.status(200).json(history);
  } catch (error) {
    next(error);
  }
};

export const streamTransactionHistory = async (
  req: Request,
  res: Response,
  next: NextFunction
) => {
  const pageSize = req.query.pageSize ? Number(req.query.pageSize) : undefined;

  const abort = new AbortController();
  req.on('close', () => abort.abort());

  res.setHeader('Content-Type', 'application/x-ndjson');
  res.setHeader('Cache-Control', 'no-cache');
  res.setHeader('X-Content-Type-Options', 'nosniff');
  res.flushHeaders();

  try {
    const stellarService = new StellarService();
    const stream = stellarService.streamTransactionHistory(
      req.params.userAddress,
      pageSize,
      abort.signal
    );

    for await (const item of stream) {
      if (abort.signal.aborted) break;
      res.write(JSON.stringify(item) + '\n');
    }

    if (!abort.signal.aborted) {
      res.end();
    }
  } catch (error) {
    if (!res.headersSent) {
      next(error);
    } else {
      res.write(JSON.stringify({ error: 'Stream interrupted' }) + '\n');
      res.end();
    }
  }
};

export const getAuditLogs = (req: Request, res: Response) => {
  const { action, actor, status, from, to, limit, offset } = req.query as Record<string, string>;
  const entries = auditLogService.search({
    action,
    actor,
    status,
    from,
    to,
    limit: limit ? Number(limit) : undefined,
    offset: offset ? Number(offset) : undefined,
  });
  return res.status(200).json({ total: auditLogService.count(), entries });
};

export const exportAuditLogs = (req: Request, res: Response) => {
  const { action, actor, status, from, to } = req.query as Record<string, string>;
  const json = auditLogService.export({ action, actor, status, from, to });
  res.setHeader('Content-Type', 'application/json');
  res.setHeader('Content-Disposition', 'attachment; filename="audit-logs.json"');
  return res.status(200).send(json);
};

export const verifyAuditLogIntegrity = (_req: Request, res: Response) => {
  const result = auditLogService.verify();
  return res.status(result.valid ? 200 : 409).json(result);
};

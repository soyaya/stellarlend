// Robust global Axios mock to prevent real network calls
import axios from 'axios';
jest.mock('axios');
const mockedAxios = axios as jest.Mocked<typeof axios>;
beforeAll(() => {
  mockedAxios.create.mockReturnThis();
  const axiosResponse = {
    data: {},
    status: 200,
    statusText: 'OK',
    headers: {},
    config: { url: '' },
  };
  mockedAxios.get.mockResolvedValue(axiosResponse);
  mockedAxios.post.mockResolvedValue(axiosResponse);
  mockedAxios.request.mockResolvedValue(axiosResponse);
});
afterEach(() => {
  jest.clearAllMocks();
});

// Mock StellarService before importing app
import { StellarService } from '../services/stellar.service';
import { idempotencyStore } from '../middleware/idempotency';
jest.mock('../services/stellar.service');
const mockStellarService: jest.Mocked<StellarService> = {
  buildUnsignedTransaction: jest.fn().mockResolvedValue('unsigned_xdr_string'),
  submitTransaction: jest.fn().mockResolvedValue({
    success: true,
    transactionHash: 'mock_tx_hash',
    status: 'success',
  }),
  monitorTransaction: jest.fn().mockResolvedValue({
    success: true,
    transactionHash: 'mock_tx_hash',
    status: 'success',
    ledger: 12345,
  }),
  getProtocolStats: jest.fn().mockResolvedValue({
    totalDeposits: '1000000',
    totalBorrows: '500000',
    utilizationRate: '0.50',
    numberOfUsers: 150,
    tvl: '1500000',
  }),
  healthCheck: jest.fn().mockResolvedValue({
    horizon: true,
    sorobanRpc: true,
  }),
  getTransactionHistory: jest.fn().mockResolvedValue({
    transactions: [
      {
        transactionHash: 'tx_hash_1',
        type: 'deposit',
        amount: '1000000',
        assetAddress: 'GTEST123...',
        timestamp: '2023-01-01T00:00:00Z',
        status: 'success',
        ledger: 12345,
      },
    ],
    pagination: {
      hasNextPage: false,
      limit: 10,
    },
  }),
} as any;
(StellarService as jest.Mock).mockImplementation(() => mockStellarService);

// Mock logger to capture audit logs
import logger from '../utils/logger';
jest.mock('../utils/logger');
const mockLogger = logger as jest.Mocked<typeof logger>;

import request from 'supertest';
import app, { resetRateLimiters } from '../app';

describe('Lending Controller', () => {
  beforeEach(async () => {
    jest.clearAllMocks();
    idempotencyStore.clear();
    await resetRateLimiters();
  });

  describe('GET /api/lending/prepare/:operation', () => {
    const validBody = {
      userAddress: 'GDZZJ3UPZZCKY5DBH6ZGMPMRORRBG4ECIORASBUAXPPNCL4SYRHNLYU2',
      amount: '1000000',
    };

    it.each(['deposit', 'borrow', 'repay', 'withdraw'])(
      'should return unsigned XDR for %s',
      async (operation) => {
        const response = await request(app)
          .get(`/api/lending/prepare/${operation}`)
          .send(validBody);

        expect(response.status).toBe(200);
        expect(response.body.unsignedXdr).toBe('unsigned_xdr_string');
        expect(response.body.operation).toBe(operation);
        expect(response.body.expiresAt).toBeDefined();
      }
    );

    it('should return 400 for invalid operation', async () => {
      const response = await request(app).get('/api/lending/prepare/invalid_op').send(validBody);

      expect(response.status).toBe(400);
    });

    it('should return 400 for missing userAddress', async () => {
      const response = await request(app)
        .get('/api/lending/prepare/deposit')
        .send({ amount: '1000000' });

      expect(response.status).toBe(400);
    });

    it('should return 400 for zero amount', async () => {
      const response = await request(app)
        .get('/api/lending/prepare/deposit')
        .send({ ...validBody, amount: '0' });

      expect(response.status).toBe(400);
    });

    it('should not accept userSecret in request body', async () => {
      const response = await request(app)
        .get('/api/lending/prepare/deposit')
        .send({ ...validBody, userSecret: 'SXXXXX' });

      expect(response.status).toBe(200);
      // userSecret must never be forwarded to the service
      expect(mockStellarService.buildUnsignedTransaction).toHaveBeenCalledWith(
        'deposit',
        validBody.userAddress,
        undefined,
        validBody.amount
      );
    });
  });

  describe('POST /api/lending/submit', () => {
    it('should submit signed XDR and return transaction result', async () => {
      const response = await request(app)
        .post('/api/lending/submit')
        .send({ signedXdr: 'signed_xdr_string' });

      expect(response.status).toBe(200);
      expect(response.body.success).toBe(true);
      expect(response.body.transactionHash).toBe('mock_tx_hash');
    });

    it('should log audit entry when transaction succeeds with full audit data', async () => {
      const auditData = {
        signedXdr: 'signed_xdr_string',
        operation: 'deposit',
        userAddress: 'GDZZJ3UPZZCKY5DBH6ZGMPMRORRBG4ECIORASBUAXPPNCL4SYRHNLYU2',
        amount: '1000000',
        assetAddress: 'GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAH2U'
      };

      const response = await request(app)
        .post('/api/lending/submit')
        .send(auditData);

      expect(response.status).toBe(200);
      expect(response.body.success).toBe(true);
      
      // Verify audit log was called with correct structure
      expect(mockLogger.info).toHaveBeenCalledWith('AUDIT', expect.objectContaining({
        action: 'DEPOSIT',
        userAddress: 'GDZZJ3UPZZCKY5DBH6ZGMPMRORRBG4ECIORASBUAXPPNCL4SYRHNLYU2',
        amount: '1000000',
        assetAddress: 'GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAH2U',
        txHash: 'mock_tx_hash',
        timestamp: expect.any(String),
        ip: expect.any(String),
        status: 'success',
        ledger: 12345
      }));
    });

    it('should log audit entry with redacted data when audit fields are missing', async () => {
      const response = await request(app)
        .post('/api/lending/submit')
        .send({ signedXdr: 'signed_xdr_string' });

      expect(response.status).toBe(200);
      expect(response.body.success).toBe(true);
      
      // Verify audit log was called with redacted values
      expect(mockLogger.info).toHaveBeenCalledWith('AUDIT', expect.objectContaining({
        action: 'TRANSACTION_EXECUTED',
        userAddress: 'REDACTED',
        amount: 'REDACTED',
        assetAddress: 'REDACTED',
        txHash: 'mock_tx_hash',
        timestamp: expect.any(String),
        ip: expect.any(String),
        status: 'success',
        ledger: 12345
      }));
    });

    it('should validate optional audit fields when provided', async () => {
      const response = await request(app)
        .post('/api/lending/submit')
        .send({
          signedXdr: 'signed_xdr_string',
          operation: 'invalid_operation',
          userAddress: 'invalid_address',
          amount: 'invalid_amount'
        });

      expect(response.status).toBe(400);
    });

    it('should return 400 when transaction fails', async () => {
      mockStellarService.submitTransaction.mockResolvedValueOnce({
        success: false,
        status: 'failed',
        error: 'Insufficient collateral',
      });

      const response = await request(app)
        .post('/api/lending/submit')
        .send({ signedXdr: 'signed_xdr_string' });

      expect(response.status).toBe(400);
      expect(response.body.success).toBe(false);
      
      // No audit log should be generated for failed transactions
      expect(mockLogger.info).not.toHaveBeenCalledWith('AUDIT', expect.any(Object));
    });

    it('should return 400 when signedXdr is missing', async () => {
      const response = await request(app).post('/api/lending/submit').send({});

      expect(response.status).toBe(400);
    });

    it('should never log secrets in audit entries', async () => {
      const response = await request(app)
        .post('/api/lending/submit')
        .send({
          signedXdr: 'signed_xdr_string',
          operation: 'deposit',
          userAddress: 'GDZZJ3UPZZCKY5DBH6ZGMPMRORRBG4ECIORASBUAXPPNCL4SYRHNLYU2',
          amount: '1000000',
          // Note: userSecret should not be a field that gets logged
        });

      expect(response.status).toBe(200);
      
      // Verify audit log does not contain any secret fields
      const auditCall = (mockLogger.info.mock.calls as Array<any[]>).find(
        (call) => call[0] === 'AUDIT' && typeof call[1] === 'object' && call[1] !== null
      );
      expect(auditCall).toBeDefined();
      const auditData = (auditCall?.[1] ?? {}) as Record<string, unknown>;
      
      // Ensure no secret fields are present
      expect(Object.keys(auditData)).not.toContain('userSecret');
      expect(Object.keys(auditData)).not.toContain('privateKey');
      expect(Object.keys(auditData)).not.toContain('secret');
    });
  });

  describe('GET /api/health', () => {
    it('should return healthy status when all services are up', async () => {
      const response = await request(app).get('/api/health');

      expect(response.status).toBe(200);
      expect(response.body.status).toBe('healthy');
    });

    it('should return unhealthy status when services are down', async () => {
      mockStellarService.healthCheck.mockResolvedValueOnce({
        horizon: false,
        sorobanRpc: true,
      });

      const response = await request(app).get('/api/health');

      expect(response.status).toBe(503);
      expect(response.body.status).toBe('unhealthy');
    });
  });

  describe('GET /api/health/live', () => {
    it('should return ok without checking upstream dependencies', async () => {
      const response = await request(app).get('/api/health/live');

      expect(response.status).toBe(200);
      expect(response.body).toEqual({ status: 'ok' });
      expect(mockStellarService.healthCheck).not.toHaveBeenCalled();
    });
  });

  describe('GET /api/health/ready', () => {
    it('should return ok when all dependencies are up', async () => {
      const response = await request(app).get('/api/health/ready');

      expect(response.status).toBe(200);
      expect(response.body).toEqual({
        status: 'ok',
        horizon: 'up',
        soroban: 'up',
      });
    });

    it('should return 503 when a dependency is unavailable', async () => {
      mockStellarService.healthCheck.mockResolvedValueOnce({
        horizon: false,
        sorobanRpc: true,
      });

      const response = await request(app).get('/api/health/ready');

      expect(response.status).toBe(503);
      expect(response.body).toEqual({
        status: 'error',
        horizon: 'down',
        soroban: 'up',
      });
    });
  });

  describe('GET /api/protocol/stats', () => {
    it('should return protocol statistics', async () => {
      const response = await request(app).get('/api/protocol/stats');

      expect(response.status).toBe(200);
      expect(response.body).toEqual({
        totalDeposits: '1000000',
        totalBorrows: '500000',
        utilizationRate: '0.50',
        numberOfUsers: 150,
        tvl: '1500000',
      });
      expect(response.headers['cache-control']).toBe('public, max-age=30');
    });
  });

  describe('Idempotency-Key', () => {
    const idemKey = '123e4567-e89b-12d3-a456-426614174000';

    it('should replay a cached submit response for duplicate POST requests', async () => {
      const firstResponse = await request(app)
        .post('/api/lending/submit')
        .set('Idempotency-Key', idemKey)
        .send({ signedXdr: 'signed_xdr_string' });

      const secondResponse = await request(app)
        .post('/api/lending/submit')
        .set('Idempotency-Key', idemKey)
        .send({ signedXdr: 'signed_xdr_string' });

      expect(firstResponse.status).toBe(200);
      expect(firstResponse.headers['idempotency-status']).toBe('created');
      expect(secondResponse.status).toBe(200);
      expect(secondResponse.headers['idempotency-status']).toBe('cached');
      expect(secondResponse.body).toEqual(firstResponse.body);
      expect(mockStellarService.submitTransaction).toHaveBeenCalledTimes(1);
      expect(mockStellarService.monitorTransaction).toHaveBeenCalledTimes(1);
    });

    it('should reject non-UUID idempotency keys', async () => {
      const response = await request(app)
        .post('/api/lending/submit')
        .set('Idempotency-Key', 'not-a-uuid')
        .send({ signedXdr: 'signed_xdr_string' });

      expect(response.status).toBe(400);
      expect(response.body.error).toMatch(/Idempotency-Key/i);
    });
  });
});

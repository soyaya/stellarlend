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
  healthCheck: jest.fn().mockResolvedValue({
    horizon: true,
    sorobanRpc: true,
  }),
} as any;
(StellarService as jest.Mock).mockImplementation(() => mockStellarService);

// Mock logger to capture audit logs
import logger from '../utils/logger';
jest.mock('../utils/logger');
const mockLogger = logger as jest.Mocked<typeof logger>;

import request from 'supertest';
import app from '../app';

describe('Lending Controller', () => {
  beforeEach(() => {
    jest.clearAllMocks();
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
      const auditCall = mockLogger.info.mock.calls.find(call => call[0] === 'AUDIT');
      expect(auditCall).toBeDefined();
      const auditData = auditCall![1];
      
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
        sorobanRpc: false,
      });

      const response = await request(app).get('/api/health');

      expect(response.status).toBe(503);
      expect(response.body.status).toBe('unhealthy');
    });
  });
});

import request from 'supertest';
import express from 'express';
import { bodySizeLimitMiddleware } from '../middleware/bodySizeLimit';
import { errorHandler } from '../middleware/errorHandler';
import { config } from '../config';

describe('Body Size Limit Middleware', () => {
  const createTestApp = (limit = config.bodySizeLimit.limit) => {
    const testApp = express();
    // Override the config for testing
    const originalLimit = config.bodySizeLimit.limit;
    config.bodySizeLimit.limit = limit;
    testApp.use(bodySizeLimitMiddleware);
    testApp.post('/test', (req, res) => {
      res.status(200).json({ success: true });
    });
    testApp.use(errorHandler);
    // Don't restore limit here because the request executes asynchronously later.
    // Instead we'll manage it per-test or use a wrapper.
    return testApp;
  };

  const originalLimit = config.bodySizeLimit.limit;

  afterEach(() => {
    config.bodySizeLimit.limit = originalLimit;
  });

  describe('bodySizeLimitMiddleware', () => {
    it('should allow requests with body size within limit', async () => {
      const app = createTestApp('10kb');
      const smallBody = { data: 'small' };

      const response = await request(app).post('/test').send(smallBody);

      expect(response.status).toBe(200);
      expect(response.body.success).toBe(true);
    });

    it('should return 413 when Content-Length exceeds limit', async () => {
      const app = createTestApp('1kb');

      // Send a request with a body larger than 1kb
      const largeData = { data: 'x'.repeat(2000) };

      const response = await request(app).post('/test').send(largeData);

      // express.json() limit should catch this, returning 413
      expect(response.status).toBe(413);
    });

    it('should throw PayloadTooLargeError when body size exceeds configured limit', async () => {
      // Test the parseSizeLimit function indirectly via the middleware behavior
      const app = createTestApp('1b'); // 1 byte limit

      const response = await request(app).post('/test').send({ data: 'test' });

      expect(response.status).toBe(413);
    });
  });

  describe('parseSizeLimit helper', () => {
    // Test helper indirectly through the middleware
    it('should handle different size units', async () => {
      const testCases = [
        { limit: '100b', shouldPass: true, body: { data: 'test' } },
        { limit: '1kb', shouldPass: true, body: { data: 'test' } },
        { limit: '1mb', shouldPass: true, body: { data: 'x'.repeat(500) } },
      ];

      for (const tc of testCases) {
        const app = createTestApp(tc.limit);
        const response = await request(app).post('/test').send(tc.body);
        expect(response.status).toBe(200);
      }
    });
  });
});

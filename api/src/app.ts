import express, { Application, Request, Response, NextFunction } from 'express';
import helmet from 'helmet';
import cors from 'cors';
import rateLimit, { MemoryStore } from 'express-rate-limit';
import { config } from './config';
import { bodySizeLimitMiddleware } from './middleware/bodySizeLimit';
import lendingRoutes from './routes/lending.routes';
import healthRoutes from './routes/health.routes';
import protocolRoutes from './routes/protocol.routes';
import portfolioRoutes from './routes/portfolio.routes';
import gasRoutes from './routes/gas.routes';
import { errorHandler } from './middleware/errorHandler';
import { idempotencyMiddleware } from './middleware/idempotency';
import { swaggerSpec } from './config/swagger';
import logger from './utils/logger';
import { requestIdMiddleware } from './middleware/requestId';
import { sanitizeInput } from './middleware/sanitizeInput';
import { redisCacheService } from './services/redisCache.service';

const app: Application = express();
app.use(requestIdMiddleware);

const ipRateLimitStore = new MemoryStore();
const userRateLimitStore = new MemoryStore();

app.use(
  helmet({
    hsts: {
      maxAge: 31536000,
      includeSubDomains: true,
      preload: true,
    },
  })
);

// Enforce HTTPS in production
if (config.server.env === 'production') {
  app.use((req, res, next) => {
    if (req.header('x-forwarded-proto') !== 'https' && !req.secure) {
      return res.redirect(`https://${req.header('host')}${req.url}`);
    }
    next();
  });
}

app.use(cors());
app.use(express.json({ limit: config.bodySizeLimit.limit }));
app.use(express.urlencoded({ extended: true, limit: config.bodySizeLimit.limit }));
app.use(sanitizeInput);
app.use(bodySizeLimitMiddleware);

const limiter = rateLimit({
  windowMs: config.rateLimit.windowMs,
  max: config.rateLimit.maxRequests,
  message: 'Too many requests from this IP, please try again later.',
  store: ipRateLimitStore,
});

app.use('/api/', limiter);

// Per-user rate limiter for lending endpoints
const userRateLimiter = rateLimit({
  windowMs: 60 * 1000, // 1 minute window
  max: 10, // 10 requests per minute per user
  store: userRateLimitStore,
  keyGenerator: (req) => {
    // Try to get userAddress from request body first, then query params, then fall back to IP
    const userAddress = req.body?.userAddress || req.query?.userAddress || req.ip;
    return userAddress;
  },
  message: { success: false, error: 'Too many requests for this account' },
  standardHeaders: true,
  legacyHeaders: false,
});

// Lazy-load Swagger UI so the module is only imported when /api/docs is hit
let swaggerUiLoaded = false;
app.use('/api/docs', (req: Request, res: Response, next: NextFunction) => {
  if (swaggerUiLoaded) return next();
  import('swagger-ui-express').then((swaggerUi) => {
    app.use('/api/docs', swaggerUi.serve, swaggerUi.setup(swaggerSpec));
    swaggerUiLoaded = true;
    next();
  }).catch(next);
});

app.get('/api/openapi.json', (_req, res) => {
  res.json(swaggerSpec);
});

app.use('/api/health', healthRoutes);
app.use('/api/protocol', protocolRoutes);
app.use('/api/lending', idempotencyMiddleware, userRateLimiter, lendingRoutes);
app.use('/api/portfolio', portfolioRoutes);
app.use('/api/gas', userRateLimiter, gasRoutes);

app.use(errorHandler);

void redisCacheService.warmup();

export async function resetRateLimiters(): Promise<void> {
  await Promise.all([ipRateLimitStore.resetAll(), userRateLimitStore.resetAll()]);
}

export default app;

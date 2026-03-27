import express, { Application, Request, Response, NextFunction } from 'express';
import helmet from 'helmet';
import cors from 'cors';
import rateLimit from 'express-rate-limit';
import { config } from './config';
import lendingRoutes from './routes/lending.routes';
import healthRoutes from './routes/health.routes';
import { errorHandler } from './middleware/errorHandler';
import { swaggerSpec } from './config/swagger';
import logger from './utils/logger';

const app: Application = express();

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
app.use(express.json());
app.use(express.urlencoded({ extended: true }));

const limiter = rateLimit({
  windowMs: config.rateLimit.windowMs,
  max: config.rateLimit.maxRequests,
  message: 'Too many requests from this IP, please try again later.',
});

app.use('/api/', limiter);

// Per-user rate limiter for lending endpoints
const userRateLimiter = rateLimit({
  windowMs: 60 * 1000, // 1 minute window
  max: 10, // 10 requests per minute per user
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
app.use('/api/lending', userRateLimiter, lendingRoutes);

app.use(errorHandler);

export default app;

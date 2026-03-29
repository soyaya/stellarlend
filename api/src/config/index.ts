import dotenv from 'dotenv';

dotenv.config();

if (!process.env.CONTRACT_ID) {
  throw new Error('CONTRACT_ID environment variable is required');
}

const jwtSecret = process.env.JWT_SECRET;
if (!jwtSecret || jwtSecret === 'default-secret-change-me' || jwtSecret.length < 32) {
  throw new Error('JWT_SECRET must be set to a strong secret (min 32 chars)');
}

export const config = {
  server: {
    port: parseInt(process.env.PORT || '3000', 10),
    env: process.env.NODE_ENV || 'development',
  },
  stellar: {
    network: process.env.STELLAR_NETWORK || 'testnet',
    horizonUrl: process.env.HORIZON_URL || 'https://horizon-testnet.stellar.org',
    sorobanRpcUrl: process.env.SOROBAN_RPC_URL || 'https://soroban-testnet.stellar.org',
    networkPassphrase: process.env.NETWORK_PASSPHRASE || 'Test SDF Network ; September 2015',
    contractId: process.env.CONTRACT_ID || '',
    readOnlySimulationAccount:
      process.env.READ_ONLY_SIMULATION_ACCOUNT ||
      'GDZZJ3UPZZCKY5DBH6ZGMPMRORRBG4ECIORASBUAXPPNCL4SYRHNLYU2',
  },
  auth: {
    jwtSecret: process.env.JWT_SECRET as string,
    jwtExpiresIn: process.env.JWT_EXPIRES_IN || '24h',
  },
  rateLimit: {
    windowMs: parseInt(process.env.RATE_LIMIT_WINDOW_MS || '900000', 10),
    maxRequests: parseInt(process.env.RATE_LIMIT_MAX_REQUESTS || '100', 10),
  },
  logging: {
    level: process.env.LOG_LEVEL || 'info',
  },
  request: {
    timeout: parseInt(process.env.REQUEST_TIMEOUT || '30000', 10),
    maxRetries: parseInt(process.env.MAX_RETRIES || '3', 10),
    retryInitialDelayMs: parseInt(process.env.RETRY_INITIAL_DELAY_MS || '1000', 10),
    retryMaxDelayMs: parseInt(process.env.RETRY_MAX_DELAY_MS || '10000', 10),
  },
  cache: {
    idempotencyTtlMs: parseInt(process.env.IDEMPOTENCY_TTL_MS || '86400000', 10),
    idempotencyMaxEntries: parseInt(process.env.IDEMPOTENCY_MAX_ENTRIES || '1000', 10),
    protocolStatsTtlMs: parseInt(process.env.PROTOCOL_STATS_TTL_MS || '30000', 10),
  },
  ws: {
    /** Milliseconds between price poll cycles (default 30 s) */
    priceUpdateIntervalMs: parseInt(process.env.WS_PRICE_UPDATE_INTERVAL_MS || '30000', 10),
    /** Milliseconds between heartbeat pings (default 30 s) */
    heartbeatIntervalMs: parseInt(process.env.WS_HEARTBEAT_INTERVAL_MS || '30000', 10),
    /** Optional oracle service base URL for price data (e.g. http://localhost:4000) */
    oracleApiUrl: process.env.ORACLE_API_URL || '',
  },
};

/**
 * StellarLend Oracle Service
 * Off-chain oracle integration service that fetches price data from
 * multiple sources (CoinGecko, Binance)
 * @see https://github.com/stellarlend/stellarlend-contracts
 */

import { fileURLToPath } from 'node:url';
import { loadConfig, getSafeConfig, type OracleServiceConfig } from './config.js';
import { configureLogger, logger, logProviderHealth, logStalenessAlert } from './utils/logger.js';
import {
  createCoinGeckoProvider,
  createBinanceProvider,
  type BasePriceProvider,
} from './providers/index.js';
import {
  createValidator,
  createPriceCache,
  createPriceHistoryService,
  createAggregator,
  createContractUpdater,
  type PriceAggregator,
  type ContractUpdater,
} from './services/index.js';
import type { ProviderConfig } from './types/index.js';

/**
 * Default assets to fetch prices for
 */
const DEFAULT_ASSETS = ['XLM', 'USDC', 'BTC', 'ETH', 'SOL'];

function serializePricesForLog(prices: {
  asset: string;
  price: bigint;
  timestamp: number;
  confidence: number;
  sources: { source: string }[];
}[]) {
  return prices.map((price) => ({
    asset: price.asset,
    price: price.price.toString(),
    timestamp: price.timestamp,
    confidence: price.confidence,
    sources: price.sources.map((source) => source.source),
  }));
}

/**
 * Oracle Service
 */
export class OracleService {
  private config: OracleServiceConfig;
  private aggregator: PriceAggregator;
  private contractUpdater: ContractUpdater;
  private intervalId?: ReturnType<typeof setInterval>;
  private isRunning: boolean = false;
  private lastSuccessfulUpdate: number | null = null;

  constructor(config: OracleServiceConfig) {
    // Store config but never log adminSecretKey directly
    this.config = config;

    // Configure logging
    configureLogger(config.logLevel);

    // Create providers
    const providers: BasePriceProvider[] = [
      createCoinGeckoProvider(
        config.providers.find((p: ProviderConfig) => p.name === 'coingecko')?.apiKey
      ),
      createBinanceProvider(),
    ];

    // Create services
    const validator = createValidator({
      maxDeviationPercent: config.maxPriceDeviationPercent,
      maxStalenessSeconds: config.priceStaleThresholdSeconds,
    });

    const cache = createPriceCache(config.cacheTtlSeconds);
    const priceHistory = createPriceHistoryService();

    this.aggregator = createAggregator(providers, validator, cache, priceHistory, {
      circuitBreaker: config.circuitBreaker,
    });

    this.contractUpdater = createContractUpdater({
      network: config.stellarNetwork,
      rpcUrl: config.stellarRpcUrl,
      contractId: config.contractId,
      adminSecretKey: config.adminSecretKey,
      baseFee: config.baseFee,
      maxFee: config.maxFee,
      maxRetries: 3,
      retryDelayMs: 1000,
    });

    logger.info('Oracle service initialized', {
      network: config.stellarNetwork,
      contractId: config.contractId,
      dryRun: !!config.dryRun,
      updateInterval: config.updateIntervalMs,
      providers: this.aggregator.getProviders(),
    });
  }

  /**
   * Start the oracle service
   */
  async start(assets: string[] = DEFAULT_ASSETS): Promise<void> {
    if (this.isRunning) {
      logger.warn('Oracle service is already running');
      return;
    }

    this.isRunning = true;
    logger.info('Starting oracle service', { assets });

    // Run immediately on start
    await this.updatePrices(assets);

    // Schedule periodic updates
    this.intervalId = setInterval(async () => {
      await this.updatePrices(assets);
    }, this.config.updateIntervalMs);

    logger.info('Oracle service started', {
      intervalMs: this.config.updateIntervalMs,
    });
  }

  /**
   * Stop the oracle service
   */
  stop(): void {
    if (!this.isRunning) {
      logger.warn('Oracle service is not running');
      return;
    }

    if (this.intervalId) {
      clearInterval(this.intervalId);
      this.intervalId = undefined;
    }

    this.isRunning = false;
    logger.info('Oracle service stopped');
  }

  /**
   * Fetch and update prices for specified assets
   */
  async updatePrices(assets: string[]): Promise<void> {
    const startTime = Date.now();

    logger.info('Starting price update cycle', { assets });

    // Check for staleness
    if (this.lastSuccessfulUpdate) {
      const ageSeconds = (Date.now() - this.lastSuccessfulUpdate) / 1000;
      const thresholdSeconds = this.config.priceStaleThresholdSeconds;

      if (ageSeconds > thresholdSeconds) {
        logStalenessAlert(ageSeconds, thresholdSeconds, this.lastSuccessfulUpdate);
      }
    }

    try {
      // Fetch aggregated prices
      const prices = await this.aggregator.getPrices(assets);

      if (prices.size === 0) {
        logger.error('No prices fetched from any provider');
        return;
      }

      logger.info(`Fetched ${prices.size} prices`, {
        assets: Array.from(prices.keys()),
      });

      const priceArray = Array.from(prices.values());
      const serializedPrices = serializePricesForLog(priceArray);

      if (this.config.dryRun) {
        this.lastSuccessfulUpdate = Date.now();

        logger.info('DRY RUN: Would update prices on contract', {
          assets: serializedPrices.map((price) => price.asset),
          prices: serializedPrices,
          durationMs: Date.now() - startTime,
          contractId: this.config.contractId,
          dryRun: true,
        });

        return;
      }

      // Update contract
      const results = await this.contractUpdater.updatePrices(priceArray);

      // Log results
      const successful = results.filter((r) => r.success);
      const failed = results.filter((r) => !r.success);

      logger.info('Price update cycle complete', {
        successful: successful.length,
        failed: failed.length,
        durationMs: Date.now() - startTime,
      });

      if (successful.length > 0) {
        this.lastSuccessfulUpdate = Date.now();
      }

      if (failed.length > 0) {
        logger.warn('Some price updates failed', {
          failedAssets: failed.map((f) => f.asset),
        });
      }
    } catch (error) {
      logger.error('Price update cycle failed', { error });
    }
  }

  /**
   * Get current service status (safe for logging — secret key is masked)
   */
  getStatus() {
    const safe = getSafeConfig(this.config);
    return {
      isRunning: this.isRunning,
      network: safe.stellarNetwork,
      contractId: safe.contractId,
      adminSecretKey: safe.adminSecretKey, // masked value
      providers: this.aggregator.getProviders(),
      aggregatorStats: this.aggregator.getStats(),
      circuitBreakers: this.aggregator.getCircuitBreakerMetrics(),
    };
  }

  /**
   * Manually fetch price for a single asset (for testing)
   */
  async fetchPrice(asset: string) {
    return this.aggregator.getPrice(asset);
  }
}

/**
 * Main entry point
 */
async function main(): Promise<void> {
  logger.info('Starting StellarLend Oracle Service');

  try {
    // Load configuration
    const config = loadConfig();

    // Create and start service
    const service = new OracleService(config);

    // Handle shutdown
    process.on('SIGINT', () => {
      logger.info('Received SIGINT, shutting down...');
      service.stop();
      process.exit(0);
    });

    process.on('SIGTERM', () => {
      logger.info('Received SIGTERM, shutting down...');
      service.stop();
      process.exit(0);
    });

    // Start service
    await service.start();
  } catch (error) {
    logger.error('Failed to start oracle service', { error });
    process.exit(1);
  }
}

function isExecutedDirectly(): boolean {
  const entryFile = process.argv[1];

  if (!entryFile) {
    return false;
  }

  return fileURLToPath(import.meta.url) === entryFile;
}

// Run only when executed as the entrypoint, not when imported by tests/modules
if (isExecutedDirectly()) {
  main().catch((error) => {
    logger.error('Unhandled oracle service error', { error });
    process.exit(1);
  });
}

// Export for programmatic use
export { loadConfig, maskSecret, getSafeConfig } from './config.js';
export type { OracleServiceConfig } from './config.js';

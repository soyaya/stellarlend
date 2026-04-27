/**
 * Memory Leak Detection Test for Oracle Service
 *
 * This test runs the Oracle service update cycle many times
 * and monitors heap usage to ensure there are no linear memory leaks.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { OracleService } from '../src/index.js';
import type { OracleServiceConfig } from '../src/config.js';

// Mock contract updater to avoid actual blockchain calls
vi.mock('../src/services/contract-updater.js', () => ({
  createContractUpdater: vi.fn(() => ({
    updatePrices: vi
      .fn()
      .mockResolvedValue([{ success: true, asset: 'XLM', price: 150000n, timestamp: Date.now() }]),
    healthCheck: vi.fn().mockResolvedValue(true),
    getAdminPublicKey: vi.fn().mockReturnValue('GTEST123'),
  })),
  ContractUpdater: vi.fn(),
}));

// Mock providers to avoid actual API calls
vi.mock('../src/providers/coingecko.js', () => ({
  createCoinGeckoProvider: vi.fn(() => ({
    name: 'coingecko',
    isEnabled: true,
    priority: 1,
    weight: 0.6,
    getSupportedAssets: () => ['XLM', 'BTC', 'ETH'],
    fetchPrice: vi.fn().mockResolvedValue({
      asset: 'XLM',
      price: 0.15,
      timestamp: Math.floor(Date.now() / 1000),
      source: 'coingecko',
    }),
  })),
}));

vi.mock('../src/providers/binance.js', () => ({
  createBinanceProvider: vi.fn(() => ({
    name: 'binance',
    isEnabled: true,
    priority: 2,
    weight: 0.4,
    getSupportedAssets: () => ['XLM', 'BTC', 'ETH'],
    fetchPrice: vi.fn().mockResolvedValue({
      asset: 'XLM',
      price: 0.152,
      timestamp: Math.floor(Date.now() / 1000),
      source: 'binance',
    }),
  })),
}));

describe('OracleService Memory Leak Detection', () => {
  let mockConfig: OracleServiceConfig;

  beforeEach(() => {
    mockConfig = {
      stellarNetwork: 'testnet',
      stellarRpcUrl: 'https://soroban-testnet.stellar.org',
      contractId: 'CTEST123',
      adminSecretKey: 'STEST123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ123456',
      updateIntervalMs: 1000,
      maxPriceDeviationPercent: 10,
      priceStaleThresholdSeconds: 300,
      cacheTtlSeconds: 30,
      logLevel: 'error',
      providers: [
        {
          name: 'coingecko',
          enabled: true,
          priority: 1,
          weight: 0.6,
          baseUrl: 'https://api.coingecko.com/api/v3',
          rateLimit: { maxRequests: 10, windowMs: 60000 },
        },
        {
          name: 'binance',
          enabled: true,
          priority: 2,
          weight: 0.4,
          baseUrl: 'https://api.binance.com/api/v3',
          rateLimit: { maxRequests: 1200, windowMs: 60000 },
        },
      ],
    };
  });

  it('should maintain stable memory usage over 2000 update cycles', async () => {
    const service = new OracleService(mockConfig);
    const iterations = 2000;
    const assets = ['XLM', 'BTC', 'ETH', 'SOL'];
    const memoryCheckpoints: number[] = [];

    console.log(`Starting memory leak test: ${iterations} iterations`);

    // Warm-up phase (100 iterations) to let the engine stabilize
    for (let i = 0; i < 100; i++) {
      await service.updatePrices(assets);
    }

    const initialMemory = process.memoryUsage().heapUsed;
    console.log(`Initial memory after warm-up: ${(initialMemory / 1024 / 1024).toFixed(2)} MB`);

    // Measurement phase
    for (let i = 0; i < iterations; i++) {
      await service.updatePrices(assets);

      // Check memory every 500 iterations
      if (i % 500 === 0) {
        const currentMemory = process.memoryUsage().heapUsed;
        memoryCheckpoints.push(currentMemory);
        console.log(`Iteration ${i}: ${(currentMemory / 1024 / 1024).toFixed(2)} MB`);
      }
    }

    const finalMemory = process.memoryUsage().heapUsed;
    console.log(`Final memory: ${(finalMemory / 1024 / 1024).toFixed(2)} MB`);

    const memoryIncrease = finalMemory - initialMemory;
    const memoryIncreasePercent = (memoryIncrease / initialMemory) * 100;

    console.log(
      `Memory increase: ${(memoryIncrease / 1024 / 1024).toFixed(2)} MB (${memoryIncreasePercent.toFixed(2)}%)`
    );

    // A leak would typically show much larger growth.
    // We allow for some growth due to Node.js's lazy garbage collection,
    // but anything over 50% increase in 2000 iterations of mocked work is suspicious.
    // Ideally it should be very close to 0 or even negative if GC kicks in.
    expect(memoryIncreasePercent).toBeLessThan(55);
  }, 60000); // 1 minute timeout
});

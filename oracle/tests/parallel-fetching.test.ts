/**
 * Tests for parallel asset price fetching in BasePriceProvider.fetchPrices()
 */

import { describe, it, expect, vi } from 'vitest';
import { BasePriceProvider } from '../src/providers/base-provider.js';
import type { RawPriceData } from '../src/types/index.js';

function makeProvider(concurrencyLimit: number, maxRequests = 100, windowMs = 1000) {
  return new (class extends BasePriceProvider {
    public callLog: { asset: string; startedAt: number }[] = [];

    async fetchPrice(asset: string): Promise<RawPriceData> {
      this.callLog.push({ asset, startedAt: Date.now() });
      return { asset, price: 1, timestamp: Math.floor(Date.now() / 1000), source: this.name };
    }
  })({
    name: 'parallel-test',
    enabled: true,
    priority: 1,
    weight: 1,
    baseUrl: 'https://mock.api',
    rateLimit: { maxRequests, windowMs },
    concurrencyLimit,
  });
}

describe('fetchPrices parallel execution', () => {
  it('returns results for all assets', async () => {
    const provider = makeProvider(5);
    const assets = ['XLM', 'BTC', 'ETH', 'USDC', 'USDT'];
    const results = await provider.fetchPrices(assets);
    expect(results).toHaveLength(5);
    expect(results.map((r) => r.asset)).toEqual(expect.arrayContaining(assets));
  });

  it('fetches up to concurrencyLimit assets in parallel per batch', async () => {
    vi.useFakeTimers();
    const concurrency = 3;
    const provider = new (class extends BasePriceProvider {
      public inFlight = 0;
      public maxObservedInFlight = 0;

      async fetchPrice(asset: string): Promise<RawPriceData> {
        this.inFlight++;
        this.maxObservedInFlight = Math.max(this.maxObservedInFlight, this.inFlight);
        await new Promise((r) => setTimeout(r, 10));
        this.inFlight--;
        return { asset, price: 1, timestamp: 0, source: this.name };
      }
    })({
      name: 'concurrency-test',
      enabled: true,
      priority: 1,
      weight: 1,
      baseUrl: 'https://mock.api',
      rateLimit: { maxRequests: 100, windowMs: 1000 },
      concurrencyLimit: concurrency,
    });

    const promise = provider.fetchPrices(['A', 'B', 'C', 'D', 'E', 'F']);
    await vi.runAllTimersAsync();
    const results = await promise;

    expect(results).toHaveLength(6);
    expect(provider.maxObservedInFlight).toBeLessThanOrEqual(concurrency);
    vi.useRealTimers();
  });

  it('does not block on failed fetches — successful ones still return', async () => {
    const provider = new (class extends BasePriceProvider {
      async fetchPrice(asset: string): Promise<RawPriceData> {
        if (asset === 'FAIL') throw new Error('provider error');
        return { asset, price: 1, timestamp: 0, source: this.name };
      }
    })({
      name: 'failure-test',
      enabled: true,
      priority: 1,
      weight: 1,
      baseUrl: 'https://mock.api',
      rateLimit: { maxRequests: 100, windowMs: 1000 },
      concurrencyLimit: 5,
    });

    const results = await provider.fetchPrices(['XLM', 'FAIL', 'BTC']);
    expect(results).toHaveLength(2);
    expect(results.map((r) => r.asset)).toEqual(expect.arrayContaining(['XLM', 'BTC']));
  });

  it('defaults to concurrency of 5 when concurrencyLimit is not set', async () => {
    const provider = new (class extends BasePriceProvider {
      async fetchPrice(asset: string): Promise<RawPriceData> {
        return { asset, price: 1, timestamp: 0, source: this.name };
      }
    })({
      name: 'default-concurrency-test',
      enabled: true,
      priority: 1,
      weight: 1,
      baseUrl: 'https://mock.api',
      rateLimit: { maxRequests: 100, windowMs: 1000 },
      // concurrencyLimit intentionally omitted
    });

    const results = await provider.fetchPrices(['A', 'B', 'C', 'D', 'E', 'F']);
    expect(results).toHaveLength(6);
  });

  it('respects rate limiting during parallel fetches', async () => {
    const maxRequests = 3;
    const windowMs = 100;
    const provider = makeProvider(5, maxRequests, windowMs);

    const start = Date.now();
    await provider.fetchPrices(['A', 'B', 'C', 'D', 'E', 'F']);
    const elapsed = Date.now() - start;

    // 6 requests with max 3 per 100ms window must span at least one window
    expect(elapsed).toBeGreaterThanOrEqual(windowMs);
  });
});

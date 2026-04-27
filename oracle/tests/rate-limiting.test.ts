/**
 * Load tests for provider rate limiting under concurrent requests.
 */

import { describe, it, expect, vi } from 'vitest';
import { BasePriceProvider } from '../src/providers/base-provider.js';
import type { RawPriceData } from '../src/types/index.js';

class RateLimitTestProvider extends BasePriceProvider {
  public completionTimes: number[] = [];

  constructor(maxRequests: number, windowMs: number) {
    super({
      name: 'rate-limit-test-provider',
      enabled: true,
      priority: 1,
      weight: 1,
      baseUrl: 'https://mock.api',
      rateLimit: { maxRequests, windowMs },
    });
  }

  async fetchPrice(asset: string): Promise<RawPriceData> {
    await this.enforceRateLimit();
    this.completionTimes.push(Date.now());
    return {
      asset: asset.toUpperCase(),
      price: 1,
      timestamp: Math.floor(Date.now() / 1000),
      source: this.name,
    };
  }
}

describe('Oracle rate limiting load tests', () => {
  it('queues concurrent requests above configured rate limit', async () => {
    const provider = new RateLimitTestProvider(5, 50);
    const start = Date.now();

    await Promise.all(Array.from({ length: 20 }, () => provider.fetchPrice('XLM')));

    const elapsed = Date.now() - start;
    expect(provider.completionTimes).toHaveLength(20);
    // At least one batch must be deferred beyond the initial window.
    const deferredCount = provider.completionTimes.filter((t) => t - start >= 50).length;
    expect(deferredCount).toBeGreaterThan(0);
    expect(elapsed).toBeGreaterThanOrEqual(50);
  });

  it('does not allow more than maxRequests in the first window', async () => {
    const maxRequests = 4;
    const windowMs = 60;
    const provider = new RateLimitTestProvider(maxRequests, windowMs);
    const start = Date.now();

    await Promise.all(Array.from({ length: 12 }, () => provider.fetchPrice('XLM')));

    const inFirstWindow = provider.completionTimes.filter((t) => t - start < windowMs).length;
    expect(inFirstWindow).toBeLessThanOrEqual(maxRequests);
    expect(provider.completionTimes).toHaveLength(12);
  });

  it('prevents boundary bursts (moving window, inclusive)', async () => {
    const maxRequests = 3;
    const windowMs = 100;

    vi.useFakeTimers();
    vi.setSystemTime(0);

    const provider = new RateLimitTestProvider(maxRequests, windowMs);

    // First burst: fills the limiter.
    await Promise.all(Array.from({ length: maxRequests }, () => provider.fetchPrice('XLM')));
    expect(provider.completionTimes).toHaveLength(maxRequests);
    expect(provider.completionTimes.every((t) => t === 0)).toBe(true);

    // Second burst starts just before the boundary. A fixed-window limiter
    // can let this burst through immediately at t=100; the moving window
    // should delay it past t=100.
    vi.setSystemTime(windowMs - 1); // 99
    const secondBurst = Promise.all(
      Array.from({ length: maxRequests }, () => provider.fetchPrice('XLM'))
    );

    // Drive timers enough for the queued sleeps to resolve.
    await vi.advanceTimersByTimeAsync(10);
    await secondBurst;

    expect(provider.completionTimes).toHaveLength(maxRequests * 2);

    const withinInclusiveBoundary = provider.completionTimes.filter((t) => t <= windowMs).length;
    expect(withinInclusiveBoundary).toBeLessThanOrEqual(maxRequests);

    vi.useRealTimers();
  });
});

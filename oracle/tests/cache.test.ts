/**
 * Tests for Cache Service
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { Cache, PriceCache, createCache, createPriceCache } from '../src/services/cache.js';

describe('Cache', () => {
  let cache: Cache;

  beforeEach(() => {
    cache = createCache({
      defaultTtlSeconds: 10,
      maxEntries: 100,
    });
  });

  describe('get/set', () => {
    it('should store and retrieve values', async () => {
      await cache.set('key1', 'value1');

      expect(await cache.get('key1')).toBe('value1');
    });

    it('should return undefined for missing keys', async () => {
      expect(await cache.get('nonexistent')).toBeUndefined();
    });

    it('should handle different data types', async () => {
      await cache.set('string', 'hello');
      await cache.set('number', 42);
      await cache.set('object', { foo: 'bar' });
      await cache.set('array', [1, 2, 3]);
      await cache.set('bigint', 12345678901234567890n);

      expect(await cache.get('string')).toBe('hello');
      expect(await cache.get('number')).toBe(42);
      expect(await cache.get('object')).toEqual({ foo: 'bar' });
      expect(await cache.get('array')).toEqual([1, 2, 3]);
      expect(await cache.get('bigint')).toBe(12345678901234567890n);
    });
  });

  describe('TTL expiration', () => {
    it('should expire entries after TTL', async () => {
      cache = createCache({ defaultTtlSeconds: 0.1 });
      await cache.set('temp', 'value');

      expect(await cache.get('temp')).toBe('value');

      await new Promise((r) => setTimeout(r, 150));

      expect(await cache.get('temp')).toBeUndefined();
    });

    it('should use custom TTL when provided', async () => {
      await cache.set('custom', 'value', 0.05);

      expect(await cache.get('custom')).toBe('value');

      await new Promise((r) => setTimeout(r, 100));

      expect(await cache.get('custom')).toBeUndefined();
    });
  });

  describe('has', () => {
    it('should return true for existing keys', async () => {
      await cache.set('exists', 'value');

      expect(await cache.has('exists')).toBe(true);
    });

    it('should return false for missing keys', async () => {
      expect(await cache.has('missing')).toBe(false);
    });

    it('should return false for expired keys', async () => {
      cache = createCache({ defaultTtlSeconds: 0.05 });
      await cache.set('expires', 'value');

      await new Promise((r) => setTimeout(r, 100));

      expect(await cache.has('expires')).toBe(false);
    });
  });

  describe('delete', () => {
    it('should delete existing keys', async () => {
      await cache.set('toDelete', 'value');

      expect(await cache.delete('toDelete')).toBe(true);
      expect(await cache.get('toDelete')).toBeUndefined();
    });

    it('should return false for non-existent keys', async () => {
      expect(await cache.delete('nonexistent')).toBe(false);
    });
  });

  describe('clear', () => {
    it('should remove all entries', async () => {
      await cache.set('key1', 'value1');
      await cache.set('key2', 'value2');
      await cache.set('key3', 'value3');

      await cache.clear();

      expect(await cache.get('key1')).toBeUndefined();
      expect(await cache.get('key2')).toBeUndefined();
      expect(await cache.get('key3')).toBeUndefined();
    });
  });

  describe('stats', () => {
    it('should track hits and misses', async () => {
      await cache.set('hit', 'value');

      await cache.get('hit');
      await cache.get('hit');
      await cache.get('miss');

      const stats = cache.getStats();

      expect(stats.hits).toBe(2);
      expect(stats.misses).toBe(1);
      expect(stats.hitRate).toBeCloseTo(0.667, 2);
    });

    it('should track size', async () => {
      await cache.set('a', 1);
      await cache.set('b', 2);
      await cache.set('c', 3);

      const stats = cache.getStats();

      expect(stats.size).toBe(3);
    });

    it('should report eviction count', async () => {
      cache = createCache({ maxEntries: 3, evictBatchFraction: 0.1 });

      await cache.set('a', 1);
      await cache.set('b', 2);
      await cache.set('c', 3);
      await cache.set('d', 4);

      const stats = cache.getStats();
      expect(stats.evictions).toBeGreaterThanOrEqual(1);
    });
  });

  describe('LRU eviction', () => {
    it('should evict least recently used entry first (single eviction)', async () => {
      cache = createCache({ maxEntries: 3, evictBatchFraction: 0.1 });

      await cache.set('first', 1);
      await cache.set('second', 2);
      await cache.set('third', 3);
      await cache.get('first');
      await cache.set('fourth', 4);

      expect(await cache.get('second')).toBeUndefined();
      expect(await cache.get('first')).toBe(1);
      expect(await cache.get('third')).toBe(3);
      expect(await cache.get('fourth')).toBe(4);
    });

    it('should evict a batch of LRU entries when at capacity', async () => {
      cache = createCache({ maxEntries: 10, evictBatchFraction: 0.5 });

      for (let i = 0; i < 10; i++) {
        await cache.set(`key${i}`, i);
      }

      for (let i = 5; i < 10; i++) {
        await cache.get(`key${i}`);
      }

      await cache.set('new', 99);

      for (let i = 0; i < 5; i++) {
        expect(await cache.get(`key${i}`)).toBeUndefined();
      }
      for (let i = 5; i < 10; i++) {
        expect(await cache.get(`key${i}`)).toBe(i);
      }
      expect(await cache.get('new')).toBe(99);
    });

    it('should evict 10% batch by default when at capacity', async () => {
      cache = createCache({ maxEntries: 10, evictBatchFraction: 0.1 });

      for (let i = 0; i < 10; i++) {
        await cache.set(`key${i}`, i);
      }

      await cache.set('extra', 100);

      expect(await cache.get('key0')).toBeUndefined();
      expect(cache.getStats().evictions).toBe(1);
    });

    it('should update LRU order when a key is overwritten', async () => {
      cache = createCache({ maxEntries: 3, evictBatchFraction: 0.1 });

      await cache.set('a', 1);
      await cache.set('b', 2);
      await cache.set('c', 3);
      await cache.set('a', 10);
      await cache.set('d', 4);

      expect(await cache.get('b')).toBeUndefined();
      expect(await cache.get('a')).toBe(10);
      expect(await cache.get('c')).toBe(3);
      expect(await cache.get('d')).toBe(4);
    });
  });

  describe('eviction under load', () => {
    it('should handle rapid insertions without exceeding maxEntries by more than batchSize', async () => {
      const maxEntries = 100;
      cache = createCache({ maxEntries, evictBatchFraction: 0.1 });

      for (let i = 0; i < 200; i++) {
        await cache.set(`load-key-${i}`, i);
        expect(cache.getStats().size).toBeLessThanOrEqual(maxEntries);
      }

      const stats = cache.getStats();
      expect(stats.evictions).toBeGreaterThan(0);
      expect(stats.size).toBeLessThanOrEqual(maxEntries);
    });

    it('should maintain high hit rate when recently set keys are accessed', async () => {
      cache = createCache({ maxEntries: 50, evictBatchFraction: 0.1 });

      for (let i = 0; i < 50; i++) {
        await cache.set(`k${i}`, i);
      }

      for (let i = 0; i < 50; i++) {
        await cache.get(`k${i}`);
      }

      const stats = cache.getStats();
      expect(stats.hitRate).toBeGreaterThan(0.8);
    });
  });

  describe('cleanup', () => {
    it('should remove expired entries', async () => {
      cache = createCache({ defaultTtlSeconds: 0.05 });

      await cache.set('expire1', 1);
      await cache.set('expire2', 2);

      await new Promise((r) => setTimeout(r, 100));

      const cleaned = cache.cleanup();

      expect(cleaned).toBe(2);
      expect(cache.getStats().size).toBe(0);
    });
  });
});

describe('PriceCache', () => {
  let priceCache: PriceCache;

  beforeEach(() => {
    priceCache = createPriceCache(30);
  });

  describe('price operations', () => {
    it('should store and retrieve prices as bigint', async () => {
      const price = 150000n;

      await priceCache.setPrice('XLM', price);

      expect(await priceCache.getPrice('XLM')).toBe(price);
    });

    it('should normalize asset symbols to uppercase', async () => {
      await priceCache.setPrice('xlm', 150000n);

      expect(await priceCache.getPrice('XLM')).toBe(150000n);
      expect(await priceCache.getPrice('xlm')).toBe(150000n);
    });

    it('should check if price exists', async () => {
      await priceCache.setPrice('BTC', 50000000000n);

      expect(await priceCache.hasPrice('BTC')).toBe(true);
      expect(await priceCache.hasPrice('ETH')).toBe(false);
    });
  });

  describe('clear', () => {
    it('should clear all prices', async () => {
      await priceCache.setPrice('XLM', 150000n);
      await priceCache.setPrice('BTC', 50000000000n);

      await priceCache.clear();

      expect(await priceCache.hasPrice('XLM')).toBe(false);
      expect(await priceCache.hasPrice('BTC')).toBe(false);
    });
  });

  describe('stats', () => {
    it('should return cache statistics', async () => {
      await priceCache.setPrice('XLM', 150000n);
      await priceCache.getPrice('XLM');
      await priceCache.getPrice('ETH');

      const stats = priceCache.getStats();

      expect(stats.hits).toBe(1);
      expect(stats.misses).toBe(1);
    });

    it('should include eviction count in stats', () => {
      const stats = priceCache.getStats();
      expect(stats.evictions).toBeDefined();
      expect(stats.evictions).toBeGreaterThanOrEqual(0);
    });
  });
});

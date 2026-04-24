/**
 * Tests for Price History Service
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { PriceHistoryService, createPriceHistoryService } from '../src/services/price-history.js';
import type { AggregatedPrice } from '../src/types/index.js';

describe('PriceHistoryService', () => {
    let service: PriceHistoryService;

    beforeEach(() => {
        vi.clearAllMocks();
        service = createPriceHistoryService({ maxEntries: 5 });
    });

    describe('initialization', () => {
        it('should create service with default config', () => {
            const defaultService = createPriceHistoryService();
            expect(defaultService).toBeDefined();
            expect(defaultService).toBeInstanceOf(PriceHistoryService);
        });

        it('should create service with custom config', () => {
            const customService = createPriceHistoryService({ maxEntries: 10 });
            expect(customService).toBeDefined();
            
            const stats = customService.getStats();
            expect(stats.maxEntriesPerAsset).toBe(10);
        });

        it('should start with empty history', () => {
            const stats = service.getStats();
            expect(stats.trackedAssets).toBe(0);
            expect(stats.totalEntries).toBe(0);
            expect(stats.assets).toEqual([]);
        });
    });

    describe('addPriceEntry', () => {
        it('should add price entry for new asset', () => {
            const timestamp = Date.now();
            service.addPriceEntry('XLM', 150000n, timestamp);

            const history = service.getPriceHistory('XLM');
            expect(history).toHaveLength(1);
            expect(history[0]).toEqual({
                price: 150000n,
                timestamp,
            });

            const stats = service.getStats();
            expect(stats.trackedAssets).toBe(1);
            expect(stats.totalEntries).toBe(1);
            expect(stats.assets).toContain('XLM');
        });

        it('should handle asset case normalization', () => {
            service.addPriceEntry('xlm', 150000n, Date.now());
            service.addPriceEntry('Xlm', 160000n, Date.now());

            const history = service.getPriceHistory('XLM');
            expect(history).toHaveLength(2);

            const assets = service.getAssets();
            expect(assets).toContain('XLM');
            expect(assets).not.toContain('xlm');
            expect(assets).not.toContain('Xlm');
        });

        it('should add multiple entries for same asset', () => {
            const baseTime = Date.now();
            for (let i = 0; i < 3; i++) {
                service.addPriceEntry('BTC', 50000000000n + BigInt(i * 1000), baseTime + i * 1000);
            }

            const history = service.getPriceHistory('BTC');
            expect(history).toHaveLength(3);
            expect(history[0].price).toBe(50000000000n);
            expect(history[2].price).toBe(50000002000n);
        });

        it('should handle circular buffer behavior', () => {
            // Fill beyond capacity
            for (let i = 0; i < 7; i++) {
                service.addPriceEntry('ETH', 1000000000n + BigInt(i * 1000), Date.now() + i * 1000);
            }

            const history = service.getPriceHistory('ETH');
            expect(history).toHaveLength(5); // Should be limited to maxEntries
            
            // Should contain the last 5 entries (circular buffer)
            expect(history[0].price).toBe(1000002000n); // Entry at index 2
            expect(history[4].price).toBe(1000006000n); // Entry at index 6
        });

        it('should add aggregated price entry', () => {
            const aggregatedPrice: AggregatedPrice = {
                asset: 'USDC',
                price: 1000000n,
                sources: [],
                timestamp: Math.floor(Date.now() / 1000),
                confidence: 95,
            };

            service.addAggregatedPrice(aggregatedPrice);

            const history = service.getPriceHistory('USDC');
            expect(history).toHaveLength(1);
            expect(history[0]).toEqual({
                price: 1000000n,
                timestamp: aggregatedPrice.timestamp,
            });
        });
    });

    describe('getPriceHistory', () => {
        beforeEach(() => {
            const baseTime = Date.now();
            for (let i = 0; i < 5; i++) {
                service.addPriceEntry('XLM', 150000n + BigInt(i * 1000), baseTime + i * 1000);
            }
        });

        it('should return all entries for asset', () => {
            const history = service.getPriceHistory('XLM');
            expect(history).toHaveLength(5);
            expect(history[0].price).toBe(150000n);
            expect(history[4].price).toBe(154000n);
        });

        it('should return limited entries', () => {
            const history = service.getPriceHistory('XLM', 3);
            expect(history).toHaveLength(3);
        });

        it('should return empty array for non-existent asset', () => {
            const history = service.getPriceHistory('NONEXISTENT');
            expect(history).toHaveLength(0);
        });

        it('should return entries in chronological order for circular buffer', () => {
            // Fill beyond capacity to test circular buffer ordering
            for (let i = 5; i < 8; i++) {
                service.addPriceEntry('XLM', 150000n + BigInt(i * 1000), Date.now() + i * 1000);
            }

            const history = service.getPriceHistory('XLM');
            expect(history).toHaveLength(5);
            
            // Should be in chronological order (oldest to newest)
            for (let i = 1; i < history.length; i++) {
                expect(history[i].timestamp).toBeGreaterThan(history[i - 1].timestamp);
            }
        });
    });

    describe('calculateTWAP', () => {
        beforeEach(() => {
            const baseTime = Math.floor(Date.now() / 1000);
            // Add entries with 1-second intervals
            for (let i = 0; i < 5; i++) {
                service.addPriceEntry('BTC', 50000000000n + BigInt(i * 1000000), baseTime - (4 - i) * 1000);
            }
        });

        it('should calculate TWAP for time period', () => {
            const twap = service.calculateTWAP('BTC', 3000); // 3 seconds
            
            expect(twap).toBeDefined();
            expect(twap!.asset).toBe('BTC');
            expect(twap!.periodSeconds).toBe(3000);
            expect(twap!.dataPoints).toBeGreaterThan(1);
            expect(twap!.startTime).toBeDefined();
            expect(twap!.endTime).toBeDefined();
            expect(twap!.twap).toBeGreaterThan(0n);
        });

        it('should return null for insufficient data', () => {
            service.addPriceEntry('NEW', 100000n, Date.now());
            
            const twap = service.calculateTWAP('NEW', 3000);
            expect(twap).toBeNull();
        });

        it('should return null for non-existent asset', () => {
            const twap = service.calculateTWAP('NONEXISTENT', 3000);
            expect(twap).toBeNull();
        });

        it('should handle large time periods gracefully', () => {
            const twap = service.calculateTWAP('BTC', 999999); // Very large period
            expect(twap).toBeDefined();
            expect(twap!.dataPoints).toBeGreaterThan(0);
        });

        it('should calculate reasonable TWAP values', () => {
            const twap = service.calculateTWAP('BTC', 5000);
            
            // TWAP should be between min and max prices
            const history = service.getPriceHistory('BTC');
            const prices = history.map(h => h.price);
            const minPrice = prices.reduce((a, b) => a < b ? a : b);
            const maxPrice = prices.reduce((a, b) => a > b ? a : b);
            
            expect(twap!.twap).toBeGreaterThanOrEqual(minPrice);
            expect(twap!.twap).toBeLessThanOrEqual(maxPrice);
        });
    });

    describe('getLatestPrice', () => {
        beforeEach(() => {
            const baseTime = Date.now();
            for (let i = 0; i < 3; i++) {
                service.addPriceEntry('ETH', 1000000000n + BigInt(i * 1000), baseTime + i * 1000);
            }
        });

        it('should return latest price for asset', () => {
            const latest = service.getLatestPrice('ETH');
            expect(latest).toBeDefined();
            expect(latest!.price).toBe(1000002000n); // Last entry
        });

        it('should return null for non-existent asset', () => {
            const latest = service.getLatestPrice('NONEXISTENT');
            expect(latest).toBeNull();
        });

        it('should return null for empty history', () => {
            const latest = service.getLatestPrice('EMPTY');
            expect(latest).toBeNull();
        });
    });

    describe('getAssetStats', () => {
        beforeEach(() => {
            const baseTime = Date.now();
            service.addPriceEntry('XLM', 150000n, baseTime);
            service.addPriceEntry('XLM', 160000n, baseTime + 1000);
            service.addPriceEntry('XLM', 140000n, baseTime + 2000);
        });

        it('should return asset statistics', () => {
            const stats = service.getAssetStats('XLM');
            
            expect(stats.totalEntries).toBe(3);
            expect(stats.oldestTimestamp).toBeDefined();
            expect(stats.newestTimestamp).toBeDefined();
            expect(stats.priceRange).toBeDefined();
            expect(stats.priceRange!.min).toBe(140000n);
            expect(stats.priceRange!.max).toBe(160000n);
        });

        it('should return empty stats for non-existent asset', () => {
            const stats = service.getAssetStats('NONEXISTENT');
            expect(stats.totalEntries).toBe(0);
            expect(stats.oldestTimestamp).toBeUndefined();
            expect(stats.newestTimestamp).toBeUndefined();
            expect(stats.priceRange).toBeUndefined();
        });
    });

    describe('clearHistory', () => {
        beforeEach(() => {
            service.addPriceEntry('XLM', 150000n, Date.now());
            service.addPriceEntry('BTC', 50000000000n, Date.now());
        });

        it('should clear history for specific asset', () => {
            service.clearHistory('XLM');
            
            expect(service.getPriceHistory('XLM')).toHaveLength(0);
            expect(service.getPriceHistory('BTC')).toHaveLength(1);
            expect(service.getAssets()).not.toContain('XLM');
            expect(service.getAssets()).toContain('BTC');
        });

        it('should clear all history', () => {
            service.clearAllHistory();
            
            expect(service.getPriceHistory('XLM')).toHaveLength(0);
            expect(service.getPriceHistory('BTC')).toHaveLength(0);
            expect(service.getAssets()).toHaveLength(0);
            
            const stats = service.getStats();
            expect(stats.trackedAssets).toBe(0);
            expect(stats.totalEntries).toBe(0);
        });
    });

    describe('getAssets', () => {
        it('should return list of tracked assets', () => {
            service.addPriceEntry('XLM', 150000n, Date.now());
            service.addPriceEntry('BTC', 50000000000n, Date.now());
            service.addPriceEntry('ETH', 1000000000n, Date.now());

            const assets = service.getAssets();
            expect(assets).toHaveLength(3);
            expect(assets).toContain('XLM');
            expect(assets).toContain('BTC');
            expect(assets).toContain('ETH');
        });

        it('should return empty list when no assets tracked', () => {
            const assets = service.getAssets();
            expect(assets).toHaveLength(0);
        });
    });

    describe('getStats', () => {
        it('should return comprehensive statistics', () => {
            service.addPriceEntry('XLM', 150000n, Date.now());
            service.addPriceEntry('BTC', 50000000000n, Date.now());

            // Fill XLM beyond capacity to test circular buffer counting
            for (let i = 0; i < 5; i++) {
                service.addPriceEntry('XLM', 150000n + BigInt(i * 1000), Date.now() + i * 1000);
            }

            const stats = service.getStats();
            
            expect(stats.trackedAssets).toBe(2);
            expect(stats.totalEntries).toBeGreaterThan(0);
            expect(stats.maxEntriesPerAsset).toBe(5);
            expect(stats.assets).toContain('XLM');
            expect(stats.assets).toContain('BTC');
        });
    });

    describe('edge cases', () => {
        it('should handle zero price values', () => {
            service.addPriceEntry('ZERO', 0n, Date.now());
            
            const history = service.getPriceHistory('ZERO');
            expect(history).toHaveLength(1);
            expect(history[0].price).toBe(0n);
        });

        it('should handle very large price values', () => {
            const largePrice = 999999999999999999n;
            service.addPriceEntry('LARGE', largePrice, Date.now());
            
            const history = service.getPriceHistory('LARGE');
            expect(history).toHaveLength(1);
            expect(history[0].price).toBe(largePrice);
        });

        it('should handle duplicate timestamps', () => {
            const timestamp = Date.now();
            service.addPriceEntry('DUP', 100000n, timestamp);
            service.addPriceEntry('DUP', 110000n, timestamp);
            
            const history = service.getPriceHistory('DUP');
            expect(history).toHaveLength(2);
            expect(history[0].timestamp).toBe(timestamp);
            expect(history[1].timestamp).toBe(timestamp);
        });
    });
});

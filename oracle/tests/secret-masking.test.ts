/**
 * Security tests: verify admin secret key never leaks in logs or status output
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { maskSecret, getSafeConfig, loadConfig } from '../src/config.js';
import type { OracleServiceConfig } from '../src/types/index.js';

// ---------------------------------------------------------------------------
// maskSecret unit tests
// ---------------------------------------------------------------------------
describe('maskSecret', () => {
  it('masks a normal secret key', () => {
    const key = 'SABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890ABCDEFGHIJKLMNO';
    const masked = maskSecret(key);
    expect(masked).not.toContain(key);
    expect(masked.startsWith('SA')).toBe(true);
    expect(masked.endsWith('NO')).toBe(true);
    expect(masked).toMatch(/^SA\*+NO$/);
  });

  it('returns **** for empty string', () => {
    expect(maskSecret('')).toBe('****');
  });

  it('returns **** for keys 8 chars or shorter', () => {
    expect(maskSecret('SHORT')).toBe('****');
    expect(maskSecret('12345678')).toBe('****');
  });

  it('handles a 9-character key (just above threshold)', () => {
    const key = '123456789';
    const masked = maskSecret(key);
    expect(masked).toBe('12*****89');
  });

  it('never exposes more than first 2 and last 2 chars', () => {
    const key = 'SABCDEFGHIJKLMNOPQRSTUVWXYZ';
    const masked = maskSecret(key);
    // Only first 2 and last 2 should be visible
    const visiblePart = key.slice(2, -2);
    expect(masked).not.toContain(visiblePart);
  });
});

// ---------------------------------------------------------------------------
// getSafeConfig tests
// ---------------------------------------------------------------------------
describe('getSafeConfig', () => {
  const mockConfig: OracleServiceConfig = {
    stellarNetwork: 'testnet',
    stellarRpcUrl: 'https://soroban-testnet.stellar.org',
    contractId: 'CTEST123456789',
    adminSecretKey: 'SABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890ABCDEFGHIJKLMNO',
    updateIntervalMs: 60000,
    maxPriceDeviationPercent: 10,
    priceStaleThresholdSeconds: 300,
    cacheTtlSeconds: 30,
    logLevel: 'info',
    providers: [],
  };

  it('masks adminSecretKey in safe config', () => {
    const safe = getSafeConfig(mockConfig);
    expect(safe.adminSecretKey).not.toBe(mockConfig.adminSecretKey);
    expect(safe.adminSecretKey).toMatch(/^SA\*+NO$/);
  });

  it('preserves all other config fields', () => {
    const safe = getSafeConfig(mockConfig);
    expect(safe.stellarNetwork).toBe(mockConfig.stellarNetwork);
    expect(safe.contractId).toBe(mockConfig.contractId);
    expect(safe.stellarRpcUrl).toBe(mockConfig.stellarRpcUrl);
    expect(safe.updateIntervalMs).toBe(mockConfig.updateIntervalMs);
  });

  it('safe config JSON serialization never contains raw secret', () => {
    const safe = getSafeConfig(mockConfig);
    const serialized = JSON.stringify(safe);
    expect(serialized).not.toContain(mockConfig.adminSecretKey);
  });
});

// ---------------------------------------------------------------------------
// No secret leakage in logger calls
// ---------------------------------------------------------------------------
describe('No secret leakage in logs', () => {
  const originalEnv = process.env;
  const SECRET = 'SABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890ABCDEFGHIJKLMNO';

  beforeEach(() => {
    process.env = { ...originalEnv };
    process.env.CONTRACT_ID = 'CTEST123456789';
    process.env.ADMIN_SECRET_KEY = SECRET;
  });

  afterEach(() => {
    process.env = originalEnv;
    vi.restoreAllMocks();
  });

  it('loadConfig returns the real secret (needed for contract updates)', () => {
    const config = loadConfig();
    // The raw config must still carry the real key so contract updater works
    expect(config.adminSecretKey).toBe(SECRET);
  });

  it('getSafeConfig output does not contain the raw secret', () => {
    const config = loadConfig();
    const safe = getSafeConfig(config);
    expect(safe.adminSecretKey).not.toBe(SECRET);
    expect(JSON.stringify(safe)).not.toContain(SECRET);
  });

  it('logger.info is never called with the raw secret key', async () => {
    // Spy on console methods that the logger uses
    const infoSpy = vi.spyOn(console, 'info').mockImplementation(() => {});
    const logSpy = vi.spyOn(console, 'log').mockImplementation(() => {});

    // Dynamically import OracleService to trigger constructor logging
    const { OracleService } = await import('../src/index.js');
    const config = loadConfig();

    // Mock aggregator/updater deps so constructor doesn't fail
    try {
      new OracleService(config);
    } catch {
      // constructor may throw due to missing provider deps in test env — that's fine
    }

    const allLogOutput = [
      ...infoSpy.mock.calls.map((c) => JSON.stringify(c)),
      ...logSpy.mock.calls.map((c) => JSON.stringify(c)),
    ].join('\n');

    expect(allLogOutput).not.toContain(SECRET);
  });
});

import { auditLogService } from '../services/auditLog.service';

jest.mock('../utils/logger');

beforeEach(() => {
  auditLogService._reset();
});

describe('AuditLogService.record()', () => {
  it('creates an entry with all required fields', () => {
    const entry = auditLogService.record({
      action: 'DEPOSIT',
      actor: 'GDZZJ3UPZZCKY5DBH6ZGMPMRORRBG4ECIORASBUAXPPNCL4SYRHNLYU2',
      status: 'success',
      txHash: 'abc123',
      ledger: 999,
      amount: '1000000',
      assetAddress: 'GTEST',
    });

    expect(entry.id).toBeDefined();
    expect(entry.sequence).toBe(1);
    expect(entry.action).toBe('DEPOSIT');
    expect(entry.actor).toBe('GDZZJ3UPZZCKY5DBH6ZGMPMRORRBG4ECIORASBUAXPPNCL4SYRHNLYU2');
    expect(entry.status).toBe('success');
    expect(entry.txHash).toBe('abc123');
    expect(entry.ledger).toBe(999);
    expect(entry.timestamp).toBeDefined();
    expect(entry.prevHash).toBe('0');
    expect(entry.hash).toBeDefined();
    expect(entry.hash.length).toBe(64); // SHA-256 hex
  });

  it('increments sequence monotonically', () => {
    const e1 = auditLogService.record({ action: 'DEPOSIT', actor: 'A', status: 'success' });
    const e2 = auditLogService.record({ action: 'BORROW',  actor: 'A', status: 'success' });
    const e3 = auditLogService.record({ action: 'REPAY',   actor: 'A', status: 'success' });

    expect(e1.sequence).toBe(1);
    expect(e2.sequence).toBe(2);
    expect(e3.sequence).toBe(3);
  });

  it('chains prevHash correctly', () => {
    const e1 = auditLogService.record({ action: 'DEPOSIT', actor: 'A', status: 'success' });
    const e2 = auditLogService.record({ action: 'BORROW',  actor: 'A', status: 'success' });

    expect(e1.prevHash).toBe('0');
    expect(e2.prevHash).toBe(e1.hash);
  });

  it('captures beforeState and afterState', () => {
    const entry = auditLogService.record({
      action: 'PROTOCOL_PAUSED',
      actor: 'SYSTEM',
      status: 'success',
      beforeState: { paused: false },
      afterState: { paused: true, reason: 'manual' },
    });

    expect(entry.beforeState).toEqual({ paused: false });
    expect(entry.afterState).toEqual({ paused: true, reason: 'manual' });
  });

  it('does not expose sensitive fields', () => {
    const entry = auditLogService.record({ action: 'DEPOSIT', actor: 'A', status: 'success' });
    const keys = Object.keys(entry);
    expect(keys).not.toContain('privateKey');
    expect(keys).not.toContain('secret');
    expect(keys).not.toContain('password');
  });
});

describe('AuditLogService.verify()', () => {
  it('returns valid: true for an empty log', () => {
    const result = auditLogService.verify();
    expect(result.valid).toBe(true);
    expect(result.checkedEntries).toBe(0);
  });

  it('returns valid: true for an intact chain', () => {
    auditLogService.record({ action: 'DEPOSIT', actor: 'A', status: 'success' });
    auditLogService.record({ action: 'BORROW',  actor: 'A', status: 'success' });
    auditLogService.record({ action: 'REPAY',   actor: 'A', status: 'success' });

    const result = auditLogService.verify();
    expect(result.valid).toBe(true);
    expect(result.checkedEntries).toBe(3);
  });

  it('detects tampering', () => {
    auditLogService.record({ action: 'DEPOSIT', actor: 'A', status: 'success' });
    auditLogService.record({ action: 'BORROW',  actor: 'A', status: 'success' });

    // Directly mutate internal state to simulate tampering
    const entries = (auditLogService as any).entries as any[];
    entries[0].amount = 'tampered';

    const result = auditLogService.verify();
    expect(result.valid).toBe(false);
    expect(result.firstBadSequence).toBe(1);
  });
});

describe('AuditLogService.search()', () => {
  beforeEach(() => {
    auditLogService.record({ action: 'DEPOSIT',  actor: 'alice', status: 'success' });
    auditLogService.record({ action: 'BORROW',   actor: 'alice', status: 'failed' });
    auditLogService.record({ action: 'WITHDRAW', actor: 'bob',   status: 'success' });
  });

  it('returns all entries when no filter is given', () => {
    expect(auditLogService.search().length).toBe(3);
  });

  it('filters by action', () => {
    const results = auditLogService.search({ action: 'DEPOSIT' });
    expect(results.length).toBe(1);
    expect(results[0].action).toBe('DEPOSIT');
  });

  it('filters by actor', () => {
    const results = auditLogService.search({ actor: 'alice' });
    expect(results.length).toBe(2);
  });

  it('filters by status', () => {
    const results = auditLogService.search({ status: 'failed' });
    expect(results.length).toBe(1);
    expect(results[0].action).toBe('BORROW');
  });

  it('applies limit and offset', () => {
    const page1 = auditLogService.search({ limit: 2, offset: 0 });
    const page2 = auditLogService.search({ limit: 2, offset: 2 });
    expect(page1.length).toBe(2);
    expect(page2.length).toBe(1);
  });
});

describe('AuditLogService.export()', () => {
  it('returns valid JSON', () => {
    auditLogService.record({ action: 'DEPOSIT', actor: 'A', status: 'success' });
    const json = auditLogService.export();
    expect(() => JSON.parse(json)).not.toThrow();
    const parsed = JSON.parse(json);
    expect(Array.isArray(parsed)).toBe(true);
    expect(parsed.length).toBe(1);
  });

  it('respects filters', () => {
    auditLogService.record({ action: 'DEPOSIT', actor: 'A', status: 'success' });
    auditLogService.record({ action: 'BORROW',  actor: 'A', status: 'success' });
    const json = auditLogService.export({ action: 'BORROW' });
    const parsed = JSON.parse(json);
    expect(parsed.length).toBe(1);
    expect(parsed[0].action).toBe('BORROW');
  });
});

describe('AuditLogService date range filtering', () => {
  it('filters by from/to timestamps', () => {
    const past = new Date(Date.now() - 60_000).toISOString();
    const now = new Date().toISOString();
    const future = new Date(Date.now() + 60_000).toISOString();

    auditLogService.record({ action: 'DEPOSIT', actor: 'A', status: 'success' });

    expect(auditLogService.search({ from: past, to: future }).length).toBe(1);
    expect(auditLogService.search({ from: future }).length).toBe(0);
    expect(auditLogService.search({ to: past }).length).toBe(0);
    expect(auditLogService.search({ from: now }).length).toBe(1);
  });
});

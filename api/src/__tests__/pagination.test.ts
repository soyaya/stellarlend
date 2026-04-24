import { encodeCursor, decodeCursor, parsePaginationParams, buildPaginationMeta } from '../utils/pagination';
import { ValidationError } from '../utils/errors';

describe('encodeCursor / decodeCursor', () => {
  it('round-trips arbitrary cursor strings', () => {
    const raw = 'horizon-cursor-12345';
    expect(decodeCursor(encodeCursor(raw))).toBe(raw);
  });

  it('produces a base64url string without padding characters', () => {
    const encoded = encodeCursor('abc');
    expect(encoded).not.toMatch(/[+/=]/);
  });

  it('returns null for an empty base64url decoded value', () => {
    expect(decodeCursor('')).toBeNull();
  });

  it('returns null for a non-base64url input', () => {
    expect(decodeCursor('!!! invalid !!!')).toBeNull();
    expect(decodeCursor('has spaces')).toBeNull();
    expect(decodeCursor('has+plus/slash=')).toBeNull();
  });

  it('handles cursors with special characters', () => {
    const raw = 'cursor with spaces & special=chars?foo=bar';
    expect(decodeCursor(encodeCursor(raw))).toBe(raw);
  });
});

describe('parsePaginationParams', () => {
  it('returns default limit when none provided', () => {
    const result = parsePaginationParams({});
    expect(typeof result.limit).toBe('number');
    expect(result.limit).toBeGreaterThan(0);
  });

  it('parses a valid limit', () => {
    const result = parsePaginationParams({ limit: '25' });
    expect(result.limit).toBe(25);
  });

  it('throws ValidationError for a non-integer limit', () => {
    expect(() => parsePaginationParams({ limit: 'abc' })).toThrow(ValidationError);
  });

  it('throws ValidationError for a zero limit', () => {
    expect(() => parsePaginationParams({ limit: '0' })).toThrow(ValidationError);
  });

  it('throws ValidationError for a negative limit', () => {
    expect(() => parsePaginationParams({ limit: '-5' })).toThrow(ValidationError);
  });

  it('returns cursor: null when no cursor provided', () => {
    const result = parsePaginationParams({});
    expect(result.cursor).toBeNull();
  });

  it('decodes a valid opaque cursor', () => {
    const raw = 'horizon-paging-token-99';
    const encoded = encodeCursor(raw);
    const result = parsePaginationParams({ cursor: encoded });
    expect(result.cursor).toBe(raw);
  });

  it('throws ValidationError for an empty cursor', () => {
    expect(() => parsePaginationParams({ cursor: '' })).toThrow(ValidationError);
    expect(() => parsePaginationParams({ cursor: '   ' })).toThrow(ValidationError);
  });

  it('throws ValidationError for a malformed cursor', () => {
    expect(() => parsePaginationParams({ cursor: 'this-is-not-base64url!!!' })).toThrow(ValidationError);
  });
});

describe('buildPaginationMeta', () => {
  it('encodes the raw next cursor as an opaque string', () => {
    const raw = 'next-paging-token';
    const meta = buildPaginationMeta(raw, true, 10);
    expect(meta.cursor).toBe(encodeCursor(raw));
    expect(meta.cursor).not.toBe(raw);
  });

  it('sets cursor to null when there is no next page', () => {
    const meta = buildPaginationMeta(null, false, 10);
    expect(meta.cursor).toBeNull();
    expect(meta.hasMore).toBe(false);
  });

  it('includes total when provided', () => {
    const meta = buildPaginationMeta(null, false, 10, 42);
    expect(meta.total).toBe(42);
  });

  it('defaults total to null when not provided', () => {
    const meta = buildPaginationMeta(null, false, 10);
    expect(meta.total).toBeNull();
  });

  it('preserves the limit in the response', () => {
    const meta = buildPaginationMeta(null, false, 25);
    expect(meta.limit).toBe(25);
  });
});

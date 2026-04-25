import { config } from '../config';
import { ValidationError } from './errors';
import { PaginationParams, PaginationMeta } from '../types/pagination';

function isPositiveInteger(value: number): boolean {
  return Number.isInteger(value) && value > 0;
}

/**
 * Encode a raw upstream cursor as an opaque base64url string so callers
 * cannot depend on its internal format.
 */
export function encodeCursor(raw: string): string {
  return Buffer.from(raw, 'utf8').toString('base64url');
}

const BASE64URL_RE = /^[A-Za-z0-9_-]+$/;

/**
 * Decode a client-supplied opaque cursor back to the raw upstream value.
 * Returns null when the input is not valid base64url or decoding is empty.
 */
export function decodeCursor(opaque: string): string | null {
  if (!opaque || !BASE64URL_RE.test(opaque)) return null;
  try {
    const decoded = Buffer.from(opaque, 'base64url').toString('utf8');
    return decoded.length > 0 ? decoded : null;
  } catch {
    return null;
  }
}

export function parsePaginationParams(rawQuery: Record<string, any> = {}): PaginationParams {
  const limitFromQuery = rawQuery.limit;
  const cursorFromQuery = rawQuery.cursor;

  const defaultLimit = Number.isFinite(config.pagination.defaultLimit)
    ? config.pagination.defaultLimit
    : 10;
  const maxLimit = Number.isFinite(config.pagination.maxLimit)
    ? config.pagination.maxLimit
    : 100;

  let limit = defaultLimit;
  if (limitFromQuery !== undefined) {
    const parsedLimit = Number(limitFromQuery);
    if (!Number.isFinite(parsedLimit) || !isPositiveInteger(parsedLimit)) {
      throw new ValidationError('limit must be a positive integer');
    }
    if (parsedLimit > maxLimit) {
      throw new ValidationError(`limit must be <= ${maxLimit}`);
    }
    limit = parsedLimit;
  }

  let cursor: string | null = null;
  if (cursorFromQuery !== undefined && cursorFromQuery !== null) {
    const normalizedCursor = String(cursorFromQuery).trim();
    if (normalizedCursor.length === 0) {
      throw new ValidationError('cursor cannot be empty');
    }
    const decoded = decodeCursor(normalizedCursor);
    if (decoded === null) {
      throw new ValidationError('cursor is invalid or expired');
    }
    cursor = decoded;
  }

  return {
    limit,
    cursor,
  };
}

export function buildPaginationMeta(
  rawNextCursor: string | null,
  hasMore: boolean,
  limit: number,
  total: number | null = null
): PaginationMeta {
  return {
    cursor: rawNextCursor !== null ? encodeCursor(rawNextCursor) : null,
    hasMore,
    limit,
    total,
  };
}

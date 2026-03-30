import { config } from '../config';
import { ValidationError } from './errors';
import { PaginationParams, PaginationMeta } from '../types/pagination';

function isPositiveInteger(value: number): boolean {
  return Number.isInteger(value) && value > 0;
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
    cursor = normalizedCursor;
  }

  return {
    limit,
    cursor,
  };
}

export function buildPaginationMeta(
  cursor: string | null,
  hasMore: boolean,
  limit: number
): PaginationMeta {
  return {
    cursor,
    hasMore,
    limit,
  };
}

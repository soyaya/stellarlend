export interface PaginationParams {
  limit?: number;
  cursor?: string | null;
}

export interface PaginationMeta {
  /** Opaque cursor for the next page. Null if no further pages exist. */
  cursor: string | null;
  hasMore: boolean;
  limit: number;
  /** Total item count, included only on the first page (no cursor). Null when unavailable from upstream. */
  total: number | null;
}

export interface PaginatedResponse<T> {
  data: T[];
  pagination: PaginationMeta;
}

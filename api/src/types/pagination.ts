export interface PaginationParams {
  limit?: number;
  cursor?: string | null;
}

export interface PaginationMeta {
  cursor: string | null;
  hasMore: boolean;
  limit: number;
}

export interface PaginatedResponse<T> {
  data: T[];
  pagination: PaginationMeta;
}

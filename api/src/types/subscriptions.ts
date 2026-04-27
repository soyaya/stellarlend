export const SUBSCRIPTION_STATUSES = [
  'active',
  'paused',
  'cancelled',
  'trialing',
  'past_due',
] as const;

export const SUBSCRIPTION_INTERVALS = [
  'daily',
  'weekly',
  'monthly',
  'quarterly',
  'yearly',
] as const;

export type SubscriptionStatus = (typeof SUBSCRIPTION_STATUSES)[number];
export type SubscriptionInterval = (typeof SUBSCRIPTION_INTERVALS)[number];
export type ImportFormat = 'csv' | 'json';
export type ImportAction = 'create' | 'update' | 'skip';

export interface SubscriptionRecord {
  merchantId: string;
  subscriptionId: string;
  customerEmail: string;
  planId: string;
  status: SubscriptionStatus;
  amount: number;
  currency: string;
  interval: SubscriptionInterval;
  startDate: string;
  nextBillingDate?: string;
  metadata?: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
}

export interface ImportOptions {
  upsert?: boolean;
  previewLimit?: number;
}

export interface ImportData {
  merchantId: string;
  format: ImportFormat;
  data: string | unknown[];
  columnMapping?: Record<string, string>;
  options?: ImportOptions;
}

export interface ImportRowError {
  rowNumber: number;
  field: string;
  message: string;
}

export interface ImportRowWarning {
  rowNumber: number;
  field: string;
  message: string;
}

export interface ImportPreviewRow {
  rowNumber: number;
  action: ImportAction;
  subscription: Omit<SubscriptionRecord, 'createdAt' | 'updatedAt'>;
}

export interface ValidationSummary {
  totalRows: number;
  validRows: number;
  invalidRows: number;
  creates: number;
  updates: number;
  skips: number;
}

export interface ValidationResult {
  merchantId: string;
  format: ImportFormat;
  isValid: boolean;
  summary: ValidationSummary;
  errors: ImportRowError[];
  warnings: ImportRowWarning[];
  previewRows: ImportPreviewRow[];
  normalizedRows: ImportPreviewRow[];
}

export interface ImportHistoryEntry {
  importId: string;
  merchantId: string;
  format: ImportFormat;
  createdAt: string;
  totalRows: number;
  importedCount: number;
  updatedCount: number;
  skippedCount: number;
  errorCount: number;
  status: 'completed' | 'failed';
}

export interface ImportResult {
  merchantId: string;
  importId: string;
  importedCount: number;
  updatedCount: number;
  skippedCount: number;
  errorCount: number;
  errors: ImportRowError[];
  warnings: ImportRowWarning[];
  historyEntry: ImportHistoryEntry;
}

export interface ExportData {
  merchantId: string;
  exportedAt: string;
  format: 'json';
  count: number;
  subscriptions: SubscriptionRecord[];
}

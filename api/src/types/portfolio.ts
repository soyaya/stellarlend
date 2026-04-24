export interface PortfolioPosition {
  assetAddress?: string;
  collateral: string;
  debt: string;
  borrowInterest: string;
  /** collateral minus debt (may be negative if underwater) */
  netValue: string;
  collateralRatio: string;
  lastAccrualTime: number;
}

export interface PortfolioValue {
  totalCollateral: string;
  totalDebt: string;
  netValue: string;
  /** debt / collateral expressed as a decimal string (e.g. "0.50") */
  utilizationRate: string;
  positions: PortfolioPosition[];
  snapshotTimestamp: string;
}

export type RiskLevel = 'low' | 'moderate' | 'high' | 'critical';

export interface RiskMetrics {
  /** collateral / (debt * liquidationThreshold). Below 1.0 = liquidatable. */
  healthFactor: string;
  liquidationThreshold: string;
  /** Percentage buffer remaining before the health factor hits 1.0. */
  liquidationDistancePct: string;
  /** Estimated probability (0–100) of liquidation within the next 24 h. */
  liquidationProbabilityPct: number;
  /** 1-day parametric VaR at 95 % confidence (in stroops). */
  valueAtRisk95: string;
  /** 1-day parametric VaR at 99 % confidence (in stroops). */
  valueAtRisk99: string;
  /** Estimated maximum drawdown percentage based on leverage. */
  estimatedMaxDrawdownPct: number;
  riskLevel: RiskLevel;
}

export type SuggestionType =
  | 'add_collateral'
  | 'reduce_debt'
  | 'borrow_more'
  | 'withdraw_collateral'
  | 'rebalance'
  | 'maintain';

export type SuggestionPriority = 'urgent' | 'recommended' | 'optional';

export interface OptimizationSuggestion {
  type: SuggestionType;
  priority: SuggestionPriority;
  description: string;
  estimatedImpact?: string;
}

export interface PerformanceSummary {
  totalDeposited: string;
  totalWithdrawn: string;
  totalBorrowed: string;
  totalRepaid: string;
  /** (totalDeposited + totalRepaid) − (totalWithdrawn + totalBorrowed) */
  netFlow: string;
  transactionCount: number;
  operationBreakdown: Record<string, number>;
  firstTransactionAt?: string;
  lastTransactionAt?: string;
}

export interface PortfolioAnalyticsResponse {
  userAddress: string;
  portfolioValue: PortfolioValue;
  riskMetrics: RiskMetrics;
  suggestions: OptimizationSuggestion[];
  performance: PerformanceSummary;
  generatedAt: string;
}

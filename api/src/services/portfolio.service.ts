import { PositionResponse, TransactionHistoryItem } from '../types';
import {
  PortfolioAnalyticsResponse,
  PortfolioPosition,
  PortfolioValue,
  RiskMetrics,
  RiskLevel,
  OptimizationSuggestion,
  PerformanceSummary,
} from '../types/portfolio';

// ─── Constants ────────────────────────────────────────────────────────────────

/** Minimum collateral-to-debt ratio before liquidation (120 %). */
const LIQUIDATION_THRESHOLD = 1.2;

/**
 * Assumed annualised asset volatility used for VaR and drawdown estimates.
 * Conservative 40 % is typical for large-cap crypto collateral.
 */
const ANNUAL_VOLATILITY = 0.40;
const DAILY_VOLATILITY = ANNUAL_VOLATILITY / Math.sqrt(252);

const Z_95 = 1.6449; // 95 % one-tailed z-score
const Z_99 = 2.3263; // 99 % one-tailed z-score

const STROOP_SCALE = 10_000_000n; // 1 XLM = 10,000,000 stroops

// ─── BigInt helpers ────────────────────────────────────────────────────────────

function safeBigInt(value: string): bigint {
  try {
    const cleaned = value.trim();
    return cleaned === '' || cleaned === 'Infinity' ? 0n : BigInt(cleaned);
  } catch {
    return 0n;
  }
}

/** Scale bigint by a float factor; returns a bigint (truncated). */
function scaleByFloat(amount: bigint, factor: number): bigint {
  const scaled = Number(amount) * factor;
  return BigInt(Math.trunc(scaled));
}

/** Format a bigint ratio with 4 decimal places as a string. */
function formatRatio(numerator: bigint, denominator: bigint): string {
  if (denominator === 0n) return 'Infinity';
  const whole = numerator / denominator;
  const remainder = ((numerator % denominator) * 10000n) / denominator;
  return `${whole}.${remainder.toString().padStart(4, '0')}`;
}

// ─── Portfolio value ───────────────────────────────────────────────────────────

function buildPortfolioValue(position: PositionResponse): PortfolioValue {
  const collateral = safeBigInt(position.collateral);
  const debt = safeBigInt(position.debt);
  const interest = safeBigInt(position.borrowInterest);
  const totalDebt = debt + interest;
  const netValue = collateral - totalDebt;

  const portfolioPosition: PortfolioPosition = {
    assetAddress: undefined,
    collateral: collateral.toString(),
    debt: totalDebt.toString(),
    borrowInterest: interest.toString(),
    netValue: netValue.toString(),
    collateralRatio: totalDebt > 0n ? formatRatio(collateral, totalDebt) : 'Infinity',
    lastAccrualTime: position.lastAccrualTime,
  };

  return {
    totalCollateral: collateral.toString(),
    totalDebt: totalDebt.toString(),
    netValue: netValue.toString(),
    utilizationRate: collateral > 0n ? formatRatio(totalDebt, collateral) : '0.0000',
    positions: [portfolioPosition],
    snapshotTimestamp: new Date().toISOString(),
  };
}

// ─── Risk metrics ─────────────────────────────────────────────────────────────

function computeHealthFactor(collateral: bigint, totalDebt: bigint): number {
  if (totalDebt === 0n) return Infinity;
  return Number(collateral) / (Number(totalDebt) * LIQUIDATION_THRESHOLD);
}

function liquidationProbability(healthFactor: number): number {
  if (!isFinite(healthFactor)) return 0;
  if (healthFactor < 1.0) return 95;
  if (healthFactor < 1.2) return 60;
  if (healthFactor < 1.5) return 25;
  if (healthFactor < 2.0) return 10;
  if (healthFactor < 3.0) return 3;
  return 1;
}

function riskLevel(healthFactor: number): RiskLevel {
  if (!isFinite(healthFactor)) return 'low';
  if (healthFactor < 1.0) return 'critical';
  if (healthFactor < 1.5) return 'high';
  if (healthFactor < 2.0) return 'moderate';
  return 'low';
}

function buildRiskMetrics(collateral: bigint, totalDebt: bigint): RiskMetrics {
  const hf = computeHealthFactor(collateral, totalDebt);
  const hfDisplay = isFinite(hf) ? hf.toFixed(4) : 'Infinity';

  // Distance = how far HF is above 1.0, as a percentage of current HF
  const distancePct = isFinite(hf) ? Math.max(0, ((hf - 1.0) / hf) * 100) : 100;

  // VaR applied to net debt exposure (the amount at risk if prices move)
  const exposure = Number(totalDebt);
  const var95 = Math.trunc(exposure * DAILY_VOLATILITY * Z_95);
  const var99 = Math.trunc(exposure * DAILY_VOLATILITY * Z_99);

  // Estimated max drawdown: leverage = debt / collateral; drawdown ≈ leverage * annual_vol
  const leverage = Number(collateral) > 0 ? Number(totalDebt) / Number(collateral) : 0;
  const maxDrawdownPct = Math.min(100, leverage * ANNUAL_VOLATILITY * 100);

  return {
    healthFactor: hfDisplay,
    liquidationThreshold: LIQUIDATION_THRESHOLD.toFixed(2),
    liquidationDistancePct: distancePct.toFixed(2),
    liquidationProbabilityPct: liquidationProbability(hf),
    valueAtRisk95: var95.toString(),
    valueAtRisk99: var99.toString(),
    estimatedMaxDrawdownPct: parseFloat(maxDrawdownPct.toFixed(2)),
    riskLevel: riskLevel(hf),
  };
}

// ─── Optimization suggestions ─────────────────────────────────────────────────

function buildSuggestions(
  healthFactor: number,
  utilizationRate: number,
  collateral: bigint,
  totalDebt: bigint
): OptimizationSuggestion[] {
  const suggestions: OptimizationSuggestion[] = [];

  if (!isFinite(healthFactor) || collateral === 0n) {
    suggestions.push({
      type: 'maintain',
      priority: 'optional',
      description: 'No active position. Deposit collateral to begin earning and borrowing.',
    });
    return suggestions;
  }

  if (healthFactor < 1.0) {
    suggestions.push({
      type: 'add_collateral',
      priority: 'urgent',
      description:
        'Position is under the liquidation threshold. Add collateral immediately to avoid liquidation.',
      estimatedImpact: 'Prevents forced liquidation and associated penalty fees.',
    });
  } else if (healthFactor < 1.5) {
    suggestions.push({
      type: 'add_collateral',
      priority: 'urgent',
      description:
        'Health factor is dangerously low. Add collateral or repay debt to increase your safety buffer.',
      estimatedImpact: `Raising health factor to 2.0 would reduce liquidation risk from ${liquidationProbability(healthFactor)}% to ~10%.`,
    });
  } else if (healthFactor < 2.0) {
    suggestions.push({
      type: 'reduce_debt',
      priority: 'recommended',
      description:
        'Consider partially repaying your debt to improve your health factor and reduce risk.',
      estimatedImpact: 'Each 10% debt reduction improves the health factor proportionally.',
    });
  }

  if (utilizationRate < 0.3 && collateral > 0n) {
    suggestions.push({
      type: 'borrow_more',
      priority: 'optional',
      description:
        'Your collateral utilization is low. You can safely borrow more against your collateral.',
      estimatedImpact: `Current utilization: ${(utilizationRate * 100).toFixed(1)}%. Borrowing up to 50% utilization remains low-risk.`,
    });
  }

  if (utilizationRate > 0.7 && healthFactor >= 2.0) {
    suggestions.push({
      type: 'rebalance',
      priority: 'recommended',
      description:
        'High utilization detected. Consider adding more collateral to maintain a comfortable buffer.',
    });
  }

  if (healthFactor >= 2.0 && utilizationRate >= 0.3 && utilizationRate <= 0.7) {
    suggestions.push({
      type: 'maintain',
      priority: 'optional',
      description: 'Portfolio is well-balanced. Health factor and utilization are in the optimal range.',
    });
  }

  return suggestions;
}

// ─── Historical performance ────────────────────────────────────────────────────

function buildPerformanceSummary(history: TransactionHistoryItem[]): PerformanceSummary {
  let totalDeposited = 0n;
  let totalWithdrawn = 0n;
  let totalBorrowed = 0n;
  let totalRepaid = 0n;
  const breakdown: Record<string, number> = {};

  for (const tx of history) {
    if (tx.status !== 'success') continue;
    const amount = safeBigInt(tx.amount);
    breakdown[tx.type] = (breakdown[tx.type] ?? 0) + 1;

    switch (tx.type) {
      case 'deposit':
        totalDeposited += amount;
        break;
      case 'withdraw':
        totalWithdrawn += amount;
        break;
      case 'borrow':
        totalBorrowed += amount;
        break;
      case 'repay':
        totalRepaid += amount;
        break;
    }
  }

  const netFlow = totalDeposited + totalRepaid - totalWithdrawn - totalBorrowed;

  const sorted = [...history].sort(
    (a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime()
  );

  return {
    totalDeposited: totalDeposited.toString(),
    totalWithdrawn: totalWithdrawn.toString(),
    totalBorrowed: totalBorrowed.toString(),
    totalRepaid: totalRepaid.toString(),
    netFlow: netFlow.toString(),
    transactionCount: history.length,
    operationBreakdown: breakdown,
    firstTransactionAt: sorted[0]?.timestamp,
    lastTransactionAt: sorted[sorted.length - 1]?.timestamp,
  };
}

// ─── CSV export ───────────────────────────────────────────────────────────────

export function toCSV(history: TransactionHistoryItem[]): string {
  const header = 'date,type,amount,assetAddress,txHash,ledger,status';
  const rows = history.map((tx) =>
    [
      tx.timestamp,
      tx.type,
      tx.amount,
      tx.assetAddress ?? '',
      tx.transactionHash,
      tx.ledger ?? '',
      tx.status,
    ].join(',')
  );
  return [header, ...rows].join('\n');
}

// ─── Main analytics entry point ────────────────────────────────────────────────

export function analyzePortfolio(
  userAddress: string,
  position: PositionResponse,
  history: TransactionHistoryItem[]
): PortfolioAnalyticsResponse {
  const portfolioValue = buildPortfolioValue(position);

  const collateral = safeBigInt(position.collateral);
  const totalDebt =
    safeBigInt(position.debt) + safeBigInt(position.borrowInterest);

  const riskMetrics = buildRiskMetrics(collateral, totalDebt);
  const hf = computeHealthFactor(collateral, totalDebt);
  const utilizationRate =
    collateral > 0n ? Number(totalDebt) / Number(collateral) : 0;

  const suggestions = buildSuggestions(hf, utilizationRate, collateral, totalDebt);
  const performance = buildPerformanceSummary(history);

  return {
    userAddress,
    portfolioValue,
    riskMetrics,
    suggestions,
    performance,
    generatedAt: new Date().toISOString(),
  };
}

import { analyzePortfolio, toCSV } from '../services/portfolio.service';
import { PositionResponse, TransactionHistoryItem } from '../types';

// ─── Fixtures ─────────────────────────────────────────────────────────────────

const ADDR = 'GDZZJ3UPZZCKY5DBH6ZGMPMRORRBG4ECIORASBUAXPPNCL4SYRHNLYU2';

function makePosition(overrides: Partial<PositionResponse> = {}): PositionResponse {
  return {
    userAddress: ADDR,
    collateral: '10000000', // 1 XLM
    debt: '5000000', // 0.5 XLM
    borrowInterest: '100000',
    lastAccrualTime: 1700000000,
    collateralRatio: '1.9608',
    ...overrides,
  };
}

function makeTx(overrides: Partial<TransactionHistoryItem> = {}): TransactionHistoryItem {
  return {
    transactionHash: 'tx_abc',
    type: 'deposit',
    amount: '1000000',
    timestamp: '2024-01-01T00:00:00Z',
    status: 'success',
    ledger: 100,
    ...overrides,
  };
}

// ─── Portfolio value ───────────────────────────────────────────────────────────

describe('analyzePortfolio – portfolioValue', () => {
  it('calculates netValue as collateral minus total debt', () => {
    const pos = makePosition({ collateral: '10000000', debt: '4000000', borrowInterest: '500000' });
    const { portfolioValue } = analyzePortfolio(ADDR, pos, []);
    // net = 10000000 - (4000000 + 500000) = 5500000
    expect(portfolioValue.netValue).toBe('5500000');
    expect(portfolioValue.totalCollateral).toBe('10000000');
    expect(portfolioValue.totalDebt).toBe('4500000');
  });

  it('sets utilizationRate to 0 when collateral is zero', () => {
    const pos = makePosition({ collateral: '0', debt: '0', borrowInterest: '0' });
    const { portfolioValue } = analyzePortfolio(ADDR, pos, []);
    expect(portfolioValue.utilizationRate).toBe('0.0000');
  });

  it('correctly calculates utilization rate', () => {
    const pos = makePosition({ collateral: '10000000', debt: '5000000', borrowInterest: '0' });
    const { portfolioValue } = analyzePortfolio(ADDR, pos, []);
    // utilization = 5000000 / 10000000 = 0.5
    expect(portfolioValue.utilizationRate).toBe('0.5000');
  });

  it('populates exactly one position entry', () => {
    const { portfolioValue } = analyzePortfolio(ADDR, makePosition(), []);
    expect(portfolioValue.positions).toHaveLength(1);
  });

  it('returns a valid ISO timestamp', () => {
    const { portfolioValue } = analyzePortfolio(ADDR, makePosition(), []);
    expect(() => new Date(portfolioValue.snapshotTimestamp)).not.toThrow();
  });
});

// ─── Risk metrics ─────────────────────────────────────────────────────────────

describe('analyzePortfolio – riskMetrics', () => {
  it('returns Infinity health factor when there is no debt', () => {
    const pos = makePosition({ debt: '0', borrowInterest: '0' });
    const { riskMetrics } = analyzePortfolio(ADDR, pos, []);
    expect(riskMetrics.healthFactor).toBe('Infinity');
    expect(riskMetrics.riskLevel).toBe('low');
    expect(riskMetrics.liquidationProbabilityPct).toBe(0);
  });

  it('classifies as critical when health factor is below 1.0', () => {
    // collateral 1, debt 2 → HF = 1 / (2 * 1.2) = 0.416
    const pos = makePosition({ collateral: '1000000', debt: '2000000', borrowInterest: '0' });
    const { riskMetrics } = analyzePortfolio(ADDR, pos, []);
    expect(riskMetrics.riskLevel).toBe('critical');
    expect(riskMetrics.liquidationProbabilityPct).toBeGreaterThanOrEqual(60);
  });

  it('classifies as high when health factor is between 1.0 and 1.5', () => {
    // collateral 10, debt 7 → HF = 10 / (7 * 1.2) = 1.19
    const pos = makePosition({ collateral: '10000000', debt: '7000000', borrowInterest: '0' });
    const { riskMetrics } = analyzePortfolio(ADDR, pos, []);
    expect(riskMetrics.riskLevel).toBe('high');
  });

  it('classifies as moderate when health factor is between 1.5 and 2.0', () => {
    // collateral 10, debt 5 → HF = 10 / (5 * 1.2) = 1.666...
    const pos = makePosition({ collateral: '10000000', debt: '5000000', borrowInterest: '0' });
    const { riskMetrics } = analyzePortfolio(ADDR, pos, []);
    expect(riskMetrics.riskLevel).toBe('moderate');
  });

  it('classifies as low when health factor is >= 2.0', () => {
    // collateral 10, debt 2 → HF = 10 / (2 * 1.2) = 4.16
    const pos = makePosition({ collateral: '10000000', debt: '2000000', borrowInterest: '0' });
    const { riskMetrics } = analyzePortfolio(ADDR, pos, []);
    expect(riskMetrics.riskLevel).toBe('low');
    expect(riskMetrics.liquidationProbabilityPct).toBeLessThanOrEqual(10);
  });

  it('VaR99 is always greater than VaR95 when there is debt', () => {
    const pos = makePosition({ collateral: '10000000', debt: '5000000', borrowInterest: '0' });
    const { riskMetrics } = analyzePortfolio(ADDR, pos, []);
    expect(BigInt(riskMetrics.valueAtRisk99)).toBeGreaterThan(BigInt(riskMetrics.valueAtRisk95));
  });

  it('VaR values are zero when there is no debt', () => {
    const pos = makePosition({ debt: '0', borrowInterest: '0' });
    const { riskMetrics } = analyzePortfolio(ADDR, pos, []);
    expect(riskMetrics.valueAtRisk95).toBe('0');
    expect(riskMetrics.valueAtRisk99).toBe('0');
  });

  it('liquidationDistancePct is 0 when position is at liquidation threshold', () => {
    // HF = 1.0 exactly: collateral = debt * 1.2
    const pos = makePosition({ collateral: '12000000', debt: '10000000', borrowInterest: '0' });
    const { riskMetrics } = analyzePortfolio(ADDR, pos, []);
    const dist = parseFloat(riskMetrics.liquidationDistancePct);
    expect(dist).toBeCloseTo(0, 0);
  });

  it('maxDrawdownPct is capped at 100', () => {
    // Very high debt relative to collateral
    const pos = makePosition({ collateral: '1000000', debt: '10000000', borrowInterest: '0' });
    const { riskMetrics } = analyzePortfolio(ADDR, pos, []);
    expect(riskMetrics.estimatedMaxDrawdownPct).toBeLessThanOrEqual(100);
  });
});

// ─── Optimization suggestions ─────────────────────────────────────────────────

describe('analyzePortfolio – suggestions', () => {
  it('suggests add_collateral urgently when HF < 1.5', () => {
    const pos = makePosition({ collateral: '5000000', debt: '4000000', borrowInterest: '0' });
    const { suggestions } = analyzePortfolio(ADDR, pos, []);
    const urgent = suggestions.find((s) => s.priority === 'urgent');
    expect(urgent).toBeDefined();
    expect(['add_collateral', 'reduce_debt']).toContain(urgent!.type);
  });

  it('suggests borrow_more when utilization is low', () => {
    // 5% utilization — well below 30% threshold
    const pos = makePosition({ collateral: '100000000', debt: '1000000', borrowInterest: '0' });
    const { suggestions } = analyzePortfolio(ADDR, pos, []);
    expect(suggestions.some((s) => s.type === 'borrow_more')).toBe(true);
  });

  it('suggests maintain when portfolio is well-balanced', () => {
    // HF = 10 / (5 * 1.2) = 1.666 (moderate); util = 0.5 (in range)
    const pos = makePosition({ collateral: '10000000', debt: '5000000', borrowInterest: '0' });
    const { suggestions } = analyzePortfolio(ADDR, pos, []);
    // Should not be urgent
    expect(suggestions.every((s) => s.priority !== 'urgent')).toBe(true);
  });

  it('returns a maintain suggestion for empty positions', () => {
    const pos = makePosition({ collateral: '0', debt: '0', borrowInterest: '0' });
    const { suggestions } = analyzePortfolio(ADDR, pos, []);
    expect(suggestions.some((s) => s.type === 'maintain')).toBe(true);
  });

  it('all suggestions have a non-empty description', () => {
    const pos = makePosition();
    const { suggestions } = analyzePortfolio(ADDR, pos, []);
    for (const s of suggestions) {
      expect(s.description.length).toBeGreaterThan(0);
    }
  });
});

// ─── Performance summary ──────────────────────────────────────────────────────

describe('analyzePortfolio – performance', () => {
  const txs: TransactionHistoryItem[] = [
    makeTx({
      type: 'deposit',
      amount: '5000000',
      status: 'success',
      timestamp: '2024-01-01T00:00:00Z',
    }),
    makeTx({
      type: 'borrow',
      amount: '2000000',
      status: 'success',
      timestamp: '2024-01-02T00:00:00Z',
      transactionHash: 'tx2',
    }),
    makeTx({
      type: 'repay',
      amount: '1000000',
      status: 'success',
      timestamp: '2024-01-03T00:00:00Z',
      transactionHash: 'tx3',
    }),
    makeTx({
      type: 'withdraw',
      amount: '500000',
      status: 'success',
      timestamp: '2024-01-04T00:00:00Z',
      transactionHash: 'tx4',
    }),
    makeTx({
      type: 'deposit',
      amount: '999999',
      status: 'failed',
      timestamp: '2024-01-05T00:00:00Z',
      transactionHash: 'tx5',
    }),
  ];

  it('sums amounts by operation type (success only)', () => {
    const { performance } = analyzePortfolio(ADDR, makePosition(), txs);
    expect(performance.totalDeposited).toBe('5000000');
    expect(performance.totalBorrowed).toBe('2000000');
    expect(performance.totalRepaid).toBe('1000000');
    expect(performance.totalWithdrawn).toBe('500000');
  });

  it('computes netFlow as (deposited + repaid) - (withdrawn + borrowed)', () => {
    const { performance } = analyzePortfolio(ADDR, makePosition(), txs);
    // (5000000 + 1000000) - (500000 + 2000000) = 3500000
    expect(performance.netFlow).toBe('3500000');
  });

  it('ignores failed transactions', () => {
    const { performance } = analyzePortfolio(ADDR, makePosition(), txs);
    expect(performance.transactionCount).toBe(5); // total count, including failed
    expect(performance.totalDeposited).toBe('5000000'); // failed deposit not included
  });

  it('tracks operation breakdown counts', () => {
    const { performance } = analyzePortfolio(ADDR, makePosition(), txs);
    expect(performance.operationBreakdown.deposit).toBe(1);
    expect(performance.operationBreakdown.borrow).toBe(1);
  });

  it('sets firstTransactionAt and lastTransactionAt correctly', () => {
    const { performance } = analyzePortfolio(ADDR, makePosition(), txs);
    expect(performance.firstTransactionAt).toBe('2024-01-01T00:00:00Z');
    expect(performance.lastTransactionAt).toBe('2024-01-05T00:00:00Z');
  });

  it('returns zero totals for empty history', () => {
    const { performance } = analyzePortfolio(ADDR, makePosition(), []);
    expect(performance.totalDeposited).toBe('0');
    expect(performance.netFlow).toBe('0');
    expect(performance.transactionCount).toBe(0);
    expect(performance.firstTransactionAt).toBeUndefined();
  });
});

// ─── CSV export ────────────────────────────────────────────────────────────────

describe('toCSV', () => {
  const txs: TransactionHistoryItem[] = [
    makeTx({ type: 'deposit', amount: '1000000', timestamp: '2024-01-01T00:00:00Z' }),
    makeTx({
      type: 'borrow',
      amount: '500000',
      timestamp: '2024-01-02T00:00:00Z',
      transactionHash: 'tx2',
    }),
  ];

  it('produces a header row as the first line', () => {
    const csv = toCSV(txs);
    const lines = csv.split('\n');
    expect(lines[0]).toBe('date,type,amount,assetAddress,txHash,ledger,status');
  });

  it('produces one data row per transaction', () => {
    const csv = toCSV(txs);
    const lines = csv.split('\n').filter(Boolean);
    expect(lines).toHaveLength(3); // header + 2 rows
  });

  it('returns only the header for empty history', () => {
    const csv = toCSV([]);
    const lines = csv.split('\n').filter(Boolean);
    expect(lines).toHaveLength(1);
  });

  it('includes transaction hash in each row', () => {
    const csv = toCSV(txs);
    expect(csv).toContain('tx_abc');
    expect(csv).toContain('tx2');
  });
});

// ─── Top-level shape ──────────────────────────────────────────────────────────

describe('analyzePortfolio – output shape', () => {
  it('always returns all required top-level keys', () => {
    const result = analyzePortfolio(ADDR, makePosition(), []);
    expect(result).toHaveProperty('userAddress');
    expect(result).toHaveProperty('portfolioValue');
    expect(result).toHaveProperty('riskMetrics');
    expect(result).toHaveProperty('suggestions');
    expect(result).toHaveProperty('performance');
    expect(result).toHaveProperty('generatedAt');
  });

  it('echoes back the user address', () => {
    const result = analyzePortfolio(ADDR, makePosition(), []);
    expect(result.userAddress).toBe(ADDR);
  });

  it('generatedAt is a valid ISO timestamp', () => {
    const result = analyzePortfolio(ADDR, makePosition(), []);
    expect(new Date(result.generatedAt).toISOString()).toBe(result.generatedAt);
  });
});

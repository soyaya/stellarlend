import { describe, expect, it } from 'vitest';
import {
  analyzeTrace,
  calculateTraceOverhead,
  type ContractTraceSnapshot,
} from '../src/devtools/trace-analysis.js';

describe('trace-analysis', () => {
  it('reconstructs nested call stacks and aggregates gas usage', () => {
    const snapshot: ContractTraceSnapshot = {
      transactionHash: 'tx-123',
      network: 'mainnet',
      ledger: 101,
      invocations: [
        {
          contractId: 'CLEND',
          functionName: 'borrow',
          gasUsed: 12,
          cpuInstructions: 100,
          memoryBytes: 64,
          stateChanges: [{ key: 'debt:alice', operation: 'update', before: '10', after: '25' }],
          children: [
            {
              contractId: 'CORACLE',
              functionName: 'get_price',
              gasUsed: 4,
              cpuInstructions: 40,
              memoryBytes: 16,
            },
            {
              contractId: 'CTOKEN',
              functionName: 'transfer',
              gasUsed: 8,
              cpuInstructions: 60,
              memoryBytes: 24,
              stateChanges: [
                { key: 'balance:pool', operation: 'update', before: '100', after: '75' },
              ],
            },
          ],
        },
      ],
    };

    const analysis = analyzeTrace(snapshot);

    expect(analysis.totalGasUsed).toBe(24);
    expect(analysis.totalCpuInstructions).toBe(200);
    expect(analysis.totalMemoryBytes).toBe(104);
    expect(analysis.maxDepth).toBe(1);
    expect(analysis.totalInvocations).toBe(3);
    expect(analysis.totalStateChanges).toBe(2);
    expect(analysis.hotPaths[0]).toEqual({
      path: 'CLEND.borrow',
      gasUsed: 24,
      percentageOfTotalGas: 100,
    });
    expect(analysis.callFrames.map((frame) => frame.path)).toEqual([
      'CLEND.borrow',
      'CLEND.borrow > CORACLE.get_price',
      'CLEND.borrow > CTOKEN.transfer',
    ]);
  });

  it('surfaces warnings for large traces and expensive tracing overhead', () => {
    const snapshot: ContractTraceSnapshot = {
      elapsedMs: 10,
      tracingElapsedMs: 15,
      invocations: Array.from({ length: 3 }, (_, index) => ({
        contractId: `C${index}`,
        functionName: 'ping',
        gasUsed: 0,
        children: [
          {
            contractId: `C${index}A`,
            functionName: 'pong',
            gasUsed: 1,
            stateChanges: [{ key: `k${index}`, operation: 'create', after: '1' }],
          },
        ],
      })),
    };

    const analysis = analyzeTrace(snapshot, { largeTraceThreshold: 2 });

    expect(analysis.warnings).toContain(
      'Trace contains 6 invocations; consider filtering by contract or function before storing it long term.'
    );
    expect(analysis.warnings).toContain(
      'One or more frames reported zero local gas. Check the RPC simulator payload before using this trace for regression analysis.'
    );
    expect(analysis.warnings).toContain(
      'Trace overhead is 50%; disable tracing for load tests and keep it to focused debugging sessions.'
    );
    expect(analysis.traceOverhead).toEqual({
      baselineMs: 10,
      tracingMs: 15,
      deltaMs: 5,
      overheadPercent: 50,
    });
  });

  it('calculates trace overhead only when both timings are usable', () => {
    expect(calculateTraceOverhead(undefined, 12)).toBeUndefined();
    expect(calculateTraceOverhead(12, undefined)).toBeUndefined();
    expect(calculateTraceOverhead(12, 10)).toBeUndefined();
    expect(calculateTraceOverhead(20, 25)).toEqual({
      baselineMs: 20,
      tracingMs: 25,
      deltaMs: 5,
      overheadPercent: 25,
    });
  });
});

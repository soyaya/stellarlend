import { config } from '../config';

type PauseReason = 'manual' | 'auto-failure-threshold';

interface PauseState {
  paused: boolean;
  reason: PauseReason | null;
  since: number | null;
}

interface QueuedWithdrawal {
  userAddress: string;
  assetAddress?: string;
  amount: string;
  queuedAt: string;
}

class EmergencyPauseService {
  private state: PauseState = { paused: false, reason: null, since: null };
  private consecutiveFailures = 0;
  private readonly withdrawalQueue: QueuedWithdrawal[] = [];

  isPaused(): PauseState {
    return { ...this.state };
  }

  recordSuccess(): void {
    this.consecutiveFailures = 0;
  }

  recordFailure(): void {
    this.consecutiveFailures += 1;
    if (this.consecutiveFailures >= config.emergency.autoPauseFailureThreshold) {
      this.pause('auto-failure-threshold');
    }
  }

  pause(reason: PauseReason): void {
    this.state = {
      paused: true,
      reason,
      since: Date.now(),
    };
  }

  resume(): void {
    this.state = { paused: false, reason: null, since: null };
    this.consecutiveFailures = 0;
  }

  queueWithdrawal(entry: Omit<QueuedWithdrawal, 'queuedAt'>): void {
    this.withdrawalQueue.push({
      ...entry,
      queuedAt: new Date().toISOString(),
    });
  }

  drainWithdrawalQueue(): QueuedWithdrawal[] {
    const drained = [...this.withdrawalQueue];
    this.withdrawalQueue.length = 0;
    return drained;
  }

  getWithdrawalQueue(): QueuedWithdrawal[] {
    return [...this.withdrawalQueue];
  }
}

export const emergencyPauseService = new EmergencyPauseService();

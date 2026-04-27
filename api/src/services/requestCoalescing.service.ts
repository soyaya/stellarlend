import logger from '../utils/logger';

export interface CoalescingOptions {
  /** Maximum time to wait for coalescing (ms) */
  maxWaitMs: number;
  /** Grace period before starting coalescing (ms) */
  gracePeriodMs: number;
  /** Maximum number of concurrent requests to coalesce */
  maxConcurrent: number;
  /** Whether to enable coalescing */
  enabled: boolean;
}

export interface CoalescingMetrics {
  totalRequests: number;
  coalescedRequests: number;
  activeCoalescingGroups: number;
  averageWaitTime: number;
  timeouts: number;
  errors: number;
}

export interface PendingRequest<T> {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (error: any) => void;
  startTime: number;
  timeoutId: NodeJS.Timeout;
  requestCount: number;
}

/**
 * Request Coalescing Service
 *
 * Deduplicates concurrent identical requests to reduce backend load.
 * Multiple requests for the same data share a single backend call.
 */
export class RequestCoalescingService {
  private pendingRequests = new Map<string, PendingRequest<any>>();
  private metrics: CoalescingMetrics = {
    totalRequests: 0,
    coalescedRequests: 0,
    activeCoalescingGroups: 0,
    averageWaitTime: 0,
    timeouts: 0,
    errors: 0,
  };

  private options: CoalescingOptions;

  constructor(options: Partial<CoalescingOptions> = {}) {
    this.options = {
      maxWaitMs: 5000, // 5 seconds
      gracePeriodMs: 50, // 50ms grace period
      maxConcurrent: 10, // Max 10 concurrent requests per key
      enabled: true,
      ...options,
    };
  }

  /**
   * Execute a request with coalescing
   *
   * @param key - Unique key identifying the request
   * @param executor - Function that executes the actual request
   * @returns Promise that resolves with the result
   */
  async execute<T>(key: string, executor: () => Promise<T>): Promise<T> {
    this.metrics.totalRequests++;

    if (!this.options.enabled) {
      return executor();
    }

    const startTime = Date.now();

    // Check if there's already a pending request for this key
    const existing = this.pendingRequests.get(key);
    if (existing && existing.requestCount < this.options.maxConcurrent) {
      // Join the existing request
      existing.requestCount++;
      this.metrics.coalescedRequests++;

      logger.debug(`Coalescing request for key: ${key} (${existing.requestCount} total)`);

      try {
        const result = await existing.promise;
        this.updateMetrics(startTime);
        return result;
      } catch (error) {
        this.updateMetrics(startTime);
        throw error;
      }
    }

    // Create a new coalescing group
    return new Promise<T>((resolve, reject) => {
      const pending: PendingRequest<T> = {
        promise: null as any, // Will be set below
        resolve,
        reject,
        startTime,
        timeoutId: setTimeout(() => {
          this.handleTimeout(key, pending);
        }, this.options.maxWaitMs),
        requestCount: 1,
      };

      // Create the shared promise
      pending.promise = this.createSharedPromise(key, executor, pending);
      pending.promise.catch(() => {
        // The outer promise is rejected via pending.reject; observing this
        // internal promise prevents unhandled rejections under test/Node.
      });

      this.pendingRequests.set(key, pending);
      this.metrics.activeCoalescingGroups++;

      logger.debug(`Created new coalescing group for key: ${key}`);
    });
  }

  /**
   * Create a shared promise for the coalescing group
   */
  private async createSharedPromise<T>(
    key: string,
    executor: () => Promise<T>,
    pending: PendingRequest<T>
  ): Promise<T> {
    try {
      // Wait for grace period to allow more requests to coalesce
      await new Promise((resolve) => setTimeout(resolve, this.options.gracePeriodMs));

      const result = await executor();

      // Resolve all waiting requests
      pending.resolve(result);
      this.cleanup(key);

      return result;
    } catch (error) {
      // Reject all waiting requests
      pending.reject(error);
      this.metrics.errors++;
      this.cleanup(key);

      throw error;
    }
  }

  /**
   * Handle timeout for coalescing group
   */
  private handleTimeout<T>(key: string, pending: PendingRequest<T>) {
    logger.warn(`Coalescing timeout for key: ${key} after ${this.options.maxWaitMs}ms`);

    this.metrics.timeouts++;

    // Reject all waiting requests with timeout error
    pending.reject(new Error(`Request coalescing timeout for key: ${key}`));
    this.cleanup(key);
  }

  /**
   * Clean up completed coalescing group
   */
  private cleanup(key: string) {
    const pending = this.pendingRequests.get(key);
    if (pending) {
      clearTimeout(pending.timeoutId);
      this.pendingRequests.delete(key);
      this.metrics.activeCoalescingGroups--;
    }
  }

  /**
   * Update performance metrics
   */
  private updateMetrics(startTime: number) {
    const waitTime = Date.now() - startTime;
    // Simple moving average
    this.metrics.averageWaitTime = (this.metrics.averageWaitTime + waitTime) / 2;
  }

  /**
   * Generate a cache key from request parameters
   */
  generateKey(method: string, params: Record<string, any>): string {
    // Sort keys for consistent hashing
    const sortedParams = Object.keys(params)
      .sort()
      .reduce(
        (result, key) => {
          result[key] = params[key];
          return result;
        },
        {} as Record<string, any>
      );

    return `${method}:${JSON.stringify(sortedParams)}`;
  }

  /**
   * Get current metrics
   */
  getMetrics(): CoalescingMetrics {
    return { ...this.metrics };
  }

  /**
   * Reset metrics
   */
  resetMetrics(): void {
    this.metrics = {
      totalRequests: 0,
      coalescedRequests: 0,
      activeCoalescingGroups: 0,
      averageWaitTime: 0,
      timeouts: 0,
      errors: 0,
    };
  }

  /**
   * Get coalescing statistics
   */
  getStats() {
    const metrics = this.getMetrics();
    const coalescingRate =
      metrics.totalRequests > 0 ? (metrics.coalescedRequests / metrics.totalRequests) * 100 : 0;

    return {
      ...metrics,
      coalescingRate: `${coalescingRate.toFixed(1)}%`,
      activeGroups: this.pendingRequests.size,
    };
  }

  /**
   * Graceful shutdown - reject all pending requests
   */
  async shutdown(): Promise<void> {
    logger.info('Shutting down request coalescing service...');

    for (const [key, pending] of this.pendingRequests) {
      clearTimeout(pending.timeoutId);
      pending.reject(new Error('Service shutting down'));
    }

    this.pendingRequests.clear();
    logger.info('Request coalescing service shut down');
  }
}

// Global instance for the application
export const requestCoalescingService = new RequestCoalescingService({
  maxWaitMs: 5000,
  gracePeriodMs: 50,
  maxConcurrent: 10,
  enabled: true,
});

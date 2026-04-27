import { Request, Response, NextFunction } from 'express';
import { requestCoalescingService } from '../services/requestCoalescing.service';
import logger from '../utils/logger';

export interface CoalescingMiddlewareOptions {
  /** Custom key generator function */
  keyGenerator?: (req: Request) => string;
  /** Whether to enable coalescing for this route */
  enabled?: boolean;
  /** Maximum wait time in milliseconds */
  maxWaitMs?: number;
  /** Grace period in milliseconds */
  gracePeriodMs?: number;
}

/**
 * Middleware that enables request coalescing for routes.
 * Multiple concurrent requests for the same resource share a single backend call.
 *
 * @param options Configuration options for coalescing
 * @returns Express middleware function
 */
export function withRequestCoalescing(options: CoalescingMiddlewareOptions = {}) {
  const { keyGenerator, enabled = true, maxWaitMs, gracePeriodMs } = options;

  return (req: Request, res: Response, next: NextFunction) => {
    if (!enabled) {
      return next();
    }

    // Generate coalescing key
    const key = keyGenerator ? keyGenerator(req) : generateDefaultKey(req);

    // Store original send method
    const originalSend = res.send;
    const originalJson = res.json;
    let coalescingPromise: Promise<any> | null = null;
    let coalescingResult: any = null;
    let coalescingError: any = null;
    let isCompleted = false;

    // Wrap response methods to capture the result
    const captureResult = (result: any) => {
      coalescingResult = result;
      isCompleted = true;
      return result;
    };

    const captureError = (error: any) => {
      coalescingError = error;
      isCompleted = true;
      throw error;
    };

    res.send = function (data: any) {
      return originalSend.call(this, captureResult(data));
    };

    res.json = function (data: any) {
      return originalJson.call(this, captureResult(data));
    };

    // Execute with coalescing
    coalescingPromise = requestCoalescingService.execute(key, async () => {
      try {
        // Call next middleware/route handler
        await new Promise<void>((resolve, reject) => {
          const originalNext = next;
          next = () => resolve();

          // Temporarily replace next to capture completion
          const tempNext = (...args: any[]) => {
            if (args[0]) {
              reject(args[0]);
            } else {
              resolve();
            }
          };

          // Call the route handler
          try {
            // Reset next function
            next = tempNext;
            originalNext();
          } catch (error) {
            reject(error);
          }
        });

        // Wait for response to be completed
        let attempts = 0;
        const maxAttempts = 50; // 5 seconds max wait
        while (!isCompleted && attempts < maxAttempts) {
          await new Promise((resolve) => setTimeout(resolve, 100));
          attempts++;
        }

        if (!isCompleted) {
          throw new Error('Response not completed within timeout');
        }

        if (coalescingError) {
          throw coalescingError;
        }

        return coalescingResult;
      } catch (error) {
        logger.error('Request coalescing error:', error);
        throw error;
      }
    });

    // Handle the coalesced response
    coalescingPromise
      .then((result) => {
        if (!res.headersSent) {
          if (typeof result === 'string') {
            res.send(result);
          } else {
            res.json(result);
          }
        }
      })
      .catch((error) => {
        if (!res.headersSent) {
          logger.error('Coalesced request failed:', error);
          if (!res.statusCode || res.statusCode === 200) {
            res.status(500);
          }
          res.json({ error: 'Internal server error' });
        }
      });
  };
}

/**
 * Generate a default coalescing key from the request
 */
function generateDefaultKey(req: Request): string {
  const { method, path, query, params, body } = req;

  // Extract relevant parameters for key generation
  const keyData: any = {
    method,
    path,
  };

  // Include query parameters (sorted for consistency)
  if (Object.keys(query).length > 0) {
    keyData.query = Object.keys(query)
      .sort()
      .reduce((result: any, key) => {
        result[key] = query[key];
        return result;
      }, {});
  }

  // Include route parameters
  if (Object.keys(params).length > 0) {
    keyData.params = params;
  }

  // Include relevant body fields (exclude sensitive data)
  if (body && typeof body === 'object' && Object.keys(body).length > 0) {
    const safeBody: any = {};
    for (const [key, value] of Object.entries(body)) {
      // Exclude sensitive fields
      if (!['password', 'secret', 'token', 'key'].includes(key.toLowerCase())) {
        safeBody[key] = value;
      }
    }
    if (Object.keys(safeBody).length > 0) {
      keyData.body = safeBody;
    }
  }

  return requestCoalescingService.generateKey('http', keyData);
}

/**
 * Create coalescing middleware for specific routes
 */
export function createCoalescingMiddleware(
  keyGenerator?: (req: Request) => string,
  options: Partial<CoalescingMiddlewareOptions> = {}
) {
  return withRequestCoalescing({
    keyGenerator,
    ...options,
  });
}

/**
 * Pre-configured middleware for common use cases
 */
export const coalescingMiddleware = {
  // For protocol stats (no parameters)
  protocolStats: createCoalescingMiddleware((req) =>
    requestCoalescingService.generateKey('protocolStats', {})
  ),

  // For user-specific data
  userData: createCoalescingMiddleware((req) => {
    const userId = req.params.userId || req.query.userAddress || req.body.userAddress;
    return requestCoalescingService.generateKey('userData', { userId });
  }),

  // For paginated data
  paginatedData: createCoalescingMiddleware((req) => {
    const { limit, cursor, offset } = req.query;
    const params = req.params;
    return requestCoalescingService.generateKey('paginatedData', {
      ...params,
      limit,
      cursor,
      offset,
    });
  }),

  // Generic coalescing (uses default key generation)
  generic: createCoalescingMiddleware(),
};

import { Request, Response, NextFunction } from 'express';
import { config } from '../config';
import { ConflictError, ValidationError } from '../utils/errors';
import { BoundedTtlCache } from '../utils/boundedTtlCache';

const UUID_V4_REGEX =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

interface CachedResponse {
  signature: string;
  status: number;
  body: unknown;
}

interface PendingResponse {
  signature: string;
  promise: Promise<CachedResponse>;
  reject: (error: Error) => void;
  resolve: (response: CachedResponse) => void;
}

class IdempotencyStore {
  private readonly completed = new BoundedTtlCache<CachedResponse>({
    ttlMs: config.cache.idempotencyTtlMs,
    maxEntries: config.cache.idempotencyMaxEntries,
  });

  private readonly pending = new Map<string, PendingResponse>();

  getCompleted(key: string): CachedResponse | undefined {
    return this.completed.get(key);
  }

  getPending(key: string): PendingResponse | undefined {
    return this.pending.get(key);
  }

  begin(key: string, signature: string): void {
    let resolve!: (response: CachedResponse) => void;
    let reject!: (error: Error) => void;

    const promise = new Promise<CachedResponse>((res, rej) => {
      resolve = res;
      reject = rej;
    });

    this.pending.set(key, {
      signature,
      promise,
      reject,
      resolve,
    });
  }

  complete(key: string, response: CachedResponse): void {
    const pendingResponse = this.pending.get(key);
    if (pendingResponse) {
      pendingResponse.resolve(response);
      this.pending.delete(key);
    }

    this.completed.set(key, response);
  }

  abort(key: string): void {
    const pendingResponse = this.pending.get(key);
    if (!pendingResponse) {
      return;
    }

    pendingResponse.reject(new Error('Idempotent request aborted before response was cached'));
    this.pending.delete(key);
  }

  clear(): void {
    for (const pendingResponse of this.pending.values()) {
      pendingResponse.reject(new Error('Idempotency store cleared'));
    }

    this.pending.clear();
    this.completed.clear();
  }
}

export const idempotencyStore = new IdempotencyStore();

function buildRequestSignature(req: Request): string {
  const body =
    req.body && typeof req.body === 'object' ? JSON.stringify(req.body) : String(req.body ?? '');

  return `${req.method}:${req.baseUrl}${req.path}:${body}`;
}

function respondFromCache(res: Response, cachedResponse: CachedResponse): Response {
  res.setHeader('Idempotency-Status', 'cached');
  return res.status(cachedResponse.status).json(cachedResponse.body);
}

export async function idempotencyMiddleware(
  req: Request,
  res: Response,
  next: NextFunction
): Promise<void> {
  if (req.method !== 'POST') {
    next();
    return;
  }

  const key = req.header('Idempotency-Key');
  if (!key) {
    next();
    return;
  }

  if (!UUID_V4_REGEX.test(key)) {
    next(new ValidationError('Idempotency-Key must be a valid UUID'));
    return;
  }

  const signature = buildRequestSignature(req);
  const cachedResponse = idempotencyStore.getCompleted(key);
  if (cachedResponse) {
    if (cachedResponse.signature !== signature) {
      next(new ConflictError('Idempotency-Key has already been used for a different request'));
      return;
    }

    respondFromCache(res, cachedResponse);
    return;
  }

  const pendingResponse = idempotencyStore.getPending(key);
  if (pendingResponse) {
    if (pendingResponse.signature !== signature) {
      next(new ConflictError('Idempotency-Key has already been used for a different request'));
      return;
    }

    try {
      const awaitedResponse = await pendingResponse.promise;
      respondFromCache(res, awaitedResponse);
      return;
    } catch {
      // The original request ended before caching a response, so allow this
      // retry to become the active request for the idempotency key.
    }
  }

  idempotencyStore.begin(key, signature);
  res.setHeader('Idempotency-Status', 'created');

  const originalJson = res.json.bind(res);
  res.json = ((body: unknown) => {
    idempotencyStore.complete(key, {
      signature,
      status: res.statusCode,
      body,
    });
    return originalJson(body);
  }) as Response['json'];

  res.on('close', () => {
    if (!res.writableEnded) {
      idempotencyStore.abort(key);
    }
  });

  next();
}

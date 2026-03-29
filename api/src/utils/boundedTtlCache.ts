interface CacheEntry<T> {
  createdAt: number;
  expiresAt: number;
  value: T;
}

interface CacheOptions {
  ttlMs: number;
  maxEntries: number;
}

export class BoundedTtlCache<T> {
  private readonly ttlMs: number;
  private readonly maxEntries: number;
  private readonly entries = new Map<string, CacheEntry<T>>();

  constructor({ ttlMs, maxEntries }: CacheOptions) {
    this.ttlMs = ttlMs;
    this.maxEntries = maxEntries;
  }

  get(key: string): T | undefined {
    this.pruneExpired();

    const entry = this.entries.get(key);
    if (!entry) {
      return undefined;
    }

    if (entry.expiresAt <= Date.now()) {
      this.entries.delete(key);
      return undefined;
    }

    return entry.value;
  }

  set(key: string, value: T): void {
    const now = Date.now();

    this.pruneExpired(now);
    this.entries.delete(key);
    this.entries.set(key, {
      createdAt: now,
      expiresAt: now + this.ttlMs,
      value,
    });

    this.enforceBounds();
  }

  delete(key: string): void {
    this.entries.delete(key);
  }

  clear(): void {
    this.entries.clear();
  }

  size(): number {
    this.pruneExpired();
    return this.entries.size;
  }

  private pruneExpired(now = Date.now()): void {
    for (const [key, entry] of this.entries.entries()) {
      if (entry.expiresAt <= now) {
        this.entries.delete(key);
      }
    }
  }

  private enforceBounds(): void {
    while (this.entries.size > this.maxEntries) {
      const oldestKey = this.entries.keys().next().value;
      if (!oldestKey) {
        break;
      }
      this.entries.delete(oldestKey);
    }
  }
}

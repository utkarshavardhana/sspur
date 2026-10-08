export interface Window {
  readonly limit: number;
  readonly size: number;
  readonly start: number;
  readonly count: number;
}

export interface Bucket {
  readonly capacity: number;
  readonly rate: number;
  readonly tokens: number;
  readonly last: number;
}

export interface Keyed {
  readonly capacity: number;
  readonly rate: number;
  readonly buckets: readonly (readonly [string, Bucket])[];
}

export class LimitErr extends Error {}

export class ClockSkew extends LimitErr {
  constructor(readonly last: number, readonly now: number) {
    super(`clock went from ${last} to ${now}`);
  }
}

export class TooLarge extends LimitErr {
  constructor(readonly n: number, readonly capacity: number) {
    super(`${n} > capacity ${capacity}`);
  }
}

export function newWindow(limit: number, size: number): Window {
  return { limit, size, start: 0, count: 0 };
}

export function allow(w: Window, now: number): [boolean, Window] {
  if (now < w.start) throw new ClockSkew(w.start, now);
  const w2 = now >= w.start + w.size ? { ...w, start: now, count: 0 } : w;
  if (w2.count < w2.limit) return [true, { ...w2, count: w2.count + 1 }];
  return [false, w2];
}

export function runRequests(w: Window, times: readonly number[]): boolean[] {
  let cur = w;
  const out: boolean[] = [];
  for (const t of times) {
    const [ok, next] = allow(cur, t);
    cur = next;
    out.push(ok);
  }
  return out;
}

export function newBucket(capacity: number, rate: number, now: number): Bucket {
  return { capacity, rate, tokens: capacity, last: now };
}

function refill(b: Bucket, now: number): Bucket {
  if (now < b.last) throw new ClockSkew(b.last, now);
  return { ...b, tokens: Math.min(b.capacity, b.tokens + (now - b.last) * b.rate), last: now };
}

export function take(b: Bucket, now: number, n: number): [boolean, Bucket] {
  const r = refill(b, now);
  if (r.tokens >= n) return [true, { ...r, tokens: r.tokens - n }];
  return [false, r];
}

export function retryAfter(b: Bucket, now: number, n: number): number {
  if (n > b.capacity) throw new TooLarge(n, b.capacity);
  const r = refill(b, now);
  if (r.tokens >= n) return 0;
  return Math.floor((n - r.tokens + r.rate - 1) / r.rate);
}

export function newKeyed(capacity: number, rate: number): Keyed {
  return { capacity, rate, buckets: [] };
}

export function takeKey(k: Keyed, key: string, now: number): [boolean, Keyed] {
  const found = k.buckets.find(([name]) => name === key);
  const b = found !== undefined ? found[1] : newBucket(k.capacity, k.rate, now);
  const [ok, b2] = take(b, now, 1);
  return [ok, { ...k, buckets: [...k.buckets.filter(([name]) => name !== key), [key, b2]] }];
}

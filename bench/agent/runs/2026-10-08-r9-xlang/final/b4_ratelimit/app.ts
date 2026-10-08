export interface Window {
  readonly limit: number;
  readonly size: number;
  readonly start: number;
  readonly count: number;
}

export function newWindow(limit: number, size: number): Window {
  return { limit, size, start: 0, count: 0 };
}

export class LimitErr extends Error {
  constructor(message: string) {
    super(message);
    this.name = "LimitErr";
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

export class ClockSkew extends LimitErr {
  constructor(readonly last: number, readonly now: number) {
    super(`clock skew: now ${now} < last ${last}`);
    this.name = "ClockSkew";
  }
}

export class TooLarge extends LimitErr {
  constructor(readonly n: number, readonly capacity: number) {
    super(`too large: ${n} > capacity ${capacity}`);
    this.name = "TooLarge";
  }
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

export interface Bucket {
  readonly capacity: number;
  readonly rate: number;
  readonly tokens: number;
  readonly last: number;
}

export function newBucket(capacity: number, rate: number, now: number): Bucket {
  return { capacity, rate, tokens: capacity, last: now };
}

function refill(b: Bucket, now: number): Bucket {
  if (now < b.last) throw new ClockSkew(b.last, now);
  const tokens = Math.min(b.capacity, b.tokens + (now - b.last) * b.rate);
  return { ...b, tokens, last: now };
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
  return Math.ceil((n - r.tokens) / r.rate);
}

export interface Keyed {
  readonly capacity: number;
  readonly rate: number;
  readonly buckets: readonly (readonly [string, Bucket])[];
}

export function newKeyed(capacity: number, rate: number): Keyed {
  return { capacity, rate, buckets: [] };
}

export function takeKey(k: Keyed, key: string, now: number): [boolean, Keyed] {
  const found = k.buckets.find(([kk]) => kk === key);
  const b = found ? found[1] : newBucket(k.capacity, k.rate, now);
  const [ok, nb] = take(b, now, 1);
  const buckets: (readonly [string, Bucket])[] = found
    ? k.buckets.map((e) => (e[0] === key ? ([key, nb] as const) : e))
    : [...k.buckets, [key, nb] as const];
  return [ok, { ...k, buckets }];
}

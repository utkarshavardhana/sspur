export interface Window {
  readonly limit: number;
  readonly size: number;
  readonly start: number;
  readonly count: number;
}

export function newWindow(limit: number, size: number): Window {
  return { limit, size, start: 0, count: 0 };
}

export function allow(w: Window, now: number): [boolean, Window] {
  const w2 = now >= w.start + w.size ? { ...w, start: now, count: 0 } : w;
  if (w2.count <= w2.limit) return [true, { ...w2, count: w2.count + 1 }];
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

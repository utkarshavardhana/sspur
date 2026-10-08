export interface Entry {
  readonly key: string;
  readonly value: number;
}

export interface Cache {
  readonly cap: number;
  readonly entries: readonly Entry[];
  readonly hits: number;
  readonly misses: number;
}

export class CacheErr extends Error {}

export class BadCapacity extends CacheErr {
  constructor(readonly cap: number) {
    super(`bad capacity ${cap}`);
  }
}

export function newCache(cap: number): Cache {
  if (cap < 1) throw new BadCapacity(cap);
  return { cap, entries: [], hits: 0, misses: 0 };
}

function trim(c: Cache): Cache {
  return { ...c, entries: c.entries.slice(Math.max(0, c.entries.length - c.cap)) };
}

function touch(c: Cache, key: string, value: number): Cache {
  return { ...c, entries: [...c.entries.filter((e) => e.key !== key), { key, value }] };
}

export function peek(c: Cache, key: string): number | undefined {
  for (const e of c.entries) {
    if (e.key === key) return e.value;
  }
  return undefined;
}

export function lookup(c: Cache, key: string): [number | undefined, Cache] {
  const v = peek(c, key);
  if (v === undefined) return [undefined, { ...c, misses: c.misses + 1 }];
  return [v, { ...touch(c, key, v), hits: c.hits + 1 }];
}

export function getOr(c: Cache, key: string, def: number): [number, Cache] {
  const [v, c2] = lookup(c, key);
  return [v === undefined ? def : v, c2];
}

export function put(c: Cache, key: string, value: number): Cache {
  return trim(touch(c, key, value));
}

export function resize(c: Cache, cap: number): Cache {
  if (cap < 1) throw new BadCapacity(cap);
  return trim({ ...c, cap });
}

export function keys(c: Cache): string[] {
  return c.entries.map((e) => e.key);
}

export function size(c: Cache): number {
  return c.entries.length;
}

export function describe(c: Cache): string {
  return `cap=${c.cap} [` + c.entries.map((e) => `${e.key}=${e.value}`).join(", ") + "]";
}

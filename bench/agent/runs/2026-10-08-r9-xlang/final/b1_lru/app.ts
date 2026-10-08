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
  if (!(cap >= 1)) throw new BadCapacity(cap);
  return { cap, entries: [], hits: 0, misses: 0 };
}

export function peek(c: Cache, key: string): number | undefined {
  return c.entries.find((e) => e.key === key)?.value;
}

export function lookup(c: Cache, key: string): [number | undefined, Cache] {
  const e = c.entries.find((x) => x.key === key);
  if (e === undefined) return [undefined, { ...c, misses: c.misses + 1 }];
  return [e.value, { ...c, entries: [...c.entries.filter((x) => x.key !== key), e], hits: c.hits + 1 }];
}

export function getOr(c: Cache, key: string, def: number): [number, Cache] {
  const [v, c2] = lookup(c, key);
  return [v === undefined ? def : v, c2];
}

export function put(c: Cache, key: string, value: number): Cache {
  let rest = c.entries.filter((e) => e.key !== key);
  if (rest.length >= c.cap) rest = rest.slice(rest.length - c.cap + 1);
  return { ...c, entries: [...rest, { key, value }] };
}

export function resize(c: Cache, cap: number): Cache {
  if (!(cap >= 1)) throw new BadCapacity(cap);
  const entries = c.entries.length > cap ? c.entries.slice(c.entries.length - cap) : c.entries;
  return { ...c, cap, entries };
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

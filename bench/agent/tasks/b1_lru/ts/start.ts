export interface Entry {
  readonly key: string;
  readonly value: number;
}

export interface Cache {
  readonly cap: number;
  readonly entries: readonly Entry[];
}

export class CacheErr extends Error {}

export class BadCapacity extends CacheErr {
  constructor(readonly cap: number) {
    super(`bad capacity ${cap}`);
  }
}

export function newCache(cap: number): Cache {
  return { cap, entries: [] };
}

export function lookup(c: Cache, key: string): number | undefined {
  for (const e of c.entries) {
    if (e.key === key) return e.value;
  }
  return undefined;
}

export function getOr(c: Cache, key: string, def: number): number {
  const v = lookup(c, key);
  return v === undefined ? def : v;
}

export function put(c: Cache, key: string, value: number): Cache {
  return { ...c, entries: [...c.entries.filter((e) => e.key !== key), { key, value }] };
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

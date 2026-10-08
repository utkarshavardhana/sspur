export interface Entry {
  readonly key: string;
  readonly value: string;
}

export class CfgErr extends Error {}

export class Missing extends CfgErr {
  constructor(readonly key: string) {
    super(key);
  }
}

export class NotInt extends CfgErr {
  constructor(readonly key: string, readonly value: string) {
    super(`${key}: not an int: ${value}`);
  }
}

export class NotBool extends CfgErr {
  constructor(readonly key: string, readonly value: string) {
    super(`${key}: not a bool: ${value}`);
  }
}

export function entry(key: string, value: string): Entry {
  return { key, value };
}

export function parseLine(line: string): Entry | undefined {
  const t = line.trim();
  const i = t.indexOf("=");
  if (t.startsWith("#") || i < 0) return undefined;
  const key = t.slice(0, i).trim();
  if (key === "") return undefined;
  return entry(key, t.slice(i + 1).trim());
}

export function parse(text: string): Entry[] {
  return text
    .split(";")
    .map(parseLine)
    .filter((e): e is Entry => e !== undefined);
}

export function lookup(cfg: readonly Entry[], key: string): string | undefined {
  for (let i = cfg.length - 1; i >= 0; i--) {
    if (cfg[i].key === key) return cfg[i].value;
  }
  return undefined;
}

export function getStr(cfg: readonly Entry[], key: string): string {
  const v = lookup(cfg, key);
  if (v === undefined) throw new Missing(key);
  return v;
}

export function getInt(cfg: readonly Entry[], key: string): number {
  const v = getStr(cfg, key);
  if (!/^-?[0-9]+$/.test(v)) throw new NotInt(key, v);
  return parseInt(v, 10);
}

export function getBool(cfg: readonly Entry[], key: string, def: boolean): boolean {
  const v = lookup(cfg, key);
  if (v === undefined) return def;
  const l = v.toLowerCase();
  if (["true", "yes", "1"].includes(l)) return true;
  if (["false", "no", "0"].includes(l)) return false;
  throw new NotBool(key, v);
}

export function keys(cfg: readonly Entry[]): string[] {
  return [...new Set(cfg.map((e) => e.key))];
}

export function sampleText(): string {
  return "host=localhost;port=8080;debug=true";
}

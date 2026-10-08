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

export function entry(key: string, value: string): Entry {
  return { key, value };
}

export function parseLine(line: string): Entry | undefined {
  const parts = line.split("=");
  if (parts.length === 2) return entry(parts[0], parts[1]);
  return undefined;
}

export function parse(text: string): Entry[] {
  return text
    .split(";")
    .map(parseLine)
    .filter((e): e is Entry => e !== undefined);
}

export function lookup(cfg: readonly Entry[], key: string): string | undefined {
  return cfg.find((e) => e.key === key)?.value;
}

export function getStr(cfg: readonly Entry[], key: string): string {
  const v = lookup(cfg, key);
  if (v === undefined) throw new Missing(key);
  return v;
}

export function keys(cfg: readonly Entry[]): string[] {
  return [...new Set(cfg.map((e) => e.key))];
}

export function sampleText(): string {
  return "host=localhost;port=8080;debug=true";
}

export interface Row {
  readonly name: string;
  readonly rate: number;
  readonly hours: number;
}

export function row(name: string, rate: number, hours: number): Row {
  return { name, rate, hours };
}

export class CsvErr extends Error {}

export class BadRow extends CsvErr {
  constructor(readonly line: number) {
    super(`line ${line}`);
  }
}

export class BadNumber extends CsvErr {
  constructor(readonly line: number, readonly field: string) {
    super(`line ${line}: ${field}`);
  }
}

export function sample(): string {
  return "Beth,400,0\nDan,375,0\nKathy,400,10\nMark,500,20\nMary,550,22\nSusie,425,18";
}

// Parses an integer the way Python's int() does: surrounding whitespace, an optional sign, digits.
function toInt(s: string): number | undefined {
  const t = s.trim();
  if (!/^[+-]?[0-9]+(_[0-9]+)*$/.test(t)) return undefined;
  return parseInt(t.replace(/_/g, ""), 10);
}

function num(s: string, line: number, field: string): number {
  const n = toInt(s);
  if (n === undefined || n < 0) throw new BadNumber(line, field);
  return n;
}

export function parseRow(line: string, n: number): Row {
  const f = line.split(",").map((x) => x.trim());
  if (f.length !== 3) throw new BadRow(n);
  return row(f[0], num(f[1], n, "rate"), num(f[2], n, "hours"));
}

function dataLines(text: string): [number, string][] {
  const out: [number, string][] = [];
  text.split("\n").forEach((line, k) => {
    const i = k + 1;
    const t = line.trim();
    if (t === "" || (i === 1 && t === "name,rate,hours")) return;
    out.push([i, line]);
  });
  return out;
}

export function parse(text: string): Row[] {
  return dataLines(text).map(([n, line]) => parseRow(line, n));
}

export function pay(r: Row): number {
  if (r.hours > 40) return r.rate * 40 + Math.floor((r.rate * 3 * (r.hours - 40)) / 2);
  return r.rate * r.hours;
}

export function report(rows: readonly Row[]): string {
  const sorted = [...rows].sort((a, b) => pay(b) - pay(a) || (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
  const lines = sorted.map((r) => `${r.name}: ${pay(r)}`);
  return [...lines, `total: ${rows.reduce((s, r) => s + pay(r), 0)}`].join("\n");
}

export function validate(text: string): string[] {
  const out: string[] = [];
  for (const [n, line] of dataLines(text)) {
    try {
      parseRow(line, n);
    } catch (e) {
      if (e instanceof BadRow) out.push(`line ${e.line}: expected 3 fields`);
      else if (e instanceof BadNumber) out.push(`line ${e.line}: bad ${e.field}`);
      else throw e;
    }
  }
  return out;
}

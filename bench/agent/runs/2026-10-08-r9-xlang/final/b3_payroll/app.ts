export interface Row {
  readonly name: string;
  readonly rate: number;
  readonly hours: number;
}

export function row(name: string, rate: number, hours: number): Row {
  return { name, rate, hours };
}

export function sample(): string {
  return "Beth,400,0\nDan,375,0\nKathy,400,10\nMark,500,20\nMary,550,22\nSusie,425,18";
}

export class CsvErr extends Error {
  constructor(message: string) {
    super(message);
    this.name = new.target.name;
  }
}

export class BadRow extends CsvErr {
  constructor(readonly line: number) {
    super(`line ${line}: expected 3 fields`);
  }
}

export class BadNumber extends CsvErr {
  constructor(
    readonly line: number,
    readonly field: string,
  ) {
    super(`line ${line}: bad ${field}`);
  }
}

const HEADER = "name,rate,hours";

function isNat(s: string): boolean {
  return /^[0-9]+$/.test(s);
}

function parseLine(line: string, no: number): Row {
  const f = line.split(",").map((x) => x.trim());
  if (f.length !== 3) throw new BadRow(no);
  if (!isNat(f[1])) throw new BadNumber(no, "rate");
  if (!isNat(f[2])) throw new BadNumber(no, "hours");
  return row(f[0], parseInt(f[1], 10), parseInt(f[2], 10));
}

function dataLines(text: string): [string, number][] {
  const out: [string, number][] = [];
  text.split("\n").forEach((l, i) => {
    const t = l.trim();
    if (t === "" || (i === 0 && t === HEADER)) return;
    out.push([t, i + 1]);
  });
  return out;
}

export function parse(text: string): Row[] {
  return dataLines(text).map(([l, n]) => parseLine(l, n));
}

export function validate(text: string): string[] {
  const errs: string[] = [];
  for (const [l, n] of dataLines(text)) {
    try {
      parseLine(l, n);
    } catch (e) {
      if (e instanceof CsvErr) errs.push(e.message);
      else throw e;
    }
  }
  return errs;
}

export function pay(r: Row): number {
  const base = r.rate * Math.min(r.hours, 40);
  if (r.hours <= 40) return base;
  return base + Math.floor((r.rate * 3 * (r.hours - 40)) / 2);
}

export function report(rows: readonly Row[]): string {
  const sorted = [...rows].sort((a, b) => pay(b) - pay(a) || (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
  const lines = sorted.map((r) => `${r.name}: ${pay(r)}`);
  lines.push(`total: ${rows.reduce((s, r) => s + pay(r), 0)}`);
  return lines.join("\n");
}

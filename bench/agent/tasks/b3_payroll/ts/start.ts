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

// Parses an integer the way Python's int() does: surrounding whitespace, an optional sign, digits.
function toInt(s: string): number {
  const t = s.trim();
  if (!/^[+-]?[0-9]+(_[0-9]+)*$/.test(t)) return 0;
  return parseInt(t.replace(/_/g, ""), 10);
}

export function parseRow(line: string): Row {
  const f = line.split(",");
  if (f.length < 3) throw new RangeError("list index out of range");
  return row(f[0], toInt(f[1]), toInt(f[2]));
}

export function parse(text: string): Row[] {
  return text.split("\n").map(parseRow);
}

export function pay(r: Row): number {
  return r.rate * r.hours;
}

export function report(rows: readonly Row[]): string {
  return rows.map((r) => `${r.name}: ${pay(r)}`).join("\n");
}

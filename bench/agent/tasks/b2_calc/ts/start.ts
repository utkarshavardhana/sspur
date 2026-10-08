export interface Num {
  readonly kind: "num";
  readonly v: number;
}

export interface Op {
  readonly kind: "op";
  readonly c: string;
}

export type Tok = Num | Op;

export class CalcErr extends Error {}

export class BadChar extends CalcErr {
  constructor(readonly ch: string) {
    super(`bad character ${ch}`);
  }
}

export function isDigit(ch: string): boolean {
  return ch.length === 1 && "0123456789".includes(ch);
}

export function tokenize(s: string): Tok[] {
  const toks: Tok[] = [];
  let num = "";
  for (const ch of s) {
    if (isDigit(ch)) {
      num += ch;
    } else {
      if (num !== "") {
        toks.push({ kind: "num", v: parseInt(num, 10) });
        num = "";
      }
      if (ch === "+" || ch === "-") {
        toks.push({ kind: "op", c: ch });
      } else if (ch !== " ") {
        throw new BadChar(ch);
      }
    }
  }
  if (num !== "") toks.push({ kind: "num", v: parseInt(num, 10) });
  return toks;
}

export function evalTokens(toks: readonly Tok[]): number {
  let total = 0;
  let sign = 1;
  for (const t of toks) {
    if (t.kind === "num") {
      total += sign * t.v;
    } else {
      sign = t.c === "-" ? -1 : 1;
    }
  }
  return total;
}

export function calc(s: string): number {
  return evalTokens(tokenize(s));
}

export function showResult(s: string): string {
  try {
    return `${s} = ${calc(s)}`;
  } catch (e) {
    if (e instanceof BadChar) return `${s} = error: bad character ${e.ch}`;
    throw e;
  }
}

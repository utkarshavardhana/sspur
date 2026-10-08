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

export class DivByZero extends CalcErr {}

export class BadSyntax extends CalcErr {}

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
      if (ch.length === 1 && "+-*/()".includes(ch)) {
        toks.push({ kind: "op", c: ch });
      } else if (ch !== " ") {
        throw new BadChar(ch);
      }
    }
  }
  if (num !== "") toks.push({ kind: "num", v: parseInt(num, 10) });
  return toks;
}

function isOp(toks: readonly Tok[], i: number, c: string): boolean {
  const t = toks[i];
  return t !== undefined && t.kind === "op" && t.c === c;
}

function apply(c: string, a: number, b: number): number {
  if (c === "+") return a + b;
  if (c === "-") return a - b;
  if (c === "*") return a * b;
  if (b === 0) throw new DivByZero();
  const q = Math.floor(Math.abs(a) / Math.abs(b));
  return (a >= 0) === (b >= 0) ? q : -q;
}

function parseExpr(toks: readonly Tok[], i: number): [number, number] {
  let [acc, j] = parseTerm(toks, i);
  while (isOp(toks, j, "+") || isOp(toks, j, "-")) {
    const c = isOp(toks, j, "+") ? "+" : "-";
    const [b, k] = parseTerm(toks, j + 1);
    acc = apply(c, acc, b);
    j = k;
  }
  return [acc, j];
}

function parseTerm(toks: readonly Tok[], i: number): [number, number] {
  let [acc, j] = parseUnary(toks, i);
  while (isOp(toks, j, "*") || isOp(toks, j, "/")) {
    const c = isOp(toks, j, "*") ? "*" : "/";
    const [b, k] = parseUnary(toks, j + 1);
    acc = apply(c, acc, b);
    j = k;
  }
  return [acc, j];
}

function parseUnary(toks: readonly Tok[], i: number): [number, number] {
  if (isOp(toks, i, "-")) {
    const [v, j] = parseUnary(toks, i + 1);
    return [-v, j];
  }
  return parseAtom(toks, i);
}

function parseAtom(toks: readonly Tok[], i: number): [number, number] {
  const t = toks[i];
  if (t !== undefined && t.kind === "num") return [t.v, i + 1];
  if (isOp(toks, i, "(")) {
    const [v, j] = parseExpr(toks, i + 1);
    if (!isOp(toks, j, ")")) throw new BadSyntax();
    return [v, j + 1];
  }
  throw new BadSyntax();
}

export function calc(s: string): number {
  const toks = tokenize(s);
  const [v, i] = parseExpr(toks, 0);
  if (i !== toks.length) throw new BadSyntax();
  return v + 0;
}

export function showResult(s: string): string {
  try {
    return `${s} = ${calc(s)}`;
  } catch (e) {
    if (e instanceof BadChar) return `${s} = error: bad character ${e.ch}`;
    if (e instanceof DivByZero) return `${s} = error: division by zero`;
    if (e instanceof BadSyntax) return `${s} = error: syntax`;
    throw e;
  }
}

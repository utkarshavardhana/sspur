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

export class DivByZero extends CalcErr {
  constructor() {
    super("division by zero");
  }
}

export class BadSyntax extends CalcErr {
  constructor() {
    super("syntax");
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
      if ("+-*/()".includes(ch)) {
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
  let pos = 0;
  const isOp = (c: string): boolean => {
    const t = toks[pos];
    return t !== undefined && t.kind === "op" && t.c === c;
  };

  function expr(): number {
    let left = term();
    while (isOp("+") || isOp("-")) {
      const plus = isOp("+");
      pos++;
      const right = term();
      left = plus ? left + right : left - right;
    }
    return left;
  }

  function term(): number {
    let left = unary();
    while (isOp("*") || isOp("/")) {
      const mul = isOp("*");
      pos++;
      const right = unary();
      if (mul) {
        left = left * right;
      } else {
        if (right === 0) throw new DivByZero();
        left = Math.trunc(left / right);
      }
    }
    return left;
  }

  function unary(): number {
    if (isOp("-")) {
      pos++;
      return -unary();
    }
    return atom();
  }

  function atom(): number {
    const t = toks[pos];
    if (t === undefined) throw new BadSyntax();
    if (t.kind === "num") {
      pos++;
      return t.v;
    }
    if (t.c === "(") {
      pos++;
      const v = expr();
      if (!isOp(")")) throw new BadSyntax();
      pos++;
      return v;
    }
    throw new BadSyntax();
  }

  const result = expr();
  if (pos < toks.length) throw new BadSyntax();
  return result === 0 ? 0 : result;
}

export function calc(s: string): number {
  return evalTokens(tokenize(s));
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

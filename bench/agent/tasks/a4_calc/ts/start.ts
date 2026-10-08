export class Num {
  readonly kind = "num";
  constructor(readonly v: number) {}
}

export class Var {
  readonly kind = "var";
  constructor(readonly name: string) {}
}

export class Add {
  readonly kind = "add";
  constructor(readonly a: Expr, readonly b: Expr) {}
}

export class Mul {
  readonly kind = "mul";
  constructor(readonly a: Expr, readonly b: Expr) {}
}

export type Expr = Num | Var | Add | Mul;

export class CalcErr extends Error {}

export class Unbound extends CalcErr {
  constructor(readonly name: string) {
    super(name);
  }
}

export function evalExpr(e: Expr, env: ReadonlyMap<string, number>): number {
  switch (e.kind) {
    case "num":
      return e.v;
    case "var": {
      const v = env.get(e.name);
      if (v === undefined) throw new Unbound(e.name);
      return v;
    }
    case "add":
      return evalExpr(e.a, env) + evalExpr(e.b, env);
    case "mul":
      return evalExpr(e.a, env) * evalExpr(e.b, env);
  }
}

export function show(e: Expr): string {
  switch (e.kind) {
    case "num":
      return String(e.v);
    case "var":
      return e.name;
    case "add":
      return `(${show(e.a)} + ${show(e.b)})`;
    case "mul":
      return `${show(e.a)} * ${show(e.b)}`;
  }
}

function isNum(e: Expr, v: number): boolean {
  return e instanceof Num && e.v === v;
}

export function simplify(e: Expr): Expr {
  if (e instanceof Add) {
    if (isNum(e.a, 0)) return simplify(e.b);
    if (isNum(e.b, 0)) return simplify(e.a);
    return new Add(simplify(e.a), simplify(e.b));
  }
  if (e instanceof Mul) {
    if (isNum(e.a, 1)) return simplify(e.b);
    if (isNum(e.b, 1)) return simplify(e.a);
    return new Mul(simplify(e.a), simplify(e.b));
  }
  return e;
}

export function run(e: Expr, env: ReadonlyMap<string, number>): string {
  try {
    return `${show(e)} = ${evalExpr(e, env)}`;
  } catch (err) {
    if (err instanceof Unbound) return `${show(e)} = error: unbound ${err.name}`;
    throw err;
  }
}

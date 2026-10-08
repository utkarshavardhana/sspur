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

export class Sub {
  readonly kind = "sub";
  constructor(readonly a: Expr, readonly b: Expr) {}
}

export class Div {
  readonly kind = "div";
  constructor(readonly a: Expr, readonly b: Expr) {}
}

export type Expr = Num | Var | Add | Mul | Sub | Div;

export class CalcErr extends Error {}

export class Unbound extends CalcErr {
  constructor(readonly name: string) {
    super(name);
  }
}

export class DivByZero extends CalcErr {}

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
    case "sub":
      return evalExpr(e.a, env) - evalExpr(e.b, env);
    case "div": {
      const a = evalExpr(e.a, env);
      const b = evalExpr(e.b, env);
      if (b === 0) throw new DivByZero();
      return Math.floor(a / b);
    }
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
    case "sub":
      return `(${show(e.a)} - ${show(e.b)})`;
    case "div":
      return `${show(e.a)} / ${show(e.b)}`;
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
  if (e instanceof Sub) {
    if (isNum(e.b, 0)) return simplify(e.a);
    return new Sub(simplify(e.a), simplify(e.b));
  }
  if (e instanceof Div) {
    if (isNum(e.b, 1)) return simplify(e.a);
    return new Div(simplify(e.a), simplify(e.b));
  }
  return e;
}

export function run(e: Expr, env: ReadonlyMap<string, number>): string {
  try {
    return `${show(e)} = ${evalExpr(e, env)}`;
  } catch (err) {
    if (err instanceof Unbound) return `${show(e)} = error: unbound ${err.name}`;
    if (err instanceof DivByZero) return `${show(e)} = error: division by zero`;
    throw err;
  }
}

export function varNames(e: Expr): string[] {
  const out: string[] = [];
  const walk = (x: Expr): void => {
    if (x instanceof Var) {
      if (!out.includes(x.name)) out.push(x.name);
    } else if (!(x instanceof Num)) {
      walk(x.a);
      walk(x.b);
    }
  };
  walk(e);
  return out;
}

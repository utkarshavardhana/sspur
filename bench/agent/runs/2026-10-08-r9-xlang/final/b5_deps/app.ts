export interface Pkg {
  readonly name: string;
  readonly deps: readonly string[];
}

export class DepErr extends Error {}

export class Unknown extends DepErr {
  constructor(readonly name: string) {
    super(name);
  }
}

export function pkg(name: string, deps: readonly string[]): Pkg {
  return { name, deps };
}

export function registry(): Pkg[] {
  return [pkg("app", ["web", "db"]), pkg("web", ["http", "log"]), pkg("db", ["log"]), pkg("http", []), pkg("log", [])];
}

export function findPkg(reg: readonly Pkg[], name: string): Pkg {
  for (const p of reg) {
    if (p.name === name) return p;
  }
  throw new Unknown(name);
}

export class Missing extends DepErr {
  constructor(
    readonly pkg: string,
    readonly dep: string,
  ) {
    super(`${pkg} needs missing ${dep}`);
  }
}

export class Cycle extends DepErr {
  constructor(readonly path: readonly string[]) {
    super(path.join(" -> "));
  }
}

export class Stuck extends DepErr {
  constructor(readonly names: readonly string[]) {
    super(names.join(", "));
  }
}

export function installOrder(reg: readonly Pkg[], target: string): string[] {
  findPkg(reg, target);
  const out: string[] = [];
  const done = new Set<string>();
  const chain: string[] = [];
  const visit = (name: string): void => {
    const p = findPkg(reg, name);
    chain.push(name);
    for (const d of p.deps) {
      const at = chain.indexOf(d);
      if (at >= 0) throw new Cycle([...chain.slice(at), d]);
      if (done.has(d)) continue;
      if (!reg.some((q) => q.name === d)) throw new Missing(name, d);
      visit(d);
    }
    chain.pop();
    done.add(name);
    out.push(name);
  };
  visit(target);
  return out;
}

export function plan(reg: readonly Pkg[], target: string): string {
  try {
    return installOrder(reg, target).join(" -> ");
  } catch (e) {
    if (e instanceof Unknown) return `error: unknown package ${e.name}`;
    if (e instanceof Missing) return `error: ${e.pkg} needs missing ${e.dep}`;
    if (e instanceof Cycle) return `error: cycle ${e.path.join(" -> ")}`;
    throw e;
  }
}

export function fullOrder(reg: readonly Pkg[]): string[] {
  const out: string[] = [];
  const done = new Set<string>();
  let rest = [...new Set(reg.map((p) => p.name))].sort();
  while (rest.length > 0) {
    const next = rest.find((n) => findPkg(reg, n).deps.every((d) => done.has(d)));
    if (next === undefined) throw new Stuck(rest);
    out.push(next);
    done.add(next);
    rest = rest.filter((n) => n !== next);
  }
  return out;
}

export function dependents(reg: readonly Pkg[], name: string): string[] {
  findPkg(reg, name);
  const seen = new Set<string>([name]);
  const queue = [name];
  while (queue.length > 0) {
    const cur = queue.shift() as string;
    for (const p of reg) {
      if (p.deps.includes(cur) && !seen.has(p.name)) {
        seen.add(p.name);
        queue.push(p.name);
      }
    }
  }
  seen.delete(name);
  return [...seen].sort();
}

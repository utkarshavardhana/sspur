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

export class Missing extends DepErr {
  constructor(readonly pkg: string, readonly dep: string) {
    super(`${pkg} needs ${dep}`);
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

function visit(reg: readonly Pkg[], name: string, chain: readonly string[], done: string[]): void {
  if (done.includes(name)) return;
  const at = chain.indexOf(name);
  if (at >= 0) throw new Cycle([...chain.slice(at), name]);
  const p = findPkg(reg, name);
  for (const d of p.deps) {
    if (!reg.some((q) => q.name === d)) throw new Missing(name, d);
    visit(reg, d, [...chain, name], done);
  }
  done.push(name);
}

export function installOrder(reg: readonly Pkg[], target: string): string[] {
  const done: string[] = [];
  visit(reg, target, [], done);
  return done;
}

export function plan(reg: readonly Pkg[], target: string): string {
  try {
    return installOrder(reg, target).join(" -> ");
  } catch (e) {
    if (e instanceof Unknown) return `error: unknown package ${e.name}`;
    if (e instanceof Missing) return `error: ${e.pkg} needs missing ${e.dep}`;
    if (e instanceof Cycle) return "error: cycle " + e.path.join(" -> ");
    throw e;
  }
}

export function fullOrder(reg: readonly Pkg[]): string[] {
  const done: string[] = [];
  for (;;) {
    const ready = reg
      .filter((p) => !done.includes(p.name) && p.deps.every((d) => done.includes(d)))
      .map((p) => p.name)
      .sort();
    if (ready.length === 0) break;
    done.push(ready[0]);
  }
  if (done.length < reg.length) {
    throw new Stuck(reg.filter((p) => !done.includes(p.name)).map((p) => p.name).sort());
  }
  return done;
}

export function dependents(reg: readonly Pkg[], name: string): string[] {
  findPkg(reg, name);
  const found = new Set([name]);
  let changed = true;
  while (changed) {
    changed = false;
    for (const p of reg) {
      if (!found.has(p.name) && p.deps.some((d) => found.has(d))) {
        found.add(p.name);
        changed = true;
      }
    }
  }
  found.delete(name);
  return [...found].sort();
}

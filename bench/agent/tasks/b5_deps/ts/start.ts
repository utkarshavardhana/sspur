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

export function installOrder(reg: readonly Pkg[], target: string): string[] {
  const p = findPkg(reg, target);
  const out: string[] = [];
  for (const d of p.deps) out.push(...installOrder(reg, d));
  return [...out, target];
}

export function plan(reg: readonly Pkg[], target: string): string {
  try {
    return installOrder(reg, target).join(" -> ");
  } catch (e) {
    if (e instanceof Unknown) return `error: unknown package ${e.name}`;
    throw e;
  }
}

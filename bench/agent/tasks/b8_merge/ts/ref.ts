// A JSON value. Objects keep their keys in insertion order.
export type J = null | boolean | number | string | J[] | { [k: string]: J };

export class ConfigErr extends Error {}

export class MissingKey extends ConfigErr {
  constructor(readonly path: string) {
    super(path);
  }
}

export function isObj(j: J | undefined): j is { [k: string]: J } {
  return typeof j === "object" && j !== null && !Array.isArray(j);
}

function quote(s: string): string {
  return '"' + s.replaceAll("\\", "\\\\").replaceAll('"', '\\"') + '"';
}

export function render(j: J): string {
  if (j === null) return "null";
  if (typeof j === "boolean") return j ? "true" : "false";
  if (typeof j === "number") return String(j);
  if (typeof j === "string") return quote(j);
  if (Array.isArray(j)) return "[" + j.map(render).join(",") + "]";
  return "{" + Object.entries(j).map(([k, v]) => quote(k) + ":" + render(v)).join(",") + "}";
}

export function getField(j: J, key: string): J | undefined {
  if (isObj(j)) return Object.hasOwn(j, key) ? j[key] : undefined;
  if (Array.isArray(j) && /^[0-9]+$/.test(key)) {
    const i = Number(key);
    return i < j.length ? j[i] : undefined;
  }
  return undefined;
}

export function getPath(j: J, path: string): J | undefined {
  let cur: J | undefined = j;
  for (const k of path.split(".")) {
    if (cur === undefined) return undefined;
    cur = getField(cur, k);
  }
  return cur;
}

export function merge(target: J, patch: J): J {
  if (!isObj(patch)) return patch;
  const out: { [k: string]: J } = isObj(target) ? { ...target } : {};
  for (const [k, v] of Object.entries(patch)) {
    if (v === null) delete out[k];
    else out[k] = merge(Object.hasOwn(out, k) ? out[k] : null, v);
  }
  return out;
}

export function mergeAll(base: J, patches: readonly J[]): J {
  let cur = base;
  for (const p of patches) cur = merge(cur, p);
  return cur;
}

export function requirePaths(j: J, paths: readonly string[]): void {
  for (const path of paths) {
    if (getPath(j, path) === undefined) throw new MissingKey(path);
  }
}

export function defaults(): J {
  return { name: "app", port: 8080, db: { host: "localhost", pool: 5 }, tags: ["a"] };
}

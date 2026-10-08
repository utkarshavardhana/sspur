// A JSON value. Objects keep their keys in insertion order.
export type J = null | boolean | number | string | J[] | { [k: string]: J };

export function isObj(j: J | undefined): j is { [k: string]: J } {
  return typeof j === "object" && j !== null && !Array.isArray(j);
}

function esc(s: string): string {
  return '"' + s.replace(/\\/g, "\\\\").replace(/"/g, '\\"') + '"';
}

export function render(j: J): string {
  if (j === null) return "null";
  if (typeof j === "boolean") return j ? "true" : "false";
  if (typeof j === "number") return String(j);
  if (typeof j === "string") return esc(j);
  if (Array.isArray(j)) return "[" + j.map(render).join(",") + "]";
  return "{" + Object.entries(j).map(([k, v]) => esc(k) + ":" + render(v)).join(",") + "}";
}

export function getField(j: J, key: string): J | undefined {
  if (isObj(j) && Object.hasOwn(j, key)) return j[key];
  return undefined;
}

export function getPath(j: J, path: string): J | undefined {
  let cur: J | undefined = j;
  for (const k of path.split(".")) {
    if (Array.isArray(cur)) {
      cur = /^\d+$/.test(k) && Number(k) < cur.length ? cur[Number(k)] : undefined;
    } else {
      cur = cur === undefined ? undefined : getField(cur, k);
    }
    if (cur === undefined) return undefined;
  }
  return cur;
}

function setKey(o: { [k: string]: J }, k: string, v: J): void {
  Object.defineProperty(o, k, { value: v, enumerable: true, writable: true, configurable: true });
}

export function merge(target: J, patch: J): J {
  if (!isObj(patch)) return patch;
  const out: { [k: string]: J } = {};
  const t = isObj(target) ? target : {};
  for (const [k, v] of Object.entries(t)) setKey(out, k, v);
  for (const [k, v] of Object.entries(patch)) {
    if (v === null) {
      delete out[k];
    } else {
      setKey(out, k, merge(Object.hasOwn(t, k) ? t[k] : null, v));
    }
  }
  return out;
}

export function mergeAll(base: J, patches: readonly J[]): J {
  return patches.reduce<J>((acc, p) => merge(acc, p), base);
}

export class ConfigErr extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ConfigErr";
  }
}

export class MissingKey extends ConfigErr {
  readonly path: string;
  constructor(path: string) {
    super("missing key: " + path);
    this.name = "MissingKey";
    this.path = path;
  }
}

export function requirePaths(j: J, paths: readonly string[]): void {
  for (const p of paths) {
    if (getPath(j, p) === undefined) throw new MissingKey(p);
  }
}

export function defaults(): J {
  return { name: "app", port: 8080, db: { host: "localhost", pool: 5 }, tags: ["a"] };
}

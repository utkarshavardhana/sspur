// A JSON value. Objects keep their keys in insertion order.
export type J = null | boolean | number | string | J[] | { [k: string]: J };

export function isObj(j: J | undefined): j is { [k: string]: J } {
  return typeof j === "object" && j !== null && !Array.isArray(j);
}

export function render(j: J): string {
  if (j === null) return "null";
  if (typeof j === "boolean") return j ? "true" : "false";
  if (typeof j === "number") return String(j);
  if (typeof j === "string") return '"' + j + '"';
  if (Array.isArray(j)) return "[" + j.map(render).join(",") + "]";
  return "{" + Object.entries(j).map(([k, v]) => '"' + k + '":' + render(v)).join(",") + "}";
}

export function getField(j: J, key: string): J | undefined {
  if (isObj(j) && Object.hasOwn(j, key)) return j[key];
  return undefined;
}

export function getPath(j: J, path: string): J | undefined {
  let cur: J | undefined = j;
  for (const k of path.split(".")) {
    if (cur === undefined || cur === null) return undefined;
    cur = getField(cur, k);
  }
  return cur;
}

export function merge(target: J, patch: J): J {
  if (isObj(target) && isObj(patch)) {
    const out: { [k: string]: J } = {};
    for (const [k, v] of Object.entries(target)) {
      if (!Object.hasOwn(patch, k)) out[k] = v;
    }
    return Object.assign(out, patch);
  }
  return patch;
}

export function defaults(): J {
  return { name: "app", port: 8080, db: { host: "localhost", pool: 5 }, tags: ["a"] };
}

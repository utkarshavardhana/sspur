// Runs the WebAssembly build of SSPUR off the page's thread. The page sends {id, op, ...} and
// gets back {id, result} or {id, error}.
import init, * as sspur from "./pkg/sspur_wasm.js";

let ready = null;
let panic = null;
globalThis.sspurPanic = (msg) => {
  panic = msg;
};

const ops = {
  check: (m) => sspur.check(m.src),
  run: (m) => sspur.run(m.src),
  test: (m) => sspur.test(m.src),
  fmt: (m) => sspur.fmt(m.src),
  complete: (m) => sspur.complete(m.src, m.pos),
  hover: (m) => sspur.hover(m.src, m.pos),
  builtins: () => sspur.builtins(),
  tokens: (m) => String(sspur.tokens(m.src)),
};

self.onmessage = async (e) => {
  const m = e.data;
  if (m.op === "init") {
    ready = init({ module_or_path: m.module });
    await ready;
    self.postMessage({ id: m.id, result: true });
    return;
  }
  await ready;
  try {
    self.postMessage({ id: m.id, result: JSON.parse(ops[m.op](m)) });
  } catch (err) {
    const msg = panic || (err instanceof RangeError ? "the program ran out of stack" : String((err && err.message) || err));
    self.postMessage({ id: m.id, error: msg, dead: true });
  }
};

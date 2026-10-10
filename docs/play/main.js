// The playground page: an editor, a checker that runs as you type, and Run, Test, Format and
// Share. All of SSPUR runs in Web Workers from the WebAssembly build; nothing leaves the page.
import { encode, fromHash } from "./share.js";

const $ = (id) => document.getElementById(id);
const STORE = "sspur-play-source";
const THEME = "sspur-play-theme";

// A worker that answers requests in order and can be replaced when it hangs or crashes.
class Engine {
  constructor(module) {
    this.module = module;
    this.next = 0;
    this.waiting = new Map();
    this.worker = null;
  }
  start() {
    this.worker = new Worker(new URL("worker.js", import.meta.url), { type: "module" });
    this.worker.onmessage = (e) => {
      const { id, result, error, dead } = e.data;
      const w = this.waiting.get(id);
      this.waiting.delete(id);
      if (dead) this.stop();
      if (w) error ? w.reject(new Error(error)) : w.resolve(result);
    };
    this.ready = this.call("init", { module: this.module });
  }
  call(op, args = {}) {
    if (!this.worker) this.start();
    const id = ++this.next;
    return new Promise((resolve, reject) => {
      this.waiting.set(id, { resolve, reject });
      this.worker.postMessage({ id, op, ...args });
    });
  }
  stop(reason = "stopped") {
    if (this.worker) this.worker.terminate();
    this.worker = null;
    for (const w of this.waiting.values()) w.reject(new Error(reason));
    this.waiting.clear();
  }
}

function prefersDark() {
  const own = localStorage.getItem(THEME);
  if (own) return own === "dark";
  const book = localStorage.getItem("mdbook-theme");
  if (book) return ["coal", "navy", "ayu"].includes(book.replace(/"/g, ""));
  return matchMedia("(prefers-color-scheme: dark)").matches;
}

function el(tag, cls, text) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

async function main() {
  let dark = prefersDark();
  document.documentElement.classList.toggle("dark", dark);
  const wasm = WebAssembly.compileStreaming(fetch("pkg/sspur_wasm_bg.wasm"));
  const [{ createEditor }, examples, module] = await Promise.all([import("./editor.bundle.js"), fetch("examples.json").then((r) => r.json()), wasm]);
  const ide = new Engine(module);
  const runner = new Engine(module);
  const builtins = await ide.call("builtins");
  $("engine").textContent = "Ready";

  const pick = $("examples");
  examples.forEach((ex, i) => pick.appendChild(new Option(ex.name, String(i))));
  const shared = await fromHash(location.hash).catch(() => null);
  const initial = shared ?? localStorage.getItem(STORE) ?? examples[0].source;

  let seq = 0;
  let timer = 0;
  const api = {
    complete: (src, pos) => ide.call("complete", { src, pos }).catch(() => null),
    hover: (src, pos) => ide.call("hover", { src, pos }).catch(() => null),
    keywords: () => builtins.keywords,
  };
  const editor = createEditor({
    parent: $("editor"),
    doc: initial,
    dark,
    api,
    commands: { run: () => go("run"), test: () => go("test"), format, share },
    onChange(text) {
      localStorage.setItem(STORE, text);
      if (location.hash) history.replaceState(null, "", location.pathname);
      $("link").hidden = true;
      clearTimeout(timer);
      timer = setTimeout(() => check(text), 250);
    },
  });
  editor.view.dom.addEventListener("keyup", cursor);
  editor.view.dom.addEventListener("click", cursor);

  function cursor() {
    const s = editor.view.state;
    const head = s.selection.main.head;
    const line = s.doc.lineAt(head);
    $("cursor").textContent = `Ln ${line.number}, Col ${head - line.from + 1}`;
  }

  async function check(text) {
    const mine = ++seq;
    const watchdog = setTimeout(() => ide.stop("the checker took too long"), 8000);
    try {
      const r = await ide.call("check", { src: text });
      if (mine !== seq) return;
      editor.setDiagnostics(text, r.diags);
      problems(r.diags);
      const n = await ide.call("tokens", { src: text });
      $("tokens").textContent = `about ${n} tokens`;
    } catch (e) {
      $("engine").textContent = "Checker restarted: " + e.message;
    } finally {
      clearTimeout(watchdog);
    }
  }

  function problems(diags) {
    const box = $("problems");
    box.replaceChildren();
    const errors = diags.filter((d) => d.severity === "error").length;
    $("nprob").textContent = diags.length ? String(diags.length) : "";
    $("nprob").className = errors ? "badge err" : "badge";
    if (!diags.length) box.appendChild(el("p", "quiet", "No problems."));
    for (const d of diags) {
      const row = el("button", "problem " + d.severity);
      row.appendChild(el("span", "where", `${d.line}:${d.col}`));
      row.appendChild(el("span", "code", d.code));
      row.appendChild(el("span", "msg", d.message));
      if (d.hint) row.appendChild(el("span", "hint", "hint: " + d.hint));
      row.onclick = () => editor.goto(d.from);
      box.appendChild(row);
    }
  }

  function tab(name) {
    for (const b of document.querySelectorAll(".tabs button")) b.classList.toggle("on", b.dataset.tab === name);
    for (const p of document.querySelectorAll(".panel")) p.hidden = p.id !== name;
  }
  for (const b of document.querySelectorAll(".tabs button")) b.onclick = () => tab(b.dataset.tab);

  function diagLines(box, diags) {
    for (const d of diags.filter((x) => x.severity === "error")) {
      box.appendChild(el("div", "line err", `${d.line}:${d.col} ${d.code} ${d.message}`));
      if (d.hint) box.appendChild(el("div", "line hint", "  hint: " + d.hint));
    }
  }

  async function go(op) {
    const box = $("output");
    tab("output");
    box.replaceChildren(el("div", "line quiet", op === "run" ? "Running..." : "Testing..."));
    $("stop").hidden = false;
    $("run").disabled = $("test").disabled = true;
    const t0 = performance.now();
    try {
      const r = await runner.call(op, { src: editor.text() });
      const ms = Math.round(performance.now() - t0);
      box.replaceChildren();
      if (r.status === "error") {
        diagLines(box, r.diags);
        box.appendChild(el("div", "line quiet", "The program has errors, so it didn't run."));
      } else if (op === "run") {
        for (const l of r.output) box.appendChild(el("div", "line " + l.stream, l.text));
        if (r.status === "trap") box.appendChild(el("div", "line err", r.message));
        if (r.status === "trap" && /no 'main' function/.test(r.message)) box.appendChild(el("div", "line quiet", "This program has no main. Press Test to run its tests and examples."));
        if (r.status === "exit") box.appendChild(el("div", "line quiet", `exited with code ${r.code}`));
        box.appendChild(el("div", "line quiet", `done in ${ms} ms`));
      } else {
        for (const t of r.tests) {
          const row = el("div", "line test " + (t.ok ? "pass" : "fail"));
          row.appendChild(el("span", "mark", t.ok ? "pass" : "FAIL"));
          row.appendChild(el("span", null, t.name + (t.ok ? "" : ": " + t.message)));
          box.appendChild(row);
        }
        box.appendChild(el("div", "line " + (r.failed ? "err" : "quiet"), `${r.passed} passed, ${r.failed} failed (${ms} ms)`));
      }
    } catch (e) {
      box.replaceChildren(el("div", "line err", e.message === "stopped" ? "Stopped." : e.message));
    } finally {
      $("stop").hidden = true;
      $("run").disabled = $("test").disabled = false;
    }
  }

  async function format() {
    const r = await ide.call("fmt", { src: editor.text() });
    if (r.ok) editor.setText(r.text, true);
    else {
      problems(r.diags);
      tab("problems");
    }
  }

  async function share() {
    const url = location.origin + location.pathname + "#code=" + (await encode(editor.text()));
    history.replaceState(null, "", url);
    const field = $("link");
    field.value = url;
    field.hidden = false;
    field.select();
    try {
      await navigator.clipboard.writeText(url);
      $("engine").textContent = "Link copied";
    } catch {
      $("engine").textContent = "Copy the link below";
    }
  }

  $("run").onclick = () => go("run");
  $("test").onclick = () => go("test");
  $("stop").onclick = () => runner.stop();
  $("format").onclick = format;
  $("share").onclick = share;
  pick.onchange = () => {
    if (pick.value === "") return;
    editor.setText(examples[Number(pick.value)].source);
    pick.value = "";
    tab("output");
  };
  $("theme").onclick = () => {
    dark = !dark;
    localStorage.setItem(THEME, dark ? "dark" : "light");
    document.documentElement.classList.toggle("dark", dark);
    editor.setDark(dark);
  };
  window.addEventListener("hashchange", async () => {
    const src = await fromHash(location.hash).catch(() => null);
    if (src !== null && src !== editor.text()) editor.setText(src);
  });
  check(editor.text());
  cursor();
}

main().catch((e) => {
  $("engine").textContent = "The playground failed to load: " + e.message;
});

"""Create fresh work directories and print one agent prompt per (language, task).

  python3 setup.py <work_root> [--v1] [--r7] [--tmo] [--new] [--b] [--langs=sspur,python,ts,go]
      -> <work_root>/<lang>/<task>/ and <work_root>/prompts.json

--v1 uses the SSPUR instructions of the first run (q + apply tx.json); the default uses `edit`.
--new sets up only the tasks marked "new" (a9, outside the 8-task comparison).
--b sets up only the independently specified tasks (b1 to b8, "set": "b").
--r7 keeps the first step of runs 2 to 7 (`./sspur spec && ./sspur src`) instead of `./sspur start` (run 8 on).
--langs (run 9 on) picks the languages; the default is sspur,python. ts and go are the TypeScript
       and Go ports (tasks/<id>/ts, tasks/<id>/go, xlang/xlang.py).
--tmo (run 4 on) runs ./sspur and pytest under a 300 s timeout ($TMO, default ~/code/sspur-tools/tmo).
"""
import json, os, shutil, subprocess, sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "xlang"))
import xlang

HERE = os.path.dirname(os.path.abspath(__file__))
SSPUR = os.path.abspath(os.environ.get("SSPUR", os.path.join(HERE, "../../target/release/sspur")))
TMO = os.environ.get("TMO", os.path.expanduser("~/code/sspur-tools/tmo"))

COMMON = """{intro}

Task: {title}
{steps}

Required interface (hidden tests call exactly these names): {iface}

Do not look at, list, or search any directory other than {dir}. When every step is done and the tests pass, reply with the single word DONE."""

SSPUR_INTRO_V1 = """You are working on a small codebase written in SSPUR, a new programming language you have not seen before. Working directory: {dir}

- The code lives in a content-addressed store in .sspur/, and the CLI is ./sspur (run commands as `cd {dir} && ./sspur ...`).
- Start by reading the language reference with `./sspur spec`. It is the only documentation.
- Read and change code only through the CLI: `./sspur q <query>` queries, and `./sspur apply tx.json` transactions (write transaction files inside {dir}). `./sspur test` and `./sspur check` run against the store.
- Do not read or write anything under .sspur/ directly, do not create .ssp files, and do not run `./sspur init`."""

SSPUR_INTRO = """You are working on a small codebase written in SSPUR, a new programming language you have not seen before. Working directory: {dir}

- The code lives in a store in .sspur/, and the CLI is ./sspur (run commands as `cd {dir} && ./sspur ...`).
- Start with `cd {dir} && ./sspur start`: the language reference (the only documentation) and the whole codebase, in one command.
- Change code only with `./sspur edit` as the reference describes. `./sspur q`, `./sspur test` and `./sspur check` are also available.
- Do not read or write anything under .sspur/ directly, do not create .ssp files, and do not run `./sspur init`."""

R7_START = "./sspur spec && ./sspur src`: the language reference (the only documentation) and the whole codebase."

PY_INTRO = """You are working on a small Python 3.9 codebase. Working directory: {dir}

- All code is in app.py, with its tests at the bottom. Run them with `cd {dir} && LC_ALL=en_US.UTF-8 python3 -m pytest -q app.py`."""

TS_INTRO = """You are working on a small TypeScript codebase (Node 20, strict tsc). Working directory: {dir}

- All code is in app.ts, with its tests in app.test.ts (node:test). Type-check and run them with `cd {dir} && npx tsc && node --test --test-reporter=dot dist/`.
- The task text writes names in snake_case; in the code they are camelCase (total_value is totalValue)."""

GO_INTRO = """You are working on a small Go codebase (package main). Working directory: {dir}

- All code is in app.go, with its tests in app_test.go. Run them with `cd {dir} && go test`.
- The task text writes names in snake_case; in the code they are camelCase (total_value is totalValue). Where it says raise E, return an error of type *E."""


def main(root, v1=False, tmo=False, new=False, b=False, r7=False, langs=("sspur", "python")):
    tasks = [t for t in json.load(open(os.path.join(HERE, "tasks.json"))) if t.get("new", False) == new and (t.get("set", "a") == "b") == b]
    prompts = []
    for lang in langs:
        for t in tasks:
            d = os.path.abspath(os.path.join(root, lang, t["id"]))
            shutil.rmtree(d, ignore_errors=True)
            os.makedirs(d)
            src = os.path.join(HERE, "tasks", t["id"], "start.ssp" if lang == "sspur" else "start.py")
            if lang == "ts":
                p = os.path.join(HERE, "tasks", t["id"], "ts")
                xlang.write_ts_dir(d, open(os.path.join(p, "start.ts")).read(), open(os.path.join(p, "start.test.ts")).read())
                intro = TS_INTRO.format(dir=d)
                if tmo:
                    intro = intro.replace("&& npx tsc && node", f"&& {TMO} 300 npx tsc && {TMO} 300 node")
            elif lang == "go":
                p = os.path.join(HERE, "tasks", t["id"], "go")
                xlang.write_go_dir(d, open(os.path.join(p, "start.go")).read(), open(os.path.join(p, "start_test.go")).read())
                intro = GO_INTRO.format(dir=d)
                if tmo:
                    intro = intro.replace("&& go test`", f"&& {TMO} 300 go test`")
            elif lang == "sspur":
                shutil.copy(src, os.path.join(d, "start.ssp"))
                subprocess.run([SSPUR, "init", "start.ssp"], cwd=d, check=True, capture_output=True)
                os.remove(os.path.join(d, "start.ssp"))
                intro = (SSPUR_INTRO_V1 if v1 else SSPUR_INTRO.replace("./sspur start`: the language reference (the only documentation) and the whole codebase, in one command.", R7_START) if r7 else SSPUR_INTRO).format(dir=d)
                if tmo:
                    w = os.path.join(d, "sspur")
                    open(w, "w").write(f'#!/bin/sh\nexec {TMO} 300 {SSPUR} "$@"\n')
                    os.chmod(w, 0o755)
                    intro = intro.replace("./sspur ...`)", "./sspur ...`; ./sspur runs under a 300 s timeout)")
                else:
                    os.symlink(SSPUR, os.path.join(d, "sspur"))
            else:
                shutil.copy(src, os.path.join(d, "app.py"))
                intro = PY_INTRO.format(dir=d)
                if tmo:
                    intro = intro.replace("python3 -m pytest", f"{TMO} 300 python3 -m pytest")
            steps = "\n".join(f"{i + 1}. {s}" for i, s in enumerate(t["steps"]))
            prompt = COMMON.format(intro=intro, title=t["title"], steps=steps, iface=t[lang], dir=d)
            prompts.append({"lang": lang, "task": t["id"], "dir": d, "prompt": prompt})
    json.dump(prompts, open(os.path.join(root, "prompts.json"), "w"), indent=1)
    print(f"{len(prompts)} prompts in {root}/prompts.json")


if __name__ == "__main__":
    langs = next((a.split("=", 1)[1].split(",") for a in sys.argv[2:] if a.startswith("--langs=")), ["sspur", "python"])
    main(sys.argv[1], "--v1" in sys.argv[2:], "--tmo" in sys.argv[2:], "--new" in sys.argv[2:], "--b" in sys.argv[2:], "--r7" in sys.argv[2:], langs)

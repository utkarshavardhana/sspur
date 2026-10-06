"""Create the two work directories for the large-codebase task and write their prompts.

  python3 setup_scale.py <work_root>   -> <work_root>/{sspur,python}/s1_shop/ and <work_root>/prompts.json

./sspur and pytest run under a 300 s timeout ($TMO, default ~/code/sspur-tools/tmo), as in run 4.
"""
import json, os, shutil, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
SSPUR = os.path.abspath(os.environ.get("SSPUR", os.path.join(HERE, "../../../../target/release/sspur")))
TMO = os.environ.get("TMO", os.path.expanduser("~/code/sspur-tools/tmo"))

TITLE = "Shop: four targeted changes in a large codebase"
STEPS = [
    "The EU tax rate goes up from 19% to 21%.",
    "Bug: money formats the cents without a leading zero (1005 shows as \"10.5\"). It must always show two digits of cents: \"10.05\", \"0.07\", \"1234.00\".",
    "Express shipping: ship_fee gets a new last parameter express (a boolean); an express fee is twice the normal fee. Every existing caller passes false. Add order_express_shipping(x): like order_shipping, but express.",
    "Add warehouse_restock_all(ws, n): restocks every active warehouse by n with warehouse_restock; inactive ones are unchanged, and the order is kept.",
    "Add a test for each change. All existing tests must still pass.",
]
IFACE = {
    "sspur": "fn ship_fee(weight: Int, express: Bool) -> Int; fn order_express_shipping(x: Order) -> Int; fn warehouse_restock_all(ws: List[Warehouse], n: Int) -> List[Warehouse]; tax_rate, money and the other existing functions keep their signatures",
    "python": "shop.common.ship_fee(weight, express) -> int; shop.order.order_express_shipping(x) -> int; shop.warehouse.warehouse_restock_all(ws, n) -> List[Warehouse]; tax_rate, money and the other existing functions keep their modules and signatures",
}

SSPUR_INTRO = """You are working on a large codebase written in SSPUR, a new programming language you have not seen before. Working directory: {dir}

- The code lives in a store in .sspur/, and the CLI is ./sspur (run commands as `cd {dir} && ./sspur ...`; ./sspur runs under a 300 s timeout).
- Start with `cd {dir} && ./sspur spec`: the language reference (the only documentation).
- The codebase has about 1,100 definitions (31k tokens of source), so don't print all of it. Find what you need with `./sspur q find 'WORD|OTHER'` (matching names with their signatures), `./sspur q grep TEXT` (definitions whose source contains TEXT, with those lines), `./sspur q body A,B`, `./sspur q callers NAME` and `./sspur q pack A,B,C` (each definition with the signatures it uses, its tests and its callers, several at a time).
- Change code only with `./sspur edit` as the reference describes. `./sspur test` and `./sspur check` are also available.
- Do not read or write anything under .sspur/ directly, do not create .ssp files, and do not run `./sspur init`."""

PY_INTRO = """You are working on a large Python 3.9 codebase. Working directory: {dir}

- The code is the package shop/ (a common module and 44 domain modules, about 1,100 functions and classes) with tests in tests/. Run them with `cd {dir} && LC_ALL=en_US.UTF-8 {tmo} 300 python3 -m pytest -q tests`.
- The codebase is about 36k tokens, so don't read all of it. Find what you need with grep (for example `grep -rn WORD shop tests`) and read only what you need."""

COMMON = """{intro}

Task: {title}
{steps}

Required interface (hidden tests call exactly these names): {iface}

Do not look at, list, or search any directory other than {dir}. When every step is done and the tests pass, reply with the single word DONE."""


def main(root):
    gen = tempfile.mkdtemp()
    subprocess.run([sys.executable, os.path.join(HERE, "gen.py"), gen], check=True, capture_output=True)
    prompts = []
    for lang in ("sspur", "python"):
        d = os.path.abspath(os.path.join(root, lang, "s1_shop"))
        shutil.rmtree(d, ignore_errors=True)
        os.makedirs(d)
        if lang == "sspur":
            shutil.copy(os.path.join(gen, "start.ssp"), os.path.join(d, "start.ssp"))
            subprocess.run([SSPUR, "init", "start.ssp"], cwd=d, check=True, capture_output=True)
            os.remove(os.path.join(d, "start.ssp"))
            w = os.path.join(d, "sspur")
            open(w, "w").write(f'#!/bin/sh\nexec {TMO} 300 {SSPUR} "$@"\n')
            os.chmod(w, 0o755)
            intro = SSPUR_INTRO.format(dir=d)
        else:
            shutil.copytree(os.path.join(gen, "py", "shop"), os.path.join(d, "shop"))
            shutil.copytree(os.path.join(gen, "py", "tests"), os.path.join(d, "tests"))
            intro = PY_INTRO.format(dir=d, tmo=TMO)
        steps = "\n".join(f"{i + 1}. {s}" for i, s in enumerate(STEPS))
        prompts.append({"lang": lang, "task": "s1_shop", "dir": d, "prompt": COMMON.format(intro=intro, title=TITLE, steps=steps, iface=IFACE[lang], dir=d)})
    json.dump(prompts, open(os.path.join(root, "prompts.json"), "w"), indent=1)
    print(f"{len(prompts)} prompts in {root}/prompts.json")


if __name__ == "__main__":
    main(sys.argv[1])

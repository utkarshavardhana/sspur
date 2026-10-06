"""Generate the large-codebase task (s1_shop) in both languages.

  python3 gen.py OUT [--ref]   -> OUT/start.ssp (or ref.ssp) and OUT/py/{shop/*.py, tests/test_*.py}

44 domains share one template (a record type, 20 functions, 4 tests) plus a common module,
so each language has over 1,100 definitions. --ref applies the four task changes.
"""
import os, sys

DOMAINS = ["customer", "supplier", "product", "order", "invoice", "shipment", "warehouse", "refund",
           "coupon", "review", "employee", "payslip", "vehicle", "route", "ticket", "subscription",
           "plan", "cart", "wishlist", "outlet", "shelf", "batch", "recipe", "ingredient", "menu",
           "booking", "room", "guest", "venue", "artist", "album", "track", "playlist", "course",
           "lesson", "student", "exam", "project", "sprint", "release", "parcel", "rma", "invoice_line", "voucher"]
SHIPPING = ["order", "parcel", "rma"]
REGIONS = ["US", "APAC", "UK"]


def cap(d):
    return "".join(p.capitalize() for p in d.split("_"))


def params(i):
    k = 3 + i % 7
    t1 = 500 + 100 * (i % 5)
    t2 = t1 * 4
    names = [n + str(i) for n in ("alpha", "beta", "gamma")]
    rows = [(1, names[0], 2 + i % 4, 120 + 5 * i, REGIONS[i % 3]), (2, names[1], 0, 900, REGIONS[(i + 1) % 3]), (3, names[2], 7 + i % 3, 75 + i, REGIONS[(i + 2) % 3])]
    return k, t1, t2, rows


def tax_rate(r, ref):
    return {"EU": 21 if ref else 19, "US": 7, "UK": 20}.get(r, 0)


def total(rows, ref):
    return sum(q * p + q * p * tax_rate(r, ref) // 100 for _, _, q, p, r in rows)


SSP_COMMON = '''fn money(c: Int) -> Str
= {money}

fn pct(amount: Int, p: Int) -> Int
= amount * p / 100

fn tax_rate(region: Str) -> Int
= match region
  | "EU" => {eu}
  | "US" => 7
  | "UK" => 20
  | _ => 0

fn ship_fee(weight: Int{express_p}) -> Int
= {ship_body}

fn clamp(x: Int, lo: Int, hi: Int) -> Int
= max(lo, min(hi, x))

fn label_of(name: Str, id: Int) -> Str
= "{{name}} #{{id}}"

fn join_lines(xs: List[Str]) -> Str
= xs.join("\\n")

fn regions() -> List[Str]
= ["EU", "US", "UK", "APAC"]

test money_cents = money(1234) == "12.34"

test pct_rounds_down = pct(199, 10) == 19

test clamp_high = clamp(150, 0, 100) == 100
'''

SSP_DOMAIN = '''type {T} = {{id: Int, name: Str, qty: Int, price: Int, region: Str, active: Bool}}

fn {d}_new(id: Int, name: Str, qty: Int, price: Int, region: Str) -> {T}
= {T}{{id, name, qty, price, region, active: true}}

fn {d}_valid(x: {T}) -> Bool
= x.qty >= 0 and x.price >= 0 and not x.name.is_empty

fn {d}_value(x: {T}) -> Int
= x.qty * x.price

fn {d}_tax(x: {T}) -> Int
= pct({d}_value(x), tax_rate(x.region))

fn {d}_gross(x: {T}) -> Int
= {d}_value(x) + {d}_tax(x)

fn {d}_label(x: {T}) -> Str
= label_of(x.name, x.id)

fn {d}_line(x: {T}) -> Str
= {d}_label(x) + ": " + money({d}_gross(x))

fn {d}_total(xs: List[{T}]) -> Int
= xs.map({d}_gross).sum

fn {d}_count_active(xs: List[{T}]) -> Int
= xs.filter(_.active).len

fn {d}_in_region(xs: List[{T}], r: Str) -> List[{T}]
= xs.filter(_.region == r)

fn {d}_top(xs: List[{T}], n: Int) -> List[{T}]
= xs.sort_by(x => -{d}_value(x)).take(n)

fn {d}_names(xs: List[{T}]) -> List[Str]
= xs.map(_.name).sort

fn {d}_find(xs: List[{T}], id: Int) -> Opt[{T}]
= xs.find(_.id == id)

fn {d}_restock(x: {T}, n: Int) -> {T}
= x with qty := x.qty + n

fn {d}_deactivate(x: {T}) -> {T}
= x with active := false

fn {d}_discounted(x: {T}, p: Int) -> {T}
= x with price := x.price - pct(x.price, clamp(p, 0, 100))

fn {d}_report(xs: List[{T}]) -> Str
= join_lines(xs.map({d}_line))

fn {d}_score(x: {T}) -> Int
= {d}_value(x) / {k} + (if x.active then 1 else 0)

fn {d}_bucket(x: {T}) -> Str
= if {d}_value(x) >= {t2} then "large" else if {d}_value(x) >= {t1} then "medium" else "small"

fn {d}_sample() -> List[{T}]
= [{rows}]

test {d}_value_ok = {d}_value({d}_new(1, "a", 2, 300, "US")) == 600

test {d}_total_ok = {d}_total({d}_sample()) == {total}

test {d}_bucket_ok = {d}_bucket({d}_new(1, "a", 1, {t1}, "US")) == "medium"

test {d}_names_ok = {d}_names({d}_sample()) == [{names}]
'''


def ssp(ref):
    out = [SSP_COMMON.format(
        money='"{c / 100}." + (c % 100).str.pad_left(2, "0")' if ref else '"{c / 100}.{c % 100}"',
        eu=21 if ref else 19,
        express_p=", express: Bool" if ref else "",
        ship_body="if weight <= 0 then 0 else (299 + weight * 15) * (if express then 2 else 1)" if ref else "if weight <= 0 then 0 else 299 + weight * 15")]
    for i, d in enumerate(DOMAINS):
        k, t1, t2, rows = params(i)
        out.append(SSP_DOMAIN.format(T=cap(d), d=d, k=k, t1=t1, t2=t2, total=total(rows, ref),
                                     rows=", ".join(f'{d}_new({a}, "{b}", {q}, {p}, "{r}")' for a, b, q, p, r in rows),
                                     names=", ".join(f'"{r[1]}"' for r in sorted(rows, key=lambda r: r[1]))))
        if d in SHIPPING:
            arg = ", false" if ref else ""
            out.append(f"fn {d}_shipping(x: {cap(d)}) -> Int\n= ship_fee(x.qty{arg})\n")
            out.append(f'test {d}_shipping_ok = {d}_shipping({d}_new(1, "a", 2, 1, "US")) == 329\n')
    if ref:
        out.append("fn order_express_shipping(x: Order) -> Int\n= ship_fee(x.qty, true)\n")
        out.append("fn warehouse_restock_all(ws: List[Warehouse], n: Int) -> List[Warehouse]\n= ws.map(w => if w.active then warehouse_restock(w, n) else w)\n")
    return "\n".join(out)


PY_COMMON = '''from typing import List


def money(c: int) -> str:
    return {money}


def pct(amount: int, p: int) -> int:
    return amount * p // 100


def tax_rate(region: str) -> int:
    if region == "EU":
        return {eu}
    if region == "US":
        return 7
    if region == "UK":
        return 20
    return 0


def ship_fee(weight: int{express_p}) -> int:
{ship_body}


def clamp(x: int, lo: int, hi: int) -> int:
    return max(lo, min(hi, x))


def label_of(name: str, id: int) -> str:
    return f"{{name}} #{{id}}"


def join_lines(xs: List[str]) -> str:
    return "\\n".join(xs)


def regions() -> List[str]:
    return ["EU", "US", "UK", "APAC"]
'''

PY_COMMON_TEST = '''from shop.common import *


def test_money_cents():
    assert money(1234) == "12.34"


def test_pct_rounds_down():
    assert pct(199, 10) == 19


def test_clamp_high():
    assert clamp(150, 0, 100) == 100
'''

PY_DOMAIN = '''from dataclasses import dataclass, replace
from typing import List, Optional

from shop.common import clamp, join_lines, label_of, money, pct, tax_rate{extra_import}


@dataclass(frozen=True)
class {T}:
    id: int
    name: str
    qty: int
    price: int
    region: str
    active: bool


def {d}_new(id: int, name: str, qty: int, price: int, region: str) -> {T}:
    return {T}(id, name, qty, price, region, True)


def {d}_valid(x: {T}) -> bool:
    return x.qty >= 0 and x.price >= 0 and x.name != ""


def {d}_value(x: {T}) -> int:
    return x.qty * x.price


def {d}_tax(x: {T}) -> int:
    return pct({d}_value(x), tax_rate(x.region))


def {d}_gross(x: {T}) -> int:
    return {d}_value(x) + {d}_tax(x)


def {d}_label(x: {T}) -> str:
    return label_of(x.name, x.id)


def {d}_line(x: {T}) -> str:
    return {d}_label(x) + ": " + money({d}_gross(x))


def {d}_total(xs: List[{T}]) -> int:
    return sum({d}_gross(x) for x in xs)


def {d}_count_active(xs: List[{T}]) -> int:
    return len([x for x in xs if x.active])


def {d}_in_region(xs: List[{T}], r: str) -> List[{T}]:
    return [x for x in xs if x.region == r]


def {d}_top(xs: List[{T}], n: int) -> List[{T}]:
    return sorted(xs, key=lambda x: -{d}_value(x))[:n]


def {d}_names(xs: List[{T}]) -> List[str]:
    return sorted(x.name for x in xs)


def {d}_find(xs: List[{T}], id: int) -> Optional[{T}]:
    return next((x for x in xs if x.id == id), None)


def {d}_restock(x: {T}, n: int) -> {T}:
    return replace(x, qty=x.qty + n)


def {d}_deactivate(x: {T}) -> {T}:
    return replace(x, active=False)


def {d}_discounted(x: {T}, p: int) -> {T}:
    return replace(x, price=x.price - pct(x.price, clamp(p, 0, 100)))


def {d}_report(xs: List[{T}]) -> str:
    return join_lines([{d}_line(x) for x in xs])


def {d}_score(x: {T}) -> int:
    return {d}_value(x) // {k} + (1 if x.active else 0)


def {d}_bucket(x: {T}) -> str:
    if {d}_value(x) >= {t2}:
        return "large"
    if {d}_value(x) >= {t1}:
        return "medium"
    return "small"


def {d}_sample() -> List[{T}]:
    return [{rows}]
'''

PY_DOMAIN_TEST = '''from shop.{d} import *


def test_{d}_value_ok():
    assert {d}_value({d}_new(1, "a", 2, 300, "US")) == 600


def test_{d}_total_ok():
    assert {d}_total({d}_sample()) == {total}


def test_{d}_bucket_ok():
    assert {d}_bucket({d}_new(1, "a", 1, {t1}, "US")) == "medium"


def test_{d}_names_ok():
    assert {d}_names({d}_sample()) == [{names}]
'''


def py(root, ref):
    os.makedirs(os.path.join(root, "shop"), exist_ok=True)
    os.makedirs(os.path.join(root, "tests"), exist_ok=True)
    open(os.path.join(root, "shop", "__init__.py"), "w").write("")
    open(os.path.join(root, "shop", "common.py"), "w").write(PY_COMMON.format(
        money='f"{c // 100}." + str(c % 100).rjust(2, "0")' if ref else 'f"{c // 100}.{c % 100}"',
        eu=21 if ref else 19,
        express_p=", express: bool" if ref else "",
        ship_body="    if weight <= 0:\n        return 0\n    return (299 + weight * 15) * (2 if express else 1)" if ref else "    if weight <= 0:\n        return 0\n    return 299 + weight * 15"))
    open(os.path.join(root, "tests", "test_common.py"), "w").write(PY_COMMON_TEST)
    for i, d in enumerate(DOMAINS):
        k, t1, t2, rows = params(i)
        src = PY_DOMAIN.format(T=cap(d), d=d, k=k, t1=t1, t2=t2, extra_import=", ship_fee" if d in SHIPPING else "",
                               rows=", ".join(f'{d}_new({a}, "{b}", {q}, {p}, "{r}")' for a, b, q, p, r in rows))
        test = PY_DOMAIN_TEST.format(d=d, t1=t1, total=total(rows, ref), names=", ".join(f'"{r[1]}"' for r in sorted(rows, key=lambda r: r[1])))
        if d in SHIPPING:
            arg = ", False" if ref else ""
            src += f"\n\ndef {d}_shipping(x: {cap(d)}) -> int:\n    return ship_fee(x.qty{arg})\n"
            test += f'\n\ndef test_{d}_shipping_ok():\n    assert {d}_shipping({d}_new(1, "a", 2, 1, "US")) == 329\n'
        if ref and d == "order":
            src += "\n\ndef order_express_shipping(x: Order) -> int:\n    return ship_fee(x.qty, True)\n"
        if ref and d == "warehouse":
            src += "\n\ndef warehouse_restock_all(ws: List[Warehouse], n: int) -> List[Warehouse]:\n    return [warehouse_restock(w, n) if w.active else w for w in ws]\n"
        open(os.path.join(root, "shop", d + ".py"), "w").write(src)
        open(os.path.join(root, "tests", f"test_{d}.py"), "w").write(test)


if __name__ == "__main__":
    out, ref = sys.argv[1], "--ref" in sys.argv[2:]
    os.makedirs(out, exist_ok=True)
    open(os.path.join(out, "ref.ssp" if ref else "start.ssp"), "w").write(ssp(ref))
    py(os.path.join(out, "py_ref" if ref else "py"), ref)
    for name in ("customer", "route"):
        i = DOMAINS.index(name)
        print(name, "total", total(params(i)[3], ref))

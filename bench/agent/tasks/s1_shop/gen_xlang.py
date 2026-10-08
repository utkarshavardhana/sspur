"""Generate the large-codebase task (s1_shop) in TypeScript and Go, from the same template as gen.py.

  python3 gen_xlang.py OUT [--ref]   -> OUT/ts/{shop/*.ts, tests/*.test.ts} and OUT/go/{go.mod, shop/*.go}

Same 44 domains, record type, 20 functions and 4 tests per domain, common module, sample data and
expected values as gen.py; names are camelCase (customer_count_active is customerCountActive).
Go is one package, shop, with in-package tests next to each file.
"""
import os, sys

from gen import DOMAINS, SHIPPING, cap, params, total


def camel(s):
    a, *rest = s.split("_")
    return a + "".join(p.capitalize() for p in rest)


TS_COMMON = '''export function money(c: number): string {{
  return {money};
}}

export function pct(amount: number, p: number): number {{
  return Math.floor((amount * p) / 100);
}}

export function taxRate(region: string): number {{
  switch (region) {{
    case "EU":
      return {eu};
    case "US":
      return 7;
    case "UK":
      return 20;
    default:
      return 0;
  }}
}}

export function shipFee(weight: number{express_p}): number {{
{ship_body}
}}

export function clamp(x: number, lo: number, hi: number): number {{
  return Math.max(lo, Math.min(hi, x));
}}

export function labelOf(name: string, id: number): string {{
  return `${{name}} #${{id}}`;
}}

export function joinLines(xs: readonly string[]): string {{
  return xs.join("\\n");
}}

export function regions(): string[] {{
  return ["EU", "US", "UK", "APAC"];
}}
'''

TS_COMMON_TEST = '''import { test } from "node:test";
import assert from "node:assert/strict";
import { clamp, money, pct } from "../shop/common";

test("money cents", () => {
  assert.equal(money(1234), "12.34");
});

test("pct rounds down", () => {
  assert.equal(pct(199, 10), 19);
});

test("clamp high", () => {
  assert.equal(clamp(150, 0, 100), 100);
});
'''

TS_DOMAIN = '''import {{ clamp, joinLines, labelOf, money, pct, taxRate{extra_import} }} from "./common";

export interface {T} {{
  readonly id: number;
  readonly name: string;
  readonly qty: number;
  readonly price: number;
  readonly region: string;
  readonly active: boolean;
}}

export function {d}New(id: number, name: string, qty: number, price: number, region: string): {T} {{
  return {{ id, name, qty, price, region, active: true }};
}}

export function {d}Valid(x: {T}): boolean {{
  return x.qty >= 0 && x.price >= 0 && x.name !== "";
}}

export function {d}Value(x: {T}): number {{
  return x.qty * x.price;
}}

export function {d}Tax(x: {T}): number {{
  return pct({d}Value(x), taxRate(x.region));
}}

export function {d}Gross(x: {T}): number {{
  return {d}Value(x) + {d}Tax(x);
}}

export function {d}Label(x: {T}): string {{
  return labelOf(x.name, x.id);
}}

export function {d}Line(x: {T}): string {{
  return {d}Label(x) + ": " + money({d}Gross(x));
}}

export function {d}Total(xs: readonly {T}[]): number {{
  return xs.reduce((s, x) => s + {d}Gross(x), 0);
}}

export function {d}CountActive(xs: readonly {T}[]): number {{
  return xs.filter((x) => x.active).length;
}}

export function {d}InRegion(xs: readonly {T}[], r: string): {T}[] {{
  return xs.filter((x) => x.region === r);
}}

export function {d}Top(xs: readonly {T}[], n: number): {T}[] {{
  return [...xs].sort((a, b) => {d}Value(b) - {d}Value(a)).slice(0, n);
}}

export function {d}Names(xs: readonly {T}[]): string[] {{
  return xs.map((x) => x.name).sort();
}}

export function {d}Find(xs: readonly {T}[], id: number): {T} | undefined {{
  return xs.find((x) => x.id === id);
}}

export function {d}Restock(x: {T}, n: number): {T} {{
  return {{ ...x, qty: x.qty + n }};
}}

export function {d}Deactivate(x: {T}): {T} {{
  return {{ ...x, active: false }};
}}

export function {d}Discounted(x: {T}, p: number): {T} {{
  return {{ ...x, price: x.price - pct(x.price, clamp(p, 0, 100)) }};
}}

export function {d}Report(xs: readonly {T}[]): string {{
  return joinLines(xs.map({d}Line));
}}

export function {d}Score(x: {T}): number {{
  return Math.floor({d}Value(x) / {k}) + (x.active ? 1 : 0);
}}

export function {d}Bucket(x: {T}): string {{
  if ({d}Value(x) >= {t2}) return "large";
  if ({d}Value(x) >= {t1}) return "medium";
  return "small";
}}

export function {d}Sample(): {T}[] {{
  return [{rows}];
}}
'''

TS_DOMAIN_TEST = '''import {{ test }} from "node:test";
import assert from "node:assert/strict";
import {{ {imports} }} from "../shop/{file}";

test("{d} value ok", () => {{
  assert.equal({d}Value({d}New(1, "a", 2, 300, "US")), 600);
}});

test("{d} total ok", () => {{
  assert.equal({d}Total({d}Sample()), {total});
}});

test("{d} bucket ok", () => {{
  assert.equal({d}Bucket({d}New(1, "a", 1, {t1}, "US")), "medium");
}});

test("{d} names ok", () => {{
  assert.deepEqual({d}Names({d}Sample()), [{names}]);
}});
'''

TS_TSCONFIG = """{
  "compilerOptions": {
    "target": "ES2022",
    "module": "commonjs",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "types": ["node"],
    "rootDir": ".",
    "outDir": "dist"
  },
  "include": ["shop/*.ts", "tests/*.ts"]
}
"""

GO_COMMON = '''package shop

import (
	"fmt"
	"strings"
)

func money(c int) string {{
	return {money}
}}

func pct(amount, p int) int {{
	return amount * p / 100
}}

func taxRate(region string) int {{
	switch region {{
	case "EU":
		return {eu}
	case "US":
		return 7
	case "UK":
		return 20
	}}
	return 0
}}

func shipFee(weight int{express_p}) int {{
{ship_body}
}}

func clamp(x, lo, hi int) int {{
	return max(lo, min(hi, x))
}}

func labelOf(name string, id int) string {{
	return fmt.Sprintf("%s #%d", name, id)
}}

func joinLines(xs []string) string {{
	return strings.Join(xs, "\\n")
}}

func regions() []string {{
	return []string{{"EU", "US", "UK", "APAC"}}
}}
'''

GO_COMMON_TEST = '''package shop

import "testing"

func TestMoneyCents(t *testing.T) {
	if got := money(1234); got != "12.34" {
		t.Fatalf("got %q", got)
	}
}

func TestPctRoundsDown(t *testing.T) {
	if got := pct(199, 10); got != 19 {
		t.Fatalf("got %d", got)
	}
}

func TestClampHigh(t *testing.T) {
	if got := clamp(150, 0, 100); got != 100 {
		t.Fatalf("got %d", got)
	}
}
'''

GO_DOMAIN = '''package shop

import "sort"

type {T} struct {{
	id     int
	name   string
	qty    int
	price  int
	region string
	active bool
}}

func {d}New(id int, name string, qty int, price int, region string) {T} {{
	return {T}{{id, name, qty, price, region, true}}
}}

func {d}Valid(x {T}) bool {{
	return x.qty >= 0 && x.price >= 0 && x.name != ""
}}

func {d}Value(x {T}) int {{
	return x.qty * x.price
}}

func {d}Tax(x {T}) int {{
	return pct({d}Value(x), taxRate(x.region))
}}

func {d}Gross(x {T}) int {{
	return {d}Value(x) + {d}Tax(x)
}}

func {d}Label(x {T}) string {{
	return labelOf(x.name, x.id)
}}

func {d}Line(x {T}) string {{
	return {d}Label(x) + ": " + money({d}Gross(x))
}}

func {d}Total(xs []{T}) int {{
	s := 0
	for _, x := range xs {{
		s += {d}Gross(x)
	}}
	return s
}}

func {d}CountActive(xs []{T}) int {{
	n := 0
	for _, x := range xs {{
		if x.active {{
			n++
		}}
	}}
	return n
}}

func {d}InRegion(xs []{T}, r string) []{T} {{
	out := []{T}{{}}
	for _, x := range xs {{
		if x.region == r {{
			out = append(out, x)
		}}
	}}
	return out
}}

func {d}Top(xs []{T}, n int) []{T} {{
	out := append([]{T}{{}}, xs...)
	sort.SliceStable(out, func(i, j int) bool {{ return {d}Value(out[i]) > {d}Value(out[j]) }})
	return out[:min(n, len(out))]
}}

func {d}Names(xs []{T}) []string {{
	out := []string{{}}
	for _, x := range xs {{
		out = append(out, x.name)
	}}
	sort.Strings(out)
	return out
}}

func {d}Find(xs []{T}, id int) ({T}, bool) {{
	for _, x := range xs {{
		if x.id == id {{
			return x, true
		}}
	}}
	return {T}{{}}, false
}}

func {d}Restock(x {T}, n int) {T} {{
	x.qty += n
	return x
}}

func {d}Deactivate(x {T}) {T} {{
	x.active = false
	return x
}}

func {d}Discounted(x {T}, p int) {T} {{
	x.price -= pct(x.price, clamp(p, 0, 100))
	return x
}}

func {d}Report(xs []{T}) string {{
	lines := []string{{}}
	for _, x := range xs {{
		lines = append(lines, {d}Line(x))
	}}
	return joinLines(lines)
}}

func {d}Score(x {T}) int {{
	s := {d}Value(x) / {k}
	if x.active {{
		s++
	}}
	return s
}}

func {d}Bucket(x {T}) string {{
	if {d}Value(x) >= {t2} {{
		return "large"
	}}
	if {d}Value(x) >= {t1} {{
		return "medium"
	}}
	return "small"
}}

func {d}Sample() []{T} {{
	return []{T}{{{rows}}}
}}
'''

GO_DOMAIN_TEST = '''package shop

import (
	"reflect"
	"testing"
)

func Test{T}ValueOk(t *testing.T) {{
	if got := {d}Value({d}New(1, "a", 2, 300, "US")); got != 600 {{
		t.Fatalf("got %d", got)
	}}
}}

func Test{T}TotalOk(t *testing.T) {{
	if got := {d}Total({d}Sample()); got != {total} {{
		t.Fatalf("got %d", got)
	}}
}}

func Test{T}BucketOk(t *testing.T) {{
	if got := {d}Bucket({d}New(1, "a", 1, {t1}, "US")); got != "medium" {{
		t.Fatalf("got %q", got)
	}}
}}

func Test{T}NamesOk(t *testing.T) {{
	if got := {d}Names({d}Sample()); !reflect.DeepEqual(got, []string{{{names}}}) {{
		t.Fatalf("got %v", got)
	}}
}}
'''


def ts(root, ref):
    os.makedirs(os.path.join(root, "shop"), exist_ok=True)
    os.makedirs(os.path.join(root, "tests"), exist_ok=True)
    open(os.path.join(root, "tsconfig.json"), "w").write(TS_TSCONFIG)
    open(os.path.join(root, "shop", "common.ts"), "w").write(TS_COMMON.format(
        money='`${Math.floor(c / 100)}.${String(c % 100).padStart(2, "0")}`' if ref else '`${Math.floor(c / 100)}.${c % 100}`',
        eu=21 if ref else 19,
        express_p=", express: boolean" if ref else "",
        ship_body="  if (weight <= 0) return 0;\n  return (299 + weight * 15) * (express ? 2 : 1);" if ref else "  if (weight <= 0) return 0;\n  return 299 + weight * 15;"))
    open(os.path.join(root, "tests", "common.test.ts"), "w").write(TS_COMMON_TEST)
    for i, d in enumerate(DOMAINS):
        k, t1, t2, rows = params(i)
        c = camel(d)
        src = TS_DOMAIN.format(T=cap(d), d=c, k=k, t1=t1, t2=t2, extra_import=", shipFee" if d in SHIPPING else "",
                               rows=", ".join(f'{c}New({a}, "{b}", {q}, {p}, "{r}")' for a, b, q, p, r in rows))
        imports = [f"{c}Bucket", f"{c}New", f"{c}Names", f"{c}Sample", f"{c}Total", f"{c}Value"]
        if d in SHIPPING:
            imports.append(f"{c}Shipping")
        test = TS_DOMAIN_TEST.format(d=c, file=d, imports=", ".join(sorted(imports)), t1=t1, total=total(rows, ref),
                                     names=", ".join(f'"{r[1]}"' for r in sorted(rows, key=lambda r: r[1])))
        if d in SHIPPING:
            arg = ", false" if ref else ""
            src += f"\nexport function {c}Shipping(x: {cap(d)}): number {{\n  return shipFee(x.qty{arg});\n}}\n"
            test += f'\ntest("{c} shipping ok", () => {{\n  assert.equal({c}Shipping({c}New(1, "a", 2, 1, "US")), 329);\n}});\n'
        if ref and d == "order":
            src += "\nexport function orderExpressShipping(x: Order): number {\n  return shipFee(x.qty, true);\n}\n"
        if ref and d == "warehouse":
            src += "\nexport function warehouseRestockAll(ws: readonly Warehouse[], n: number): Warehouse[] {\n  return ws.map((w) => (w.active ? warehouseRestock(w, n) : w));\n}\n"
        open(os.path.join(root, "shop", d + ".ts"), "w").write(src)
        open(os.path.join(root, "tests", f"{d}.test.ts"), "w").write(test)


def go(root, ref):
    os.makedirs(os.path.join(root, "shop"), exist_ok=True)
    open(os.path.join(root, "go.mod"), "w").write("module app\n\ngo 1.22\n")
    open(os.path.join(root, "shop", "common.go"), "w").write(GO_COMMON.format(
        money='fmt.Sprintf("%d.%02d", c/100, c%100)' if ref else 'fmt.Sprintf("%d.%d", c/100, c%100)',
        eu=21 if ref else 19,
        express_p=", express bool" if ref else "",
        ship_body="\tif weight <= 0 {\n\t\treturn 0\n\t}\n\tif express {\n\t\treturn (299 + weight*15) * 2\n\t}\n\treturn 299 + weight*15" if ref else "\tif weight <= 0 {\n\t\treturn 0\n\t}\n\treturn 299 + weight*15"))
    open(os.path.join(root, "shop", "common_test.go"), "w").write(GO_COMMON_TEST)
    for i, d in enumerate(DOMAINS):
        k, t1, t2, rows = params(i)
        c = camel(d)
        src = GO_DOMAIN.format(T=cap(d), d=c, k=k, t1=t1, t2=t2,
                               rows=", ".join(f'{c}New({a}, "{b}", {q}, {p}, "{r}")' for a, b, q, p, r in rows))
        test = GO_DOMAIN_TEST.format(T=cap(d), d=c, t1=t1, total=total(rows, ref),
                                     names=", ".join(f'"{r[1]}"' for r in sorted(rows, key=lambda r: r[1])))
        if d in SHIPPING:
            arg = ", false" if ref else ""
            src += f"\nfunc {c}Shipping(x {cap(d)}) int {{\n\treturn shipFee(x.qty{arg})\n}}\n"
            test += f'\nfunc Test{cap(d)}ShippingOk(t *testing.T) {{\n\tif got := {c}Shipping({c}New(1, "a", 2, 1, "US")); got != 329 {{\n\t\tt.Fatalf("got %d", got)\n\t}}\n}}\n'
        if ref and d == "order":
            src += "\nfunc orderExpressShipping(x Order) int {\n\treturn shipFee(x.qty, true)\n}\n"
        if ref and d == "warehouse":
            src += "\nfunc warehouseRestockAll(ws []Warehouse, n int) []Warehouse {\n\tout := []Warehouse{}\n\tfor _, w := range ws {\n\t\tif w.active {\n\t\t\tw = warehouseRestock(w, n)\n\t\t}\n\t\tout = append(out, w)\n\t}\n\treturn out\n}\n"
        open(os.path.join(root, "shop", d + ".go"), "w").write(src)
        open(os.path.join(root, "shop", f"{d}_test.go"), "w").write(test)


if __name__ == "__main__":
    out, ref = sys.argv[1], "--ref" in sys.argv[2:]
    ts(os.path.join(out, "ts_ref" if ref else "ts"), ref)
    go(os.path.join(out, "go_ref" if ref else "go"), ref)

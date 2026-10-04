import sys

n = int(sys.argv[1]) if len(sys.argv) > 1 else 200
edit = int(sys.argv[2]) if len(sys.argv) > 2 else 0
out = ["type Item = {sku: Int, qty: Int where _ > 0, price: Int where _ >= 0}",
       "type Tree = Leaf | Node{left: Tree, value: Int, right: Tree}", ""]
for i in range(n):
    k = i % 5
    c = 3 + edit if i == n // 2 else 3
    if k == 0:
        out.append(f"fn f{i}(n: Int) -> Int\n= (0..n).map(x => x * {c} + {i}).filter(_ % 2 == 0).sum\n")
    elif k == 1:
        out.append(f"fn f{i}(n: Int) -> Str\n= (0..n).map(x => \"w{{x % {c + 4}}}\").join(\",\").upper\n")
    elif k == 2:
        out.append(f"fn f{i}(n: Int) -> Int\n= (0..n).map(j => Item{{sku: j, qty: j % {c + 4} + 1, price: j % 13}}).filter(_.price > 3).map(_.price * _.qty).sum\n")
    elif k == 3:
        out.append(f"fn ins{i}(t: Tree, x: Int) -> Tree\n= match t\n  | Leaf => Node{{left: Leaf, value: x, right: Leaf}}\n  | Node{{left, value, right}} => if x < value then Node{{left: ins{i}(left, x), value, right}} else if x > value then Node{{left, value, right: ins{i}(right, x)}} else t\n")
        out.append(f"fn f{i}(n: Int) -> Int\n= do\n  var t = Leaf\n  for j in 0..n\n    t := ins{i}(t, (j * {c + 4}) % 101)\n  match t\n    | Leaf => 0\n    | Node{{value}} => value\n")
    else:
        prev = f"f{i - 4}(n)" if i >= 4 else "n"
        out.append(f"fn f{i}(n: Int) -> Int\n= do\n  var m = empty_map()\n  for j in 0..n\n    m := m.put(j % {c + 7}, j)\n  m.len + {prev}\n")
for i in range(n):
    out.append(f"test t{i} = f{i}(10) == f{i}(10)")
print("\n".join(out))

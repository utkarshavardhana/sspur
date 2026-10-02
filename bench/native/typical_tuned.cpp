#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <memory>
#include <vector>

static inline int64_t add(int64_t a, int64_t b) { int64_t r; if (__builtin_add_overflow(a, b, &r)) abort(); return r; }
static inline int64_t mul(int64_t a, int64_t b) { int64_t r; if (__builtin_mul_overflow(a, b, &r)) abort(); return r; }

struct Item { int64_t sku, qty, price; };

std::vector<Item> make_items(int64_t n) {
    std::vector<Item> v;
    v.reserve(n);
    for (int64_t i = 0; i < n; i++) {
        Item it{i, i % 7 + 1, i % 13};
        if (!(it.qty > 0) || !(it.price >= 0)) abort();
        v.push_back(it);
    }
    return v;
}

int64_t order_total(const std::vector<Item>& items) {
    int64_t s = 0;
    for (const auto& i : items) if (i.price > 3) s = add(s, mul(i.price, i.qty));
    return s;
}

struct Node {
    Node* left;
    Node* right;
    int64_t value;
};
using Tree = Node*;

static char* arena_ptr = nullptr;
static size_t arena_left = 0;
static Node* alloc_node(Node n) {
    if (arena_left < sizeof(Node)) { arena_ptr = (char*)malloc(1 << 20); arena_left = 1 << 20; }
    Node* p = (Node*)arena_ptr; arena_ptr += sizeof(Node); arena_left -= sizeof(Node);
    *p = n;
    return p;
}

Tree insert(Tree t, int64_t x) {
    if (!t) return alloc_node(Node{nullptr, nullptr, x});
    if (x < t->value) return alloc_node(Node{insert(t->left, x), t->right, t->value});
    if (x > t->value) return alloc_node(Node{t->left, insert(t->right, x), t->value});
    return t;
}

int64_t tree_sum(Tree t) { return t ? add(add(tree_sum(t->left), t->value), tree_sum(t->right)) : 0; }

int64_t tree_bench(int64_t n) {
    Tree t = nullptr;
    int64_t k = 7;
    for (int64_t i = 0; i < n; i++) {
        k = add(mul(k, 1103515245), 12345) % 2147483648LL;
        t = insert(t, k % 1000000);
    }
    return tree_sum(t);
}

struct P { double x, y, vx, vy; };

double sim_bench(int64_t n, int64_t steps) {
    std::vector<P> ps;
    for (int64_t i = 0; i < n; i++) ps.push_back({std::fmod((double)i, 100.0), 0.0, 1.5, 2.0});
    for (int64_t s = 0; s < steps; s++) {
        std::vector<P> next;
        next.reserve(ps.size());
        for (const auto& p : ps) next.push_back({p.x + p.vx * 0.01, p.y + p.vy * 0.01, (p.x > 100.0 || p.x < 0.0) ? -p.vx : p.vx, p.vy - 9.8 * 0.01});
        ps = std::move(next);
    }
    double t = 0;
    for (const auto& p : ps) t += p.x + p.y;
    return t;
}

int main() {
    printf("orders %lld\n", (long long)order_total(make_items(3000000)));
    printf("tree %lld\n", (long long)tree_bench(400000));
    printf("sim %.17g\n", sim_bench(20000, 400));
}

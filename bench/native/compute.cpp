#include <cstdint>
#include <cstdio>
#include <cstdlib>

static inline int64_t add(int64_t a, int64_t b) { int64_t r; if (__builtin_add_overflow(a, b, &r)) abort(); return r; }
static inline int64_t mul(int64_t a, int64_t b) { int64_t r; if (__builtin_mul_overflow(a, b, &r)) abort(); return r; }
static inline int64_t sub(int64_t a, int64_t b) { int64_t r; if (__builtin_sub_overflow(a, b, &r)) abort(); return r; }
static inline int64_t rem(int64_t a, int64_t b) { if (b == 0) abort(); return a % b; }
static inline int64_t dv(int64_t a, int64_t b) { if (b == 0) abort(); return a / b; }

int64_t fib(int64_t n) { if (n < 0) abort(); return n < 2 ? n : add(fib(sub(n, 1)), fib(sub(n, 2))); }

int64_t collatz_len(int64_t n) {
    if (n <= 0) abort();
    int64_t x = n, steps = 0;
    while (x != 1) { x = rem(x, 2) == 0 ? dv(x, 2) : add(mul(3, x), 1); steps = add(steps, 1); }
    return steps;
}

int64_t longest_collatz(int64_t limit) {
    int64_t best = 0, arg = 1;
    for (int64_t i = 1; i < limit; i++) { int64_t l = collatz_len(i); if (l > best) { best = l; arg = i; } }
    return arg;
}

bool is_prime(int64_t n) {
    int64_t d = 2; bool ok = n >= 2;
    while (ok && mul(d, d) <= n) { if (rem(n, d) == 0) ok = false; d = add(d, 1); }
    return ok;
}

int64_t count_primes(int64_t limit) { int64_t c = 0; for (int64_t i = 0; i < limit; i++) if (is_prime(i)) c = add(c, 1); return c; }

int64_t gcd(int64_t a, int64_t b) { if (a < 0 || b < 0) abort(); return b == 0 ? a : gcd(b, rem(a, b)); }

int64_t gcd_sum(int64_t n) {
    int64_t s = 0;
    for (int64_t i = 1; i < n; i++) for (int64_t j = 1; j < n; j++) s = add(s, gcd(i, j));
    if (s < 0) abort();
    return s;
}

int main() {
    printf("fib(30) = %lld\n", (long long)fib(30));
    printf("longest collatz below 300000 starts at %lld\n", (long long)longest_collatz(300000));
    printf("primes below 200000: %lld\n", (long long)count_primes(200000));
    printf("gcd sum 600: %lld\n", (long long)gcd_sum(600));
}

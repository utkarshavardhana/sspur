#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <vector>

static inline int64_t add(int64_t a, int64_t b) { int64_t r; if (__builtin_add_overflow(a, b, &r)) abort(); return r; }
static inline int64_t mul(int64_t a, int64_t b) { int64_t r; if (__builtin_mul_overflow(a, b, &r)) abort(); return r; }
static inline int64_t rem(int64_t a, int64_t b) { if (b == 0) abort(); return a % b; }
static inline int64_t dv(int64_t a, int64_t b) { if (b == 0) abort(); return a / b; }

int64_t collatz_len(int64_t n) {
    if (n <= 0) abort();
    int64_t x = n, steps = 0;
    while (x != 1) { x = rem(x, 2) == 0 ? dv(x, 2) : add(mul(3, x), 1); steps = add(steps, 1); }
    return steps;
}

bool is_prime(int64_t n) {
    int64_t d = 2; bool ok = n >= 2;
    while (ok && mul(d, d) <= n) { if (rem(n, d) == 0) ok = false; d = add(d, 1); }
    return ok;
}

int64_t mix(int64_t x) {
    int64_t h = x;
    for (int64_t r = 0; r < 64; r++) h = rem(add(mul(h, 31), r), 1000003);
    return h;
}

int64_t escape(int64_t i, int64_t w) {
    double cx = (double)rem(i, w) * 3.0 / (double)w - 2.0;
    double cy = (double)dv(i, w) * 2.0 / (double)w - 1.0;
    double x = 0.0, y = 0.0;
    int64_t k = 0;
    for (;;) {
        double xx = x * x, yy = y * y;
        if (!(k < 200 && xx + yy <= 4.0)) break;
        double t = xx - yy;
        t = t + cx;
        double xy = 2.0 * x * y;
        y = xy + cy;
        x = t;
        k = add(k, 1);
    }
    return k;
}

int main() {
    int64_t s = 0;
    for (int64_t i = 1; i < 3000000; i++) s = add(s, collatz_len(i));
    printf("collatz steps below 3000000: %lld\n", (long long)s);
    int64_t c = 0;
    for (int64_t i = 0; i < 4000000; i++) if (is_prime(i)) c++;
    printf("primes below 4000000: %lld\n", (long long)c);
    int64_t ps = 0;
    for (int64_t i = 0; i < 2000000; i++) if (is_prime(i)) ps = add(ps, i);
    printf("sum of primes below 2000000: %lld\n", (long long)ps);
    std::vector<int64_t> hs(2000000);
    for (int64_t i = 0; i < 2000000; i++) hs[i] = mix(i);
    int64_t hsum = 0;
    for (int64_t h : hs) hsum = add(hsum, h);
    printf("mixed %lld values, checksum %lld\n", (long long)hs.size(), (long long)hsum);
    int64_t m = 0;
    for (int64_t i = 0; i < 1000000; i++) m = add(m, escape(i, 1000));
    printf("mandelbrot 1000x1000 iterations: %lld\n", (long long)m);
}

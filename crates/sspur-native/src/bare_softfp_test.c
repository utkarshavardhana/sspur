#include <stdint.h>
#include <string.h>
#include <stdio.h>
#include <math.h>
#include "bare_softfp.c"
static uint64_t st = 0x9E3779B97F4A7C15ULL;
static uint64_t rnd(void) { uint64_t z = (st += 0x9E3779B97F4A7C15ULL); z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9ULL; z = (z ^ (z >> 27)) * 0x94D049BB133111EBULL; return z ^ (z >> 31); }
static const uint64_t special[] = {0, SF_SIGN, 0x7FF0000000000000ULL, 0xFFF0000000000000ULL, SF_QNAN, 1, 0x000FFFFFFFFFFFFFULL, 0x0010000000000000ULL, 0x3FF0000000000000ULL, 0xBFF0000000000000ULL, 0x7FEFFFFFFFFFFFFFULL, 0x3FE0000000000000ULL, 0x4340000000000000ULL, 0x43E0000000000000ULL, 0xC3E0000000000000ULL, 0x3CA0000000000000ULL};
static double pick(void) {
    uint64_t r = rnd();
    switch (r % 8) {
    case 0: return sf_d(special[(r >> 8) % (sizeof special / 8)]);
    case 1: return sf_d(rnd() & 0x800FFFFFFFFFFFFFULL);
    case 2: return (double)(int64_t)(rnd() >> (r >> 58));
    case 3: return sf_d((rnd() & 0x800FFFFFFFFFFFFFULL) | ((uint64_t)(0x3F0 + (r >> 20) % 32) << 52));
    default: return sf_d(rnd());
    }
}
static int same(double x, sf_u64 y) { sf_u64 b = sf_b(x); if (sf_isnan(b) || sf_isnan(y)) return sf_isnan(b) && sf_isnan(y); return b == y; }
int main(void) {
    long n = 0;
    for (long i = 0; i < 3000000; i++) {
        double a = pick(), b = pick();
        if (i % 4 == 0) b = sf_d(sf_b(a) + (rnd() % 5) - 2);
        sf_u64 x = sf_b(a), y = sf_b(b);
        if (!same(a + b, sf_add(x, y))) { printf("add %016llx %016llx\n", x, y); return 1; }
        if (!same(a - b, sf_sub(x, y))) { printf("sub %016llx %016llx\n", x, y); return 1; }
        if (!same(a * b, sf_mul(x, y))) { printf("mul %016llx %016llx\n", x, y); return 1; }
        if (!same(a / b, sf_div(x, y))) { printf("div %016llx %016llx\n", x, y); return 1; }
        if (!same(sqrt(a), sf_sqrt(x))) { printf("sqrt %016llx\n", x); return 1; }
        if ((a < b) != __aeabi_dcmplt(a, b) || (a <= b) != __aeabi_dcmple(a, b) || (a == b) != __aeabi_dcmpeq(a, b) || (a > b) != __aeabi_dcmpgt(a, b) || (a >= b) != __aeabi_dcmpge(a, b) || (a != a || b != b) != __aeabi_dcmpun(a, b)) { printf("cmp %016llx %016llx\n", x, y); return 1; }
        int64_t k = (int64_t)rnd() >> (rnd() % 64);
        if (!same((double)k, sf_b(__aeabi_l2d(k)))) { printf("l2d %lld\n", (long long)k); return 1; }
        uint64_t u = rnd() >> (rnd() % 64);
        if (!same((double)u, sf_b(__aeabi_ul2d(u)))) { printf("ul2d %llu\n", (unsigned long long)u); return 1; }
        if (!same((double)(int32_t)k, sf_b(__aeabi_i2d((int32_t)k)))) { printf("i2d\n"); return 1; }
        if (a == a && fabs(a) < 9.2e18 && (int64_t)a != __aeabi_d2lz(a)) { printf("d2lz %016llx\n", x); return 1; }
        if (a == a && fabs(a) < 2.1e9 && (int32_t)a != __aeabi_d2iz(a)) { printf("d2iz %016llx\n", x); return 1; }
        n++;
    }
    printf("ok %ld\n", n);
    return 0;
}

#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <vector>

static inline int64_t add(int64_t a, int64_t b) { int64_t r; if (__builtin_add_overflow(a, b, &r)) abort(); return r; }
static inline int64_t sub(int64_t a, int64_t b) { int64_t r; if (__builtin_sub_overflow(a, b, &r)) abort(); return r; }
static inline int64_t mul(int64_t a, int64_t b) { int64_t r; if (__builtin_mul_overflow(a, b, &r)) abort(); return r; }
static inline int64_t f2i(double x) { return x != x ? 0 : x >= 9223372036854775807.0 ? INT64_MAX : x <= -9223372036854775808.0 ? INT64_MIN : (int64_t)x; }
static inline double at(const std::vector<double>& v, int64_t i) { if (i < 0 || i >= (int64_t)v.size()) abort(); return v[i]; }

std::vector<int64_t> ints(int64_t n) {
    std::vector<int64_t> v(n);
    for (int64_t i = 0; i < n; i++) v[i] = (i * 7919) % 10007 - 5000;
    return v;
}

std::vector<double> floats(int64_t n, int64_t k) {
    std::vector<double> v(n);
    for (int64_t i = 0; i < n; i++) v[i] = (double)((i * k) % 1009) / 128.0 - 3.5;
    return v;
}

int64_t affine_pos(const std::vector<int64_t>& xs, int64_t r) {
    int64_t s = 0;
    for (int64_t x : xs) { int64_t y = add(mul(3, x), r); if (y > 0) s = add(s, y); }
    return s;
}

int64_t sq_sum(const std::vector<int64_t>& xs, int64_t r) {
    int64_t s = 0;
    for (int64_t x : xs) s = add(s, mul(add(x, r), sub(x, r)));
    return s;
}

int64_t count_over(const std::vector<int64_t>& xs, int64_t t) {
    int64_t c = 0;
    for (int64_t x : xs) if (x > t) c++;
    return c;
}

std::vector<int64_t> shifted(const std::vector<int64_t>& xs, int64_t r) {
    std::vector<int64_t> out(xs.size());
    for (size_t i = 0; i < xs.size(); i++) out[i] = sub(mul(xs[i], 5), r);
    return out;
}

double dot(const std::vector<double>& a, const std::vector<double>& b) {
    double s = 0.0;
    for (int64_t i = 0; i < (int64_t)a.size(); i++) s += a[i] * at(b, i);
    return s;
}

std::vector<double> saxpy(double k, const std::vector<double>& a, const std::vector<double>& b) {
    std::vector<double> out(a.size());
    for (int64_t i = 0; i < (int64_t)a.size(); i++) out[i] = k * a[i] + at(b, i);
    return out;
}

int main() {
    std::vector<int64_t> xs = ints(1000000);
    std::vector<double> a = floats(1000000, 31), b = floats(1000000, 17);
    int64_t s1 = 0, s2 = 0, s3 = 0, s4 = 0;
    double d = 0.0, y = 0.0;
    for (int64_t r = 0; r < 40; r++) {
        s1 = add(s1, affine_pos(xs, r));
        s2 = add(s2, sq_sum(xs, r));
        s3 = add(s3, count_over(xs, r * 50 - 1000));
        std::vector<int64_t> sh = shifted(xs, r);
        int64_t t = 0;
        for (int64_t v : sh) t = add(t, v);
        s4 = add(s4, t);
        d = d + dot(a, b);
        std::vector<double> sx = saxpy((double)r * 0.25, a, b);
        double u = 0.0;
        for (double v : sx) u += v;
        y = y + u;
    }
    printf("affine %lld squares %lld count %lld shifted %lld\n", (long long)s1, (long long)s2, (long long)s3, (long long)s4);
    printf("dot %lld saxpy %lld\n", (long long)f2i(std::round(d * 1000.0)), (long long)f2i(std::round(y * 1000.0)));
}

#include <charconv>
#include <chrono>
#include <string>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <vector>

static inline int64_t add(int64_t a, int64_t b) { int64_t r; if (__builtin_add_overflow(a, b, &r)) abort(); return r; }
static inline int64_t mul(int64_t a, int64_t b) { int64_t r; if (__builtin_mul_overflow(a, b, &r)) abort(); return r; }
static inline float at(const std::vector<float>& v, int64_t i) { if (i < 0 || i >= (int64_t)v.size()) abort(); return v[i]; }
static inline float& at(std::vector<float>& v, int64_t i) { if (i < 0 || i >= (int64_t)v.size()) abort(); return v[i]; }

static void saxpy(float a, const std::vector<float>& x, std::vector<float>& y) {
    for (int64_t i = 0; i < (int64_t)y.size(); i++) at(y, i) = a * at(x, i) + at(y, i);
}

static void partial_sums(const std::vector<float>& x, std::vector<float>& out) {
    int64_t w = (int64_t)out.size(), chunks = (int64_t)x.size() / w;
    for (int64_t g = 0; g < w; g++) at(out, g) = 0.0f;
    for (int64_t k = 0; k < chunks; k++)
        for (int64_t g = 0; g < w; g++) at(out, g) = at(out, g) + at(x, add(g, mul(k, w)));
}

static void matmul(const std::vector<float>& a, const std::vector<float>& b, std::vector<float>& c, int64_t n) {
    for (int64_t i = 0; i < n * n; i++) at(c, i) = 0.0f;
    for (int64_t i = 0; i < n; i++)
        for (int64_t k = 0; k < n; k++) {
            float aik = at(a, add(mul(i, n), k));
            for (int64_t j = 0; j < n; j++) at(c, add(mul(i, n), j)) = at(c, add(mul(i, n), j)) + aik * at(b, add(mul(k, n), j));
        }
}

static double sum(const std::vector<float>& v) {
    double s = -0.0;
    for (float x : v) s += (double)x;
    return s;
}

static std::string f(double x) {
    char buf[64];
    auto r = std::to_chars(buf, buf + sizeof buf, x, std::chars_format::fixed);
    std::string s(buf, r.ptr);
    if (s.find('.') == std::string::npos) s += ".0";
    return s;
}

static int64_t ms(std::chrono::steady_clock::time_point a, std::chrono::steady_clock::time_point b) {
    return std::chrono::duration_cast<std::chrono::milliseconds>(b - a).count();
}

int main() {
    const int64_t n = 4194304;
    auto t0 = std::chrono::steady_clock::now();
    std::vector<float> x(n), y(n);
    for (int64_t i = 0; i < n; i++) { x[i] = (float)((double)(i % 1000) * 0.001); y[i] = (float)(double)(i % 7); }
    auto t1 = std::chrono::steady_clock::now();
    for (int r = 0; r < 100; r++) saxpy(0.5f, x, y);
    printf("saxpy %s\n", f(sum(y)).c_str());
    auto t2 = std::chrono::steady_clock::now();
    std::vector<float> parts(65536);
    double total = 0.0;
    for (int r = 0; r < 100; r++) {
        partial_sums(y, parts);
        total = total + sum(parts);
    }
    printf("reduce %s\n", f(total).c_str());
    auto t3 = std::chrono::steady_clock::now();
    const int64_t m = 512;
    std::vector<float> a(m * m), b(m * m), c(m * m);
    for (int64_t i = 0; i < m * m; i++) { a[i] = (float)((double)(i % 17) * 0.125 - 1.0); b[i] = (float)((double)(i % 13) * 0.25 - 1.5); }
    for (int r = 0; r < 20; r++) matmul(a, b, c, m);
    printf("matmul %s %s %s\n", f(sum(c)).c_str(), f(c[0]).c_str(), f(c[m * m - 1]).c_str());
    auto t4 = std::chrono::steady_clock::now();
    fprintf(stderr, "setup %lld ms, saxpy %lld ms, reduce %lld ms, matmul %lld ms\n", (long long)ms(t0, t1), (long long)ms(t1, t2), (long long)ms(t2, t3), (long long)ms(t3, t4));
}

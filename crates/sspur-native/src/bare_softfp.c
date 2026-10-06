/* IEEE 754 binary64 in software for cores whose FPU is single precision (Cortex-M4).
   Round to nearest even, subnormals, infinities and quiet NaNs, following Berkeley
   SoftFloat 3's rounding and packing; division and square root are bit-serial. */
#ifdef __arm__
#define SS_AAPCS __attribute__((pcs("aapcs")))
#else
#define SS_AAPCS
#endif
typedef unsigned long long sf_u64;
typedef long long sf_i64;
#define SF_SIGN 0x8000000000000000ULL
#define SF_MAG 0x7FFFFFFFFFFFFFFFULL
#define SF_QNAN 0x7FF8000000000000ULL
static sf_u64 sf_b(double x) { sf_u64 b; memcpy(&b, &x, 8); return b; }
static double sf_d(sf_u64 b) { double x; memcpy(&x, &b, 8); return x; }
static int sf_clz(sf_u64 a) { return a >> 32 ? __builtin_clz((uint32_t)(a >> 32)) : (uint32_t)a ? 32 + __builtin_clz((uint32_t)a) : 64; }
static int sf_exp(sf_u64 a) { return (int)((a >> 52) & 0x7FF); }
static sf_u64 sf_frac(sf_u64 a) { return a & 0x000FFFFFFFFFFFFFULL; }
static sf_u64 sf_pack(int sign, int exp, sf_u64 sig) { return ((sf_u64)sign << 63) + ((sf_u64)exp << 52) + sig; }
static int sf_isnan(sf_u64 a) { return (a & SF_MAG) > 0x7FF0000000000000ULL; }
static sf_u64 sf_nan(sf_u64 a, sf_u64 b) { return sf_isnan(a) ? a | 0x0008000000000000ULL : sf_isnan(b) ? b | 0x0008000000000000ULL : SF_QNAN; }
static sf_u64 sf_jam(sf_u64 a, int dist) { return dist < 63 ? (a >> dist) | ((a << (-dist & 63)) != 0) : (a != 0); }
static sf_u64 sf_round_pack(int sign, int exp, sf_u64 sig) {
    unsigned rb = (unsigned)(sig & 0x3FF);
    if ((unsigned)exp >= 0x7FD) {
        if (exp < 0) { sig = sf_jam(sig, -exp); exp = 0; rb = (unsigned)(sig & 0x3FF); }
        else if (exp > 0x7FD || sig + 0x200 >= SF_SIGN) return sf_pack(sign, 0x7FF, 0);
    }
    sig = (sig + 0x200) >> 10;
    if (rb == 0x200) sig &= ~1ULL;
    if (!sig) exp = 0;
    return sf_pack(sign, exp, sig);
}
static sf_u64 sf_norm_round_pack(int sign, int exp, sf_u64 sig) {
    int sh = sf_clz(sig) - 1;
    exp -= sh;
    if (sh >= 10 && (unsigned)exp < 0x7FD) return sf_pack(sign, sig ? exp : 0, sig << (sh - 10));
    return sf_round_pack(sign, exp, sig << sh);
}
static void sf_norm_sub(sf_u64 sig, int* exp, sf_u64* out) { int sh = sf_clz(sig) - 11; *exp = 1 - sh; *out = sig << sh; }
static sf_u64 sf_add_mags(sf_u64 a, sf_u64 b, int sign) {
    int ea = sf_exp(a), eb = sf_exp(b), ez; sf_u64 sa = sf_frac(a), sb = sf_frac(b), sz;
    int d = ea - eb;
    if (!d) {
        if (!ea) return a + sb;
        if (ea == 0x7FF) return (sa | sb) ? sf_nan(a, b) : a;
        ez = ea; sz = (0x0020000000000000ULL + sa + sb) << 9;
        return sf_round_pack(sign, ez, sz);
    }
    sa <<= 9; sb <<= 9;
    if (d < 0) {
        if (eb == 0x7FF) return sb ? sf_nan(a, b) : sf_pack(sign, 0x7FF, 0);
        ez = eb; sa = ea ? sa + 0x2000000000000000ULL : sa << 1; sa = sf_jam(sa, -d);
    } else {
        if (ea == 0x7FF) return sa ? sf_nan(a, b) : a;
        ez = ea; sb = eb ? sb + 0x2000000000000000ULL : sb << 1; sb = sf_jam(sb, d);
    }
    sz = 0x2000000000000000ULL + sa + sb;
    if (sz < 0x4000000000000000ULL) { ez--; sz <<= 1; }
    return sf_round_pack(sign, ez, sz);
}
static sf_u64 sf_sub_mags(sf_u64 a, sf_u64 b, int sign) {
    int ea = sf_exp(a), eb = sf_exp(b), ez; sf_u64 sa = sf_frac(a), sb = sf_frac(b), sz;
    int d = ea - eb;
    if (!d) {
        if (ea == 0x7FF) return (sa | sb) ? sf_nan(a, b) : SF_QNAN;
        sf_i64 diff = (sf_i64)(sa - sb);
        if (!diff) return 0;
        if (ea) ea--;
        if (diff < 0) { sign = !sign; diff = -diff; }
        int sh = sf_clz((sf_u64)diff) - 11;
        ez = ea - sh;
        if (ez < 0) { sh = ea; ez = 0; }
        return sf_pack(sign, ez, (sf_u64)diff << sh);
    }
    sa <<= 10; sb <<= 10;
    if (d < 0) {
        sign = !sign;
        if (eb == 0x7FF) return sb ? sf_nan(a, b) : sf_pack(sign, 0x7FF, 0);
        sa += ea ? 0x4000000000000000ULL : sa; sa = sf_jam(sa, -d);
        sb |= 0x4000000000000000ULL; ez = eb; sz = sb - sa;
    } else {
        if (ea == 0x7FF) return sa ? sf_nan(a, b) : a;
        sb += eb ? 0x4000000000000000ULL : sb; sb = sf_jam(sb, d);
        sa |= 0x4000000000000000ULL; ez = ea; sz = sa - sb;
    }
    return sf_norm_round_pack(sign, ez - 1, sz);
}
static sf_u64 sf_add(sf_u64 a, sf_u64 b) { int s = (int)(a >> 63); return s == (int)(b >> 63) ? sf_add_mags(a, b, s) : sf_sub_mags(a, b, s); }
static sf_u64 sf_sub(sf_u64 a, sf_u64 b) { int s = (int)(a >> 63); return s == (int)(b >> 63) ? sf_sub_mags(a, b, s) : sf_add_mags(a, b, s); }
static sf_u64 sf_mul(sf_u64 a, sf_u64 b) {
    int s = (int)((a ^ b) >> 63), ea = sf_exp(a), eb = sf_exp(b); sf_u64 sa = sf_frac(a), sb = sf_frac(b);
    if (ea == 0x7FF) { if (sa || (eb == 0x7FF && sb)) return sf_nan(a, b); return (eb | sb) ? sf_pack(s, 0x7FF, 0) : SF_QNAN; }
    if (eb == 0x7FF) { if (sb) return sf_nan(a, b); return (ea | sa) ? sf_pack(s, 0x7FF, 0) : SF_QNAN; }
    if (!ea) { if (!sa) return sf_pack(s, 0, 0); sf_norm_sub(sa, &ea, &sa); }
    if (!eb) { if (!sb) return sf_pack(s, 0, 0); sf_norm_sub(sb, &eb, &sb); }
    int ez = ea + eb - 0x3FF;
    sa = (sa | 0x0010000000000000ULL) << 10;
    sb = (sb | 0x0010000000000000ULL) << 11;
    sf_u64 al = (uint32_t)sa, ah = sa >> 32, bl = (uint32_t)sb, bh = sb >> 32;
    sf_u64 ll = al * bl, lh = al * bh, hl = ah * bl, hh = ah * bh;
    sf_u64 mid = (ll >> 32) + (uint32_t)lh + (uint32_t)hl;
    sf_u64 hi = hh + (lh >> 32) + (hl >> 32) + (mid >> 32);
    sf_u64 lo = (mid << 32) | (uint32_t)ll;
    sf_u64 sz = hi | (lo != 0);
    if (sz < 0x4000000000000000ULL) { ez--; sz <<= 1; }
    return sf_round_pack(s, ez, sz);
}
static sf_u64 sf_div(sf_u64 a, sf_u64 b) {
    int s = (int)((a ^ b) >> 63), ea = sf_exp(a), eb = sf_exp(b); sf_u64 sa = sf_frac(a), sb = sf_frac(b);
    if (ea == 0x7FF) { if (sa) return sf_nan(a, b); if (eb == 0x7FF) return sb ? sf_nan(a, b) : SF_QNAN; return sf_pack(s, 0x7FF, 0); }
    if (eb == 0x7FF) return sb ? sf_nan(a, b) : sf_pack(s, 0, 0);
    if (!eb) { if (!sb) return (ea | sa) ? sf_pack(s, 0x7FF, 0) : SF_QNAN; sf_norm_sub(sb, &eb, &sb); }
    if (!ea) { if (!sa) return sf_pack(s, 0, 0); sf_norm_sub(sa, &ea, &sa); }
    int ez = ea - eb + 0x3FE;
    sa |= 0x0010000000000000ULL; sb |= 0x0010000000000000ULL;
    if (sa < sb) { ez--; sa <<= 1; }
    sf_u64 r = sa, q = 0;
    for (int i = 0; i < 63; i++) { q <<= 1; if (r >= sb) { r -= sb; q |= 1; } r <<= 1; }
    return sf_round_pack(s, ez, q | (r != 0));
}
static sf_u64 sf_sqrt(sf_u64 a) {
    int s = (int)(a >> 63), ea = sf_exp(a); sf_u64 sa = sf_frac(a);
    if (ea == 0x7FF) { if (sa) return sf_nan(a, 0); return s ? SF_QNAN : a; }
    if (s) return (ea | sa) ? SF_QNAN : a;
    if (!ea) { if (!sa) return a; sf_norm_sub(sa, &ea, &sa); }
    int e = ea - 0x3FF; sf_u64 m = sa | 0x0010000000000000ULL;
    if (e & 1) { m <<= 1; e -= 1; }
    sf_u64 root = 0, rem = 0;
    for (int j = 107; j >= 1; j -= 2) {
        sf_u64 two = j - 1 >= 54 ? (m >> (j - 1 - 54)) & 3 : 0;
        rem = (rem << 2) | two;
        sf_u64 trial = (root << 2) | 1;
        if (rem >= trial) { rem -= trial; root = (root << 1) | 1; } else root <<= 1;
    }
    return sf_round_pack(0, e / 2 + 0x3FE, (root << 9) | (rem != 0));
}
static int sf_lt(sf_u64 a, sf_u64 b) {
    int sa = (int)(a >> 63), sb = (int)(b >> 63);
    if (sa != sb) return sa && ((a | b) & SF_MAG) != 0;
    return a != b && (sa ^ (a < b));
}
static int sf_le(sf_u64 a, sf_u64 b) {
    int sa = (int)(a >> 63), sb = (int)(b >> 63);
    if (sa != sb) return sa || ((a | b) & SF_MAG) == 0;
    return a == b || (sa ^ (a < b));
}
static sf_u64 sf_from_u64(sf_u64 a) {
    if (!a) return 0;
    if (a & SF_SIGN) return sf_round_pack(0, 0x43D, sf_jam(a, 1));
    return sf_norm_round_pack(0, 0x43C, a);
}
static sf_u64 sf_from_i64(sf_i64 a) {
    int s = a < 0;
    if (!((sf_u64)a & SF_MAG)) return s ? 0xC3E0000000000000ULL : 0;
    sf_u64 m = s ? 0 - (sf_u64)a : (sf_u64)a;
    return sf_norm_round_pack(s, 0x43C, m);
}
static sf_u64 sf_to_mag(sf_u64 a, int limit, int* big) {
    int e = sf_exp(a) - 0x3FF; *big = 0;
    if (e < 0) return 0;
    if (e >= limit) { *big = 1; return 0; }
    sf_u64 m = sf_frac(a) | 0x0010000000000000ULL;
    return e >= 52 ? m << (e - 52) : m >> (52 - e);
}
SS_AAPCS double __aeabi_dadd(double a, double b) { return sf_d(sf_add(sf_b(a), sf_b(b))); }
SS_AAPCS double __aeabi_dsub(double a, double b) { return sf_d(sf_sub(sf_b(a), sf_b(b))); }
SS_AAPCS double __aeabi_drsub(double a, double b) { return sf_d(sf_sub(sf_b(b), sf_b(a))); }
SS_AAPCS double __aeabi_dmul(double a, double b) { return sf_d(sf_mul(sf_b(a), sf_b(b))); }
SS_AAPCS double __aeabi_ddiv(double a, double b) { return sf_d(sf_div(sf_b(a), sf_b(b))); }
SS_AAPCS int __aeabi_dcmpun(double a, double b) { return sf_isnan(sf_b(a)) || sf_isnan(sf_b(b)); }
SS_AAPCS int __aeabi_dcmpeq(double a, double b) { sf_u64 x = sf_b(a), y = sf_b(b); if (sf_isnan(x) || sf_isnan(y)) return 0; return x == y || ((x | y) & SF_MAG) == 0; }
SS_AAPCS int __aeabi_dcmplt(double a, double b) { sf_u64 x = sf_b(a), y = sf_b(b); if (sf_isnan(x) || sf_isnan(y)) return 0; return sf_lt(x, y); }
SS_AAPCS int __aeabi_dcmple(double a, double b) { sf_u64 x = sf_b(a), y = sf_b(b); if (sf_isnan(x) || sf_isnan(y)) return 0; return sf_le(x, y); }
SS_AAPCS int __aeabi_dcmpgt(double a, double b) { return __aeabi_dcmplt(b, a); }
SS_AAPCS int __aeabi_dcmpge(double a, double b) { return __aeabi_dcmple(b, a); }
SS_AAPCS double __aeabi_i2d(int a) { return sf_d(sf_from_i64(a)); }
SS_AAPCS double __aeabi_ui2d(unsigned a) { return sf_d(sf_from_u64(a)); }
SS_AAPCS double __aeabi_l2d(sf_i64 a) { return sf_d(sf_from_i64(a)); }
SS_AAPCS double __aeabi_ul2d(sf_u64 a) { return sf_d(sf_from_u64(a)); }
SS_AAPCS sf_i64 __aeabi_d2lz(double a) { sf_u64 x = sf_b(a); int big; if (sf_isnan(x)) return 0; sf_u64 m = sf_to_mag(x, 63, &big); if (big) return x >> 63 ? (sf_i64)SF_SIGN : (sf_i64)SF_MAG; return x >> 63 ? (sf_i64)(0 - m) : (sf_i64)m; }
SS_AAPCS sf_u64 __aeabi_d2ulz(double a) { sf_u64 x = sf_b(a); int big; if (sf_isnan(x) || x >> 63) return 0; sf_u64 m = sf_to_mag(x, 64, &big); return big ? ~0ULL : m; }
SS_AAPCS int __aeabi_d2iz(double a) { sf_i64 v = __aeabi_d2lz(a); return v > 2147483647 ? 2147483647 : v < -2147483647 - 1 ? -2147483647 - 1 : (int)v; }
SS_AAPCS unsigned __aeabi_d2uiz(double a) { sf_u64 v = __aeabi_d2ulz(a); return v > 0xFFFFFFFFULL ? 0xFFFFFFFFu : (unsigned)v; }
static double ss_sqrt(double x) { return sf_d(sf_sqrt(sf_b(x))); }

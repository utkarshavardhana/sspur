//@ fail
static void __attribute__((noinline, cold, noreturn)) ss_fail(Status* st, const char* m) { size_t n = strlen(m); char* c = (char*)malloc(n + 1); memcpy(c, m, n); st->rbuf = (int64_t*)c; st->rlen = (int64_t)n; sspur_trap(st, 12, 0, 0, 0); }
//@ utf8
static int ss_utf8_ok(const unsigned char* p, int64_t n) {
    int64_t i = 0;
    while (i < n) {
        unsigned c = p[i];
        if (c < 0x80) { i++; continue; }
        int k; unsigned lo = 0x80, hi = 0xBF;
        if (c >= 0xC2 && c <= 0xDF) k = 1;
        else if (c == 0xE0) { k = 2; lo = 0xA0; }
        else if (c >= 0xE1 && c <= 0xEC) k = 2;
        else if (c == 0xED) { k = 2; hi = 0x9F; }
        else if (c >= 0xEE && c <= 0xEF) k = 2;
        else if (c == 0xF0) { k = 3; lo = 0x90; }
        else if (c >= 0xF1 && c <= 0xF3) k = 3;
        else if (c == 0xF4) { k = 3; hi = 0x8F; }
        else return 0;
        if (i + k >= n) return 0;
        if (p[i + 1] < lo || p[i + 1] > hi) return 0;
        for (int j = 2; j <= k; j++) if ((p[i + j] & 0xC0) != 0x80) return 0;
        i += k + 1;
    }
    return 1;
}
static int64_t ss_utf8_put(char* o, uint32_t cp) {
    if (cp < 0x80) { o[0] = (char)cp; return 1; }
    if (cp < 0x800) { o[0] = (char)(0xC0 | (cp >> 6)); o[1] = (char)(0x80 | (cp & 0x3F)); return 2; }
    if (cp < 0x10000) { o[0] = (char)(0xE0 | (cp >> 12)); o[1] = (char)(0x80 | ((cp >> 6) & 0x3F)); o[2] = (char)(0x80 | (cp & 0x3F)); return 3; }
    o[0] = (char)(0xF0 | (cp >> 18)); o[1] = (char)(0x80 | ((cp >> 12) & 0x3F)); o[2] = (char)(0x80 | ((cp >> 6) & 0x3F)); o[3] = (char)(0x80 | (cp & 0x3F)); return 4;
}
static uint32_t ss_utf8_get(Str s, int64_t* i) {
    const unsigned char* p = (const unsigned char*)s.p; unsigned c = p[*i];
    if (c < 0x80) { (*i)++; return c; }
    int k = c >= 0xF0 ? 3 : c >= 0xE0 ? 2 : 1; uint32_t cp = c & (0x3F >> k);
    for (int j = 1; j <= k; j++) cp = (cp << 6) | (p[*i + j] & 0x3F);
    *i += k + 1; return cp;
}
//@ sys
#include <fcntl.h>
#include <errno.h>
#include <dirent.h>
//@ fs fail utf8 sys
static Str ss_why(Str path, int e) {
    SB_INIT(b); sb_put(&b, path.p, path.len); sb_put(&b, ": ", 2);
    const char* w = e == -1 ? "invalid path" : e == -2 ? "invalid UTF-8" : e == ENOENT ? "not found" : (e == EACCES || e == EPERM) ? "permission denied" : e == EISDIR ? "is a directory" : e == ENOTDIR ? "not a directory" : e == EEXIST ? "already exists" : 0;
    if (w) sb_put(&b, w, (int64_t)strlen(w)); else { sb_put(&b, "os error ", 9); sb_int(&b, e); }
    return sb_done(&b);
}
static char* ss_cpath(Str p) { if (memchr(p.p, 0, (size_t)p.len)) return 0; char* c = (char*)sspur_alloc_atomic((size_t)p.len + 1); memcpy(c, p.p, (size_t)p.len); c[p.len] = 0; return c; }
static int ss_read_file(Str path, Str* out, Str* err) {
    char* c = ss_cpath(path); if (!c) { *err = ss_why(path, -1); return 0; }
    int fd = open(c, O_RDONLY | O_CLOEXEC); if (fd < 0) { *err = ss_why(path, errno); return 0; }
    int64_t cap = 4096, n = 0; char* buf = (char*)malloc((size_t)cap);
    for (;;) {
        if (n == cap) { cap *= 2; buf = (char*)realloc(buf, (size_t)cap); }
        ssize_t k = read(fd, buf + n, (size_t)(cap - n));
        if (k < 0) { if (errno == EINTR) continue; int e = errno; close(fd); free(buf); *err = ss_why(path, e); return 0; }
        if (k == 0) break;
        n += k;
    }
    close(fd);
    if (!ss_utf8_ok((const unsigned char*)buf, n)) { free(buf); *err = ss_why(path, -2); return 0; }
    char* o = (char*)sspur_alloc_atomic((size_t)n + 1); memcpy(o, buf, (size_t)n); free(buf);
    *out = (Str){n, o}; return 1;
}
static int ss_write_file(Str path, Str s, int append, Str* err) {
    char* c = ss_cpath(path); if (!c) { *err = ss_why(path, -1); return 0; }
    int fd = open(c, O_WRONLY | O_CREAT | O_CLOEXEC | (append ? O_APPEND : O_TRUNC), 0666); if (fd < 0) { *err = ss_why(path, errno); return 0; }
    int64_t off = 0;
    while (off < s.len) {
        ssize_t k = write(fd, s.p + off, (size_t)(s.len - off));
        if (k < 0) { if (errno == EINTR) continue; int e = errno; close(fd); *err = ss_why(path, e); return 0; }
        off += k;
    }
    close(fd); return 1;
}
static int ss_remove_file(Str path, Str* err) {
    char* c = ss_cpath(path); if (!c) { *err = ss_why(path, -1); return 0; }
    if (unlink(c) != 0) { *err = ss_why(path, errno); return 0; }
    return 1;
}
static int ss_list_dir(Str path, RawL* out, Str* err) {
    char* c = ss_cpath(path); if (!c) { *err = ss_why(path, -1); return 0; }
    DIR* d = opendir(c); if (!d) { *err = ss_why(path, errno); return 0; }
    RawL r = raw_alloc(8, sizeof(Str)); struct dirent* e;
    while ((e = readdir(d))) {
        if (!strcmp(e->d_name, ".") || !strcmp(e->d_name, "..")) continue;
        int64_t n = (int64_t)strlen(e->d_name); char* o = (char*)sspur_alloc_atomic((size_t)n + 1); memcpy(o, e->d_name, (size_t)n);
        Str s = {n, o}; r = raw_push(r, &s, sizeof(Str));
    }
    closedir(d);
    if (r.len > 1) raw_msort((char*)r.data, r.len, sizeof(Str), cmp_S_p, (char*)sspur_alloc((size_t)r.len * sizeof(Str)));
    *out = r; return 1;
}
//@ io fail utf8 sys
static int ss_read_line(Str* out, Status* st) {
    int64_t cap = 128, n = 0; char* buf = (char*)malloc((size_t)cap); int got = 0;
    for (;;) {
        unsigned char ch; ssize_t k = read(0, &ch, 1);
        if (k < 0 && errno == EINTR) continue;
        if (k <= 0) break;
        got = 1;
        if (ch == '\n') break;
        if (n == cap) { cap *= 2; buf = (char*)realloc(buf, (size_t)cap); }
        buf[n++] = (char)ch;
    }
    if (!got) { free(buf); return 0; }
    if (n && buf[n - 1] == '\r') n--;
    if (!ss_utf8_ok((const unsigned char*)buf, n)) { free(buf); ss_fail(st, "stdin is not valid UTF-8"); }
    char* o = (char*)sspur_alloc_atomic((size_t)n + 1); memcpy(o, buf, (size_t)n); free(buf);
    *out = (Str){n, o}; return 1;
}
//@ time
static int64_t ss_now_ms(void) { struct timespec t; clock_gettime(CLOCK_REALTIME, &t); return (int64_t)t.tv_sec * 1000 + (int64_t)t.tv_nsec / 1000000; }
static int64_t ss_mono_ns(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return (int64_t)t.tv_sec * 1000000000 + (int64_t)t.tv_nsec; }
static void ss_sleep_ms(int64_t ms) { if (ms <= 0) return; struct timespec t = {(time_t)(ms / 1000), (long)(ms % 1000) * 1000000}; while (nanosleep(&t, &t) != 0) {} }
//@ env utf8
static Str* ss_argv; static int64_t ss_argc;
void sspur_set_args(const Str* a, int64_t n) { ss_argv = (Str*)malloc((size_t)(n ? n : 1) * sizeof(Str)); for (int64_t i = 0; i < n; i++) { char* c = (char*)malloc((size_t)a[i].len + 1); memcpy(c, a[i].p, (size_t)a[i].len); ss_argv[i] = (Str){a[i].len, c}; } ss_argc = n; }
static int ss_env(Str k, Str* out) {
    if (!k.len || memchr(k.p, '=', (size_t)k.len) || memchr(k.p, 0, (size_t)k.len)) return 0;
    char* c = (char*)sspur_alloc_atomic((size_t)k.len + 1); memcpy(c, k.p, (size_t)k.len); c[k.len] = 0;
    const char* v = getenv(c); if (!v) return 0;
    int64_t n = (int64_t)strlen(v); if (!ss_utf8_ok((const unsigned char*)v, n)) return 0;
    char* o = (char*)sspur_alloc_atomic((size_t)n + 1); memcpy(o, v, (size_t)n); *out = (Str){n, o}; return 1;
}
//@ fmt fail
static Str ss_fmt_fixed(double x, int64_t d, Status* st) {
    if (d < 0 || d > 20) ss_fail(st, "fmt digits must be in 0..=20");
    if (x != x) return str_lit("NaN", 3);
    if (isinf(x)) return x > 0 ? str_lit("inf", 3) : str_lit("-inf", 4);
    char t[400]; int n = snprintf(t, sizeof t, "%.*f", (int)d, x);
    char* o = (char*)sspur_alloc_atomic((size_t)n + 1); memcpy(o, t, (size_t)n); return (Str){n, o};
}
static int ss_float_syntax(Str s) {
    const char* b = s.p; int64_t n = s.len, i = 0; int dg = 0;
    if (i < n && (b[i] == '+' || b[i] == '-')) i++;
    while (i < n && b[i] >= '0' && b[i] <= '9') { i++; dg = 1; }
    if (i < n && b[i] == '.') { i++; while (i < n && b[i] >= '0' && b[i] <= '9') { i++; dg = 1; } }
    if (!dg) return 0;
    if (i < n && (b[i] == 'e' || b[i] == 'E')) { i++; if (i < n && (b[i] == '+' || b[i] == '-')) i++; int64_t e0 = i; while (i < n && b[i] >= '0' && b[i] <= '9') i++; if (i == e0) return 0; }
    return i == n;
}
static int ss_to_f64(Str s, double* out) {
    s = str_trim(s);
    if (!ss_float_syntax(s)) return 0;
    char* c = (char*)sspur_alloc_atomic((size_t)s.len + 1); memcpy(c, s.p, (size_t)s.len); c[s.len] = 0;
    *out = strtod(c, 0); return 1;
}
//@ strx fail utf8
static Str ss_pad(Str s, int64_t n, Str fill, int left, Status* st) {
    int64_t len = utf8_len(s);
    if (!fill.len || len >= n) return s;
    int64_t fc = utf8_len(fill), copies = (n - len + fc - 1) / fc;
    if (copies > ((int64_t)1 << 28) / fill.len) sspur_trap(st, 15, 0, 0, 0);
    Str p = str_take(str_repeat(fill, copies), n - len);
    return left ? str_cat(p, s) : str_cat(s, p);
}
static RawL ss_codes(Str s) { RawL r = raw_alloc_a(s.len, 8, 1); int64_t* d = (int64_t*)r.data; int64_t i = 0, k = 0; while (i < s.len) d[k++] = ss_utf8_get(s, &i); r.len = k; r.hdr[1] = k; return r; }
static RawL ss_bytes(Str s) { RawL r = raw_alloc_a(s.len, 8, 1); int64_t* d = (int64_t*)r.data; for (int64_t i = 0; i < s.len; i++) d[i] = (unsigned char)s.p[i]; r.len = s.len; r.hdr[1] = s.len; return r; }
static int ss_from_bytes(const int64_t* d, int64_t n, Str* out) {
    char* o = (char*)sspur_alloc_atomic((size_t)n + 1);
    for (int64_t i = 0; i < n; i++) { if (d[i] < 0 || d[i] > 255) return 0; o[i] = (char)d[i]; }
    if (!ss_utf8_ok((const unsigned char*)o, n)) return 0;
    *out = (Str){n, o}; return 1;
}
static int ss_from_codes(const int64_t* d, int64_t n, Str* out) {
    if (n > ((int64_t)1 << 26)) return 0;
    char* o = (char*)sspur_alloc_atomic((size_t)n * 4 + 1); int64_t k = 0;
    for (int64_t i = 0; i < n; i++) { int64_t c = d[i]; if (c < 0 || c > 0x10FFFF || (c >= 0xD800 && c < 0xE000)) return 0; k += ss_utf8_put(o + k, (uint32_t)c); }
    *out = (Str){k, o}; return 1;
}
//@ front
static RawL raw_push_front(RawL l, const void* v, size_t es) {
    if (l.hdr) {
        int64_t off = raw_offset(l, es);
        if (off > 0 && l.hdr[3] == off && (!SS_MT() || __atomic_compare_exchange_n(&l.hdr[3], &off, off - 1, 0, __ATOMIC_ACQ_REL, __ATOMIC_RELAXED))) {
            if (!SS_MT()) l.hdr[3] = off - 1;
            l.data = (char*)l.data - es; memcpy(l.data, v, es); l.len += 1; return l;
        }
    }
    int64_t slack = l.len + 4;
    RawL n = raw_alloc_a(slack + l.len + 4, es, l.hdr ? (int)l.hdr[2] : 0);
    char* base = (char*)n.data;
    if (l.len) memcpy(base + (size_t)slack * es, l.data, (size_t)l.len * es);
    memcpy(base + (size_t)(slack - 1) * es, v, es);
    n.data = base + (size_t)(slack - 1) * es; n.len = l.len + 1; n.hdr[1] = slack + l.len; n.hdr[3] = slack - 1;
    return n;
}
//@ sbuf
static SBuf sbuf_add(SBuf b, Str s) {
    if (!s.len) return b;
    RawL l = TO_RAW(b);
    if (!l.hdr) { RawL n = raw_alloc_a(l.len + s.len * 2 + 16, 1, 1); if (l.len) memcpy(n.data, l.data, (size_t)l.len); n.len = l.len; n.hdr[1] = l.len; l = n; }
    RawL r = raw_reserve(l, s.len, 1);
    memcpy((char*)r.data + r.len, s.p, (size_t)s.len);
    r.len += s.len; r.hdr[1] = raw_offset(r, 1) + r.len;
    return (SBuf){r.len, (char*)r.data, r.hdr};
}
//@ json utf8
enum { SJ_NULL, SJ_FALSE, SJ_TRUE, SJ_NUM, SJ_STR, SJ_ARR, SJ_OBJ };
typedef struct SsJ { int t; Str s; int64_t len; struct SsJ* items; Str* keys; } SsJ;
typedef struct { const unsigned char* p; const unsigned char* e; int depth; } SsJP;
typedef struct { Str* seg; int64_t n, cap; Str msg; } SsJE;
static void sj_ws(SsJP* j) { while (j->p < j->e && (*j->p == ' ' || *j->p == '\t' || *j->p == '\n' || *j->p == '\r')) j->p++; }
static int sj_hex4(SsJP* j, unsigned* out) {
    if (j->e - j->p < 4) return 0;
    unsigned v = 0;
    for (int i = 0; i < 4; i++) { int c = j->p[i]; int h = c >= '0' && c <= '9' ? c - '0' : c >= 'a' && c <= 'f' ? c - 'a' + 10 : c >= 'A' && c <= 'F' ? c - 'A' + 10 : -1; if (h < 0) return 0; v = v * 16 + (unsigned)h; }
    j->p += 4; *out = v; return 1;
}
static int sj_str(SsJP* j, Str* out) {
    j->p++;
    char* o = (char*)sspur_alloc_atomic((size_t)(j->e - j->p) + 4); int64_t n = 0;
    for (;;) {
        if (j->p >= j->e) return 0;
        unsigned c = *j->p++;
        if (c == '"') break;
        if (c < 0x20) return 0;
        if (c != '\\') { o[n++] = (char)c; continue; }
        if (j->p >= j->e) return 0;
        unsigned x = *j->p++;
        switch (x) {
        case '"': o[n++] = '"'; break;
        case '\\': o[n++] = '\\'; break;
        case '/': o[n++] = '/'; break;
        case 'b': o[n++] = '\b'; break;
        case 'f': o[n++] = '\f'; break;
        case 'n': o[n++] = '\n'; break;
        case 'r': o[n++] = '\r'; break;
        case 't': o[n++] = '\t'; break;
        case 'u': {
            unsigned cp;
            if (!sj_hex4(j, &cp)) return 0;
            if (cp >= 0xD800 && cp < 0xDC00) {
                unsigned lo;
                if (j->e - j->p < 6 || j->p[0] != '\\' || j->p[1] != 'u') return 0;
                j->p += 2;
                if (!sj_hex4(j, &lo) || lo < 0xDC00 || lo > 0xDFFF) return 0;
                cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
            } else if (cp >= 0xDC00 && cp < 0xE000) return 0;
            n += ss_utf8_put(o + n, cp);
            break;
        }
        default: return 0;
        }
    }
    *out = (Str){n, o}; return 1;
}
static int sj_digits(SsJP* j) { const unsigned char* s = j->p; while (j->p < j->e && *j->p >= '0' && *j->p <= '9') j->p++; return j->p > s; }
static int sj_value(SsJP* j, SsJ* v) {
    sj_ws(j);
    memset(v, 0, sizeof *v);
    j->depth++;
    if (j->p >= j->e || j->depth > 256) return 0;
    unsigned c = *j->p;
    if (c == '{' || c == '[') {
        int obj = c == '{'; unsigned close = obj ? '}' : ']';
        j->p++; sj_ws(j);
        int64_t cap = 4, n = 0;
        SsJ* items = (SsJ*)sspur_alloc((size_t)cap * sizeof(SsJ)); Str* keys = obj ? (Str*)sspur_alloc((size_t)cap * sizeof(Str)) : 0;
        if (j->p < j->e && *j->p == close) j->p++;
        else for (;;) {
            if (n == cap) {
                SsJ* ni = (SsJ*)sspur_alloc((size_t)cap * 2 * sizeof(SsJ)); memcpy(ni, items, (size_t)n * sizeof(SsJ)); items = ni;
                if (obj) { Str* nk = (Str*)sspur_alloc((size_t)cap * 2 * sizeof(Str)); memcpy(nk, keys, (size_t)n * sizeof(Str)); keys = nk; }
                cap *= 2;
            }
            if (obj) {
                sj_ws(j);
                if (j->p >= j->e || *j->p != '"' || !sj_str(j, &keys[n])) return 0;
                sj_ws(j);
                if (j->p >= j->e || *j->p != ':') return 0;
                j->p++;
            }
            if (!sj_value(j, &items[n])) return 0;
            n++;
            sj_ws(j);
            if (j->p < j->e && *j->p == ',') { j->p++; continue; }
            if (j->p < j->e && *j->p == close) { j->p++; break; }
            return 0;
        }
        v->t = obj ? SJ_OBJ : SJ_ARR; v->len = n; v->items = items; v->keys = keys;
    } else if (c == '"') {
        v->t = SJ_STR;
        if (!sj_str(j, &v->s)) return 0;
    } else if (c == '-' || (c >= '0' && c <= '9')) {
        const unsigned char* s = j->p;
        if (c == '-') j->p++;
        if (j->p >= j->e || *j->p < '0' || *j->p > '9') return 0;
        if (*j->p == '0') j->p++; else sj_digits(j);
        if (j->p < j->e && *j->p == '.') { j->p++; if (!sj_digits(j)) return 0; }
        if (j->p < j->e && (*j->p == 'e' || *j->p == 'E')) { j->p++; if (j->p < j->e && (*j->p == '+' || *j->p == '-')) j->p++; if (!sj_digits(j)) return 0; }
        v->t = SJ_NUM; v->s = (Str){j->p - s, (const char*)s};
    } else {
        int64_t left = j->e - j->p;
        if (left >= 4 && !memcmp(j->p, "null", 4)) { v->t = SJ_NULL; j->p += 4; }
        else if (left >= 4 && !memcmp(j->p, "true", 4)) { v->t = SJ_TRUE; j->p += 4; }
        else if (left >= 5 && !memcmp(j->p, "false", 5)) { v->t = SJ_FALSE; j->p += 5; }
        else return 0;
    }
    j->depth--;
    return 1;
}
static SsJ* sj_parse(Str s) {
    SsJP j = {(const unsigned char*)s.p, (const unsigned char*)s.p + s.len, 0};
    SsJ* v = (SsJ*)sspur_alloc(sizeof(SsJ));
    if (!sj_value(&j, v)) return 0;
    sj_ws(&j);
    return j.p == j.e ? v : 0;
}
static SsJ* sj_get(SsJ* o, const char* k, int64_t kn) {
    if (!o || o->t != SJ_OBJ) return 0;
    for (int64_t i = 0; i < o->len; i++) if (o->keys[i].len == kn && !memcmp(o->keys[i].p, k, (size_t)kn)) return &o->items[i];
    return 0;
}
static void sj_push(SsJE* e, Str s) { if (e->n == e->cap) { int64_t nc = e->cap ? e->cap * 2 : 16; Str* ns = (Str*)sspur_alloc((size_t)nc * sizeof(Str)); if (e->n) memcpy(ns, e->seg, (size_t)e->n * sizeof(Str)); e->seg = ns; e->cap = nc; } e->seg[e->n++] = s; }
static void sj_pushi(SsJE* e, int64_t i) { SB_INIT(b); sb_put(&b, "[", 1); sb_int(&b, i); sb_put(&b, "]", 1); sj_push(e, sb_done(&b)); }
static int sj_err(SsJE* e, const char* want, int64_t wn, SsJ* v) {
    SB_INIT(b);
    if (!e->n) sb_put(&b, "value", 5);
    for (int64_t i = 0; i < e->n; i++) { if (i && e->seg[i].p[0] != '[') sb_put(&b, ".", 1); sb_put(&b, e->seg[i].p, e->seg[i].len); }
    const char* got = !v ? "nothing" : v->t == SJ_NULL ? "null" : v->t == SJ_NUM ? "a number" : v->t == SJ_STR ? "a string" : v->t == SJ_ARR ? "an array" : v->t == SJ_OBJ ? "an object" : "a boolean";
    sb_put(&b, ": expected ", 11); sb_put(&b, want, wn); sb_put(&b, ", found ", 8); sb_put(&b, got, (int64_t)strlen(got));
    e->msg = sb_done(&b);
    return 0;
}
static void sj_quote(SB* b, Str s) {
    static const char hx[] = "0123456789abcdef";
    sb_put(b, "\"", 1); int64_t run = 0;
    for (int64_t i = 0; i < s.len; i++) {
        unsigned char c = (unsigned char)s.p[i];
        if (c == '"' || c == '\\' || c < 0x20) {
            if (i > run) sb_put(b, s.p + run, i - run);
            char e[6] = {'\\', (char)c, '0', '0', hx[c >> 4], hx[c & 15]};
            if (c == '"' || c == '\\') sb_put(b, e, 2);
            else if (c == '\n') sb_put(b, "\\n", 2);
            else if (c == '\t') sb_put(b, "\\t", 2);
            else if (c == '\r') sb_put(b, "\\r", 2);
            else { e[1] = 'u'; sb_put(b, e, 6); }
            run = i + 1;
        }
    }
    if (s.len > run) sb_put(b, s.p + run, s.len - run);
    sb_put(b, "\"", 1);
}
static int sj_int(SsJ* v, int64_t* out) {
    if (!v || v->t != SJ_NUM || v->s.len > 24) return 0;
    for (int64_t i = 0; i < v->s.len; i++) if (!((v->s.p[i] >= '0' && v->s.p[i] <= '9') || (i == 0 && v->s.p[i] == '-'))) return 0;
    OptI_ o = str_to_int(v->s); if (!o.some) return 0; *out = o.v; return 1;
}
static int sj_f64(SsJ* v, double* out) {
    if (!v || v->t != SJ_NUM || v->s.len > 400) return 0;
    char t[408]; memcpy(t, v->s.p, (size_t)v->s.len); t[v->s.len] = 0; *out = strtod(t, 0); return 1;
}
static void sj_f64_put(SB* b, double x) { if (isfinite(x)) sb_f64(b, x); else sb_put(b, "null", 4); }

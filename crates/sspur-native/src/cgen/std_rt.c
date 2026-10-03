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
    const char* w = e == -1 ? "invalid path" : e == -2 ? "invalid UTF-8" : e == -3 ? "byte out of range" : e == ENOENT ? "not found" : (e == EACCES || e == EPERM) ? "permission denied" : e == EISDIR ? "is a directory" : e == ENOTDIR ? "not a directory" : e == EEXIST ? "already exists" : e == ENOTEMPTY ? "directory not empty" : 0;
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
//@ failstr
static void __attribute__((noinline, cold, noreturn)) ss_failstr(Status* st, Str m) { char* c = (char*)malloc((size_t)m.len + 1); memcpy(c, m.p, (size_t)m.len); st->rbuf = (int64_t*)c; st->rlen = m.len; sspur_trap(st, 12, 0, 0, 0); }
//@ fmtspec failstr utf8
typedef struct { uint32_t fill; int has_fill, align, sign, alt, zero, comma, ty; int64_t width, prec; } SsFs;
static int ss_fs_isal(char c) { return c == '<' || c == '>' || c == '^' || c == '='; }
static int ss_fs_parse(Str s, SsFs* f) {
    memset(f, 0, sizeof *f); f->fill = ' '; f->prec = -1;
    int64_t i = 0, n = s.len; const char* c = s.p;
    if (n > 0) {
        int64_t j = 0; uint32_t cp = ss_utf8_get(s, &j);
        if (j < n && ss_fs_isal(c[j])) { f->fill = cp; f->has_fill = 1; f->align = c[j]; i = j + 1; }
        else if (ss_fs_isal(c[0])) { f->align = c[0]; i = 1; }
    }
    if (i < n && (c[i] == '+' || c[i] == '-' || c[i] == ' ')) f->sign = c[i++];
    if (i < n && c[i] == '#') { f->alt = 1; i++; }
    if (i < n && c[i] == '0') { f->zero = 1; i++; }
    while (i < n && c[i] >= '0' && c[i] <= '9') { f->width = f->width * 10 + (c[i] - '0'); if (f->width > 100000) return 0; i++; }
    if (i < n && c[i] == ',') { f->comma = 1; i++; }
    if (i < n && c[i] == '.') {
        i++; int64_t s0 = i, p = 0;
        while (i < n && c[i] >= '0' && c[i] <= '9') { p = p * 10 + (c[i] - '0'); if (p > 1000) return 0; i++; }
        if (i == s0) return 0;
        f->prec = p;
    }
    if (i < n && c[i] && strchr("dxXobfeE%s", c[i])) f->ty = c[i++];
    return i == n;
}
static void ss_fs_bad(Str spec, Status* st) { SB_INIT(b); sb_put(&b, "bad format spec '", 17); sb_put(&b, spec.p, spec.len); sb_put(&b, "'", 1); ss_failstr(st, sb_done(&b)); }
static Str ss_fs_group(const char* d, int64_t n) {
    int64_t lead = 0; while (lead < n && d[lead] >= '0' && d[lead] <= '9') lead++;
    SB_INIT(b);
    for (int64_t k = 0; k < lead; k++) { if (k > 0 && (lead - k) % 3 == 0) sb_put(&b, ",", 1); sb_put(&b, d + k, 1); }
    sb_put(&b, d + lead, n - lead);
    return sb_done(&b);
}
static void ss_fs_rep(SB* b, uint32_t cp, int64_t k) { char t[4]; int64_t m = ss_utf8_put(t, cp); for (int64_t i = 0; i < k; i++) sb_put(b, t, m); }
static Str ss_fs_pad(SsFs* f, const char* sign, const char* prefix, Str body, int numeric) {
    int64_t len = (int64_t)strlen(sign) + (int64_t)strlen(prefix) + utf8_len(body);
    uint32_t fill = f->has_fill ? f->fill : f->zero ? '0' : ' ';
    int align = f->align ? f->align : f->zero ? '=' : numeric ? '>' : '<';
    int64_t n = f->width > len ? f->width - len : 0;
    SB_INIT(b);
    if (align == '>') ss_fs_rep(&b, fill, n);
    if (align == '^') ss_fs_rep(&b, fill, n / 2);
    sb_put(&b, sign, (int64_t)strlen(sign)); sb_put(&b, prefix, (int64_t)strlen(prefix));
    if (align == '=') ss_fs_rep(&b, fill, n);
    sb_put(&b, body.p, body.len);
    if (align == '<') ss_fs_rep(&b, fill, n);
    if (align == '^') ss_fs_rep(&b, fill, n - n / 2);
    return sb_done(&b);
}
static const char* ss_fs_sign(int neg, SsFs* f) { return neg ? "-" : f->sign == '+' ? "+" : f->sign == ' ' ? " " : ""; }
static Str ss_format_int(int64_t v, Str spec, Status* st) {
    SsFs f;
    if (!ss_fs_parse(spec, &f) || f.prec >= 0 || !(f.ty == 0 || f.ty == 'd' || f.ty == 'x' || f.ty == 'X' || f.ty == 'o' || f.ty == 'b') || (f.comma && f.ty != 0 && f.ty != 'd')) ss_fs_bad(spec, st);
    uint64_t u = v < 0 ? (uint64_t)0 - (uint64_t)v : (uint64_t)v;
    int base = f.ty == 'x' || f.ty == 'X' ? 16 : f.ty == 'o' ? 8 : f.ty == 'b' ? 2 : 10;
    const char* dg = f.ty == 'X' ? "0123456789ABCDEF" : "0123456789abcdef";
    char t[72]; int k = 72;
    do { t[--k] = dg[u % (uint64_t)base]; u /= (uint64_t)base; } while (u);
    Str d = {72 - k, t + k};
    if (f.comma) d = ss_fs_group(d.p, d.len);
    const char* pre = !f.alt ? "" : f.ty == 'x' ? "0x" : f.ty == 'X' ? "0X" : f.ty == 'o' ? "0o" : f.ty == 'b' ? "0b" : "";
    return ss_fs_pad(&f, ss_fs_sign(v < 0, &f), pre, d, 1);
}
static Str ss_fs_printf(const char* fmt, int64_t p, double x) {
    int n = snprintf(0, 0, fmt, (int)p, x);
    char* o = (char*)sspur_alloc_atomic((size_t)n + 1); snprintf(o, (size_t)n + 1, fmt, (int)p, x);
    return (Str){n, o};
}
static Str ss_fs_exp(int64_t p, double x, int upper) {
    Str m = ss_fs_printf(upper ? "%.*E" : "%.*e", p, x);
    return m;
}
static Str ss_format_f64(double x, Str spec, Status* st) {
    SsFs f;
    if (!ss_fs_parse(spec, &f) || f.alt || !(f.ty == 0 || f.ty == 'f' || f.ty == 'e' || f.ty == 'E' || f.ty == '%') || (f.comma && (f.ty == 'e' || f.ty == 'E'))) ss_fs_bad(spec, st);
    int nan = x != x, neg = signbit(x) && !nan; double a = fabs(x);
    Str body;
    if (nan) body = str_lit("NaN", 3);
    else if (isinf(a)) body = str_lit("inf", 3);
    else if (f.ty == 'e' || f.ty == 'E') body = ss_fs_exp(f.prec < 0 ? 6 : f.prec, a, f.ty == 'E');
    else if (f.ty == '%') body = str_cat(ss_fs_printf("%.*f", f.prec < 0 ? 6 : f.prec, a * 100.0), str_lit("%", 1));
    else if (f.ty == 'f' || f.prec >= 0) body = ss_fs_printf("%.*f", f.prec < 0 ? 6 : f.prec, a);
    else { SB_INIT(b); sb_f64(&b, a); body = sb_done(&b); }
    if (f.comma && isfinite(x)) body = ss_fs_group(body.p, body.len);
    return ss_fs_pad(&f, ss_fs_sign(neg, &f), "", body, 1);
}
static Str ss_format_str(Str s, Str spec, Status* st) {
    SsFs f;
    if (!ss_fs_parse(spec, &f) || f.sign || f.alt || f.zero || f.comma || f.align == '=' || !(f.ty == 0 || f.ty == 's')) ss_fs_bad(spec, st);
    if (f.prec >= 0) s = str_take(s, f.prec);
    return ss_fs_pad(&f, "", "", s, 0);
}
//@ rng
static inline uint64_t ss_rng(uint64_t* s) { uint64_t m = *s + 0x9E3779B97F4A7C15ULL; *s = m; m = (m ^ (m >> 30)) * 0xBF58476D1CE4E5B9ULL; m = (m ^ (m >> 27)) * 0x94D049BB133111EBULL; return m ^ (m >> 31); }
static inline double ss_rng_f64(uint64_t* s) { return (double)(ss_rng(s) >> 11) * (1.0 / 9007199254740992.0); }
static inline int64_t ss_rng_below(uint64_t* s, int64_t n) { return (int64_t)(((unsigned __int128)ss_rng(s) * (unsigned __int128)(uint64_t)n) >> 64); }
static double ss_rng_normal(uint64_t* s, double mean, double sd) { double u1 = ss_rng_f64(s); double u2 = ss_rng_f64(s); double r = sqrt(-2.0 * log(1.0 - u1)); double c = cos(6.283185307179586 * u2); double z = r * c; double t = sd * z; return mean + t; }
//@ fsx fs
#include <sys/stat.h>
#include <stdio.h>
static int ss_read_bytes(Str path, RawL* out, Str* err) {
    char* c = ss_cpath(path); if (!c) { *err = ss_why(path, -1); return 0; }
    int fd = open(c, O_RDONLY | O_CLOEXEC); if (fd < 0) { *err = ss_why(path, errno); return 0; }
    int64_t cap = 4096, n = 0; unsigned char* buf = (unsigned char*)malloc((size_t)cap);
    for (;;) {
        if (n == cap) { cap *= 2; buf = (unsigned char*)realloc(buf, (size_t)cap); }
        ssize_t k = read(fd, buf + n, (size_t)(cap - n));
        if (k < 0) { if (errno == EINTR) continue; int e = errno; close(fd); free(buf); *err = ss_why(path, e); return 0; }
        if (k == 0) break;
        n += k;
    }
    close(fd);
    RawL r = raw_alloc_a(n, 8, 1); int64_t* d = (int64_t*)r.data;
    for (int64_t i = 0; i < n; i++) d[i] = buf[i];
    free(buf); r.len = n; r.hdr[1] = n; *out = r; return 1;
}
static int ss_write_bytes(Str path, const int64_t* d, int64_t n, Str* err) {
    char* c = ss_cpath(path); if (!c) { *err = ss_why(path, -1); return 0; }
    for (int64_t i = 0; i < n; i++) if (d[i] < 0 || d[i] > 255) { *err = ss_why(path, -3); return 0; }
    char* b = (char*)sspur_alloc_atomic((size_t)n + 1);
    for (int64_t i = 0; i < n; i++) b[i] = (char)d[i];
    return ss_write_file(path, (Str){n, b}, 0, err);
}
static int ss_mkdir(Str path, int all, Str* err) {
    char* c = ss_cpath(path); if (!c) { *err = ss_why(path, -1); return 0; }
    if (!all) { if (mkdir(c, 0777) != 0) { *err = ss_why(path, errno); return 0; } return 1; }
    if (!path.len) { *err = ss_why(path, ENOENT); return 0; }
    for (int64_t i = 1; i <= path.len; i++) {
        if (i < path.len && c[i] != '/') continue;
        char save = c[i]; c[i] = 0;
        int r = mkdir(c, 0777), e = errno; struct stat sb;
        int ok = r == 0 || (e == EEXIST && stat(c, &sb) == 0 && S_ISDIR(sb.st_mode));
        c[i] = save;
        if (!ok) { *err = ss_why(path, e); return 0; }
    }
    return 1;
}
static int ss_remove_dir(Str path, Str* err) {
    char* c = ss_cpath(path); if (!c) { *err = ss_why(path, -1); return 0; }
    if (rmdir(c) != 0) { *err = ss_why(path, errno); return 0; }
    return 1;
}
static int ss_rename(Str from, Str to, Str* err) {
    char* a = ss_cpath(from); char* b = ss_cpath(to); if (!a || !b) { *err = ss_why(from, -1); return 0; }
    if (rename(a, b) != 0) { *err = ss_why(from, errno); return 0; }
    return 1;
}
static int64_t ss_path_kind(Str path) { char* c = ss_cpath(path); struct stat sb; if (!c || stat(c, &sb) != 0) return 0; return S_ISDIR(sb.st_mode) ? 2 : 1; }
static int ss_stat_num(Str path, int mtime, int64_t* out, Str* err) {
    char* c = ss_cpath(path); if (!c) { *err = ss_why(path, -1); return 0; }
    struct stat sb; if (stat(c, &sb) != 0) { *err = ss_why(path, errno); return 0; }
#ifdef __APPLE__
    struct timespec m = sb.st_mtimespec;
#else
    struct timespec m = sb.st_mtim;
#endif
    *out = mtime ? (int64_t)m.tv_sec * 1000 + (int64_t)m.tv_nsec / 1000000 : (int64_t)sb.st_size; return 1;
}
//@ eprint
static void ss_eprint(Str s) { int64_t off = 0; while (off < s.len) { ssize_t k = write(2, s.p + off, (size_t)(s.len - off)); if (k <= 0) break; off += k; } (void)!write(2, "\n", 1); }
//@ proc fs
#include <spawn.h>
#include <poll.h>
#include <sys/wait.h>
extern char** environ;
static Str ss_proc_why(Str prog, const char* w) { SB_INIT(b); sb_put(&b, prog.p, prog.len); sb_put(&b, ": ", 2); sb_put(&b, w, (int64_t)strlen(w)); return sb_done(&b); }
static int ss_run_cmd(Str prog, const Str* args, int64_t na, Str input, int64_t* code, Str* out, Str* errs, Str* why) {
    char* p0 = ss_cpath(prog); if (!p0) { *why = ss_why(prog, -1); return 0; }
    char** argv = (char**)sspur_alloc((size_t)(na + 2) * sizeof(char*)); argv[0] = p0;
    for (int64_t i = 0; i < na; i++) { argv[i + 1] = ss_cpath(args[i]); if (!argv[i + 1]) { *why = ss_proc_why(prog, "invalid argument"); return 0; } }
    argv[na + 1] = 0;
    int pi[2], po[2], pe[2];
    if (pipe(pi)) { *why = ss_why(prog, errno); return 0; }
    if (pipe(po)) { int e = errno; close(pi[0]); close(pi[1]); *why = ss_why(prog, e); return 0; }
    if (pipe(pe)) { int e = errno; close(pi[0]); close(pi[1]); close(po[0]); close(po[1]); *why = ss_why(prog, e); return 0; }
    posix_spawn_file_actions_t fa; posix_spawn_file_actions_init(&fa);
    posix_spawn_file_actions_adddup2(&fa, pi[0], 0); posix_spawn_file_actions_adddup2(&fa, po[1], 1); posix_spawn_file_actions_adddup2(&fa, pe[1], 2);
    int fds[6] = {pi[0], pi[1], po[0], po[1], pe[0], pe[1]};
    for (int i = 0; i < 6; i++) if (fds[i] > 2) posix_spawn_file_actions_addclose(&fa, fds[i]);
    pid_t pid; int rc = posix_spawnp(&pid, p0, &fa, 0, argv, environ);
    posix_spawn_file_actions_destroy(&fa);
    close(pi[0]); close(po[1]); close(pe[1]);
    if (rc != 0) { close(pi[1]); close(po[0]); close(pe[0]); *why = ss_why(prog, rc); return 0; }
    fcntl(pi[1], F_SETFL, fcntl(pi[1], F_GETFL) | O_NONBLOCK);
    SB_INIT(ob); SB_INIT(eb);
    int64_t off = 0; int win = 1, rout = 1, rerr = 1;
    if (!input.len) { close(pi[1]); win = 0; }
    char buf[65536];
    while (rout || rerr || win) {
        struct pollfd pf[3]; int np = 0, io = -1, ie = -1, iw = -1;
        if (rout) { io = np; pf[np].fd = po[0]; pf[np].events = POLLIN; np++; }
        if (rerr) { ie = np; pf[np].fd = pe[0]; pf[np].events = POLLIN; np++; }
        if (win) { iw = np; pf[np].fd = pi[1]; pf[np].events = POLLOUT; np++; }
        for (int i = 0; i < np; i++) pf[i].revents = 0;
        if (poll(pf, (nfds_t)np, -1) < 0) { if (errno == EINTR) continue; break; }
        if (io >= 0 && pf[io].revents) { ssize_t k = read(po[0], buf, sizeof buf); if (k > 0) sb_put(&ob, buf, k); else if (k == 0 || errno != EINTR) { rout = 0; close(po[0]); } }
        if (ie >= 0 && pf[ie].revents) { ssize_t k = read(pe[0], buf, sizeof buf); if (k > 0) sb_put(&eb, buf, k); else if (k == 0 || errno != EINTR) { rerr = 0; close(pe[0]); } }
        if (iw >= 0 && pf[iw].revents) {
            ssize_t k = write(pi[1], input.p + off, (size_t)(input.len - off));
            if (k > 0) off += k;
            if ((k < 0 && errno != EAGAIN && errno != EINTR) || off >= input.len || (pf[iw].revents & (POLLERR | POLLHUP))) { win = 0; close(pi[1]); }
        }
    }
    int status = 0;
    while (waitpid(pid, &status, 0) < 0 && errno == EINTR) {}
    *code = WIFEXITED(status) ? WEXITSTATUS(status) : WIFSIGNALED(status) ? 128 + WTERMSIG(status) : status;
    *out = sb_done(&ob); *errs = sb_done(&eb);
    if (!ss_utf8_ok((const unsigned char*)out->p, out->len) || !ss_utf8_ok((const unsigned char*)errs->p, errs->len)) { *why = ss_why(prog, -2); return 0; }
    return 1;
}

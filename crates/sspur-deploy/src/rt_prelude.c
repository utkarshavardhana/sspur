
/* sspur deploy runtime: Lambda Runtime API loop, JSON codecs, DynamoDB effect host */
#include <curl/curl.h>
#include <strings.h>
#include <ctype.h>
#include <errno.h>

typedef struct RtChunk { struct RtChunk* next; size_t used, cap; char data[]; } RtChunk;
static RtChunk* rt_arena;
static void* rt_alloc(size_t n) {
    n = (n + 15) & ~(size_t)15;
    if (!rt_arena || rt_arena->used + n > rt_arena->cap) {
        size_t cap = n > 65536 ? n : 65536;
        RtChunk* c = (RtChunk*)malloc(sizeof(RtChunk) + cap);
        c->next = rt_arena; c->used = 0; c->cap = cap; rt_arena = c;
    }
    void* p = rt_arena->data + rt_arena->used; rt_arena->used += n; return p;
}
static void rt_arena_reset(void) { while (rt_arena) { RtChunk* n = rt_arena->next; free(rt_arena); rt_arena = n; } }

typedef struct { char* p; size_t len, cap; } RtB;
static void rt_put(RtB* b, const char* s, size_t n) {
    if (b->len + n + 1 > b->cap) { size_t c = b->cap ? b->cap * 2 : 256; while (c < b->len + n + 1) c *= 2; b->p = (char*)realloc(b->p, c); b->cap = c; }
    memcpy(b->p + b->len, s, n); b->len += n; b->p[b->len] = 0;
}
static void rt_s(RtB* b, const char* s) { rt_put(b, s, strlen(s)); }
static void rt_i64(RtB* b, int64_t v) { char t[32]; int n = snprintf(t, sizeof t, "%lld", (long long)v); rt_put(b, t, (size_t)n); }
static void rt_f64(RtB* b, double v) {
    if (!isfinite(v)) { rt_s(b, "null"); return; }
    char t[40];
    for (int p = 1; p <= 17; p++) { snprintf(t, sizeof t, "%.*g", p, v); if (strtod(t, 0) == v) break; }
    rt_s(b, t);
}
static void rt_jstr(RtB* b, const char* s, size_t n) {
    rt_put(b, "\"", 1);
    size_t run = 0;
    for (size_t i = 0; i < n; i++) {
        unsigned char c = (unsigned char)s[i];
        if (c == '"' || c == '\\' || c < 0x20) {
            if (i > run) rt_put(b, s + run, i - run);
            char e[8];
            if (c == '"' || c == '\\') { e[0] = '\\'; e[1] = (char)c; rt_put(b, e, 2); }
            else if (c == '\n') rt_put(b, "\\n", 2);
            else if (c == '\t') rt_put(b, "\\t", 2);
            else if (c == '\r') rt_put(b, "\\r", 2);
            else { snprintf(e, sizeof e, "\\u%04x", c); rt_put(b, e, 6); }
            run = i + 1;
        }
    }
    if (n > run) rt_put(b, s + run, n - run);
    rt_put(b, "\"", 1);
}

enum { RJ_NULL, RJ_FALSE, RJ_TRUE, RJ_NUM, RJ_STR, RJ_ARR, RJ_OBJ };
typedef struct RtJ { int t; const char* s; int64_t len; struct RtJ* items; const char** keys; int64_t* klens; } RtJ;
typedef struct { const char* p; const char* end; int depth; } RtP;

static void rt_ws(RtP* j) { while (j->p < j->end && (*j->p == ' ' || *j->p == '\t' || *j->p == '\n' || *j->p == '\r')) j->p++; }
static int rt_hex(char c) { return c >= '0' && c <= '9' ? c - '0' : c >= 'a' && c <= 'f' ? c - 'a' + 10 : c >= 'A' && c <= 'F' ? c - 'A' + 10 : -1; }
static int rt_u4(RtP* j, unsigned* cp) {
    if (j->end - j->p < 4) return 0;
    unsigned v = 0;
    for (int i = 0; i < 4; i++) { int h = rt_hex(j->p[i]); if (h < 0) return 0; v = v * 16 + (unsigned)h; }
    j->p += 4; *cp = v; return 1;
}
static int rt_pstr(RtP* j, const char** out, int64_t* len) {
    j->p++;
    const char* q = j->p;
    while (q < j->end && *q != '"') { if (*q == '\\') q++; q++; }
    if (q >= j->end) return 0;
    char* o = (char*)rt_alloc((size_t)(q - j->p) + 1);
    size_t n = 0;
    while (j->p < j->end && *j->p != '"') {
        unsigned char c = (unsigned char)*j->p++;
        if (c < 0x20) return 0;
        if (c != '\\') { o[n++] = (char)c; continue; }
        if (j->p >= j->end) return 0;
        char e = *j->p++;
        switch (e) {
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
            if (!rt_u4(j, &cp)) return 0;
            if (cp >= 0xD800 && cp < 0xDC00) {
                unsigned lo;
                if (j->end - j->p < 6 || j->p[0] != '\\' || j->p[1] != 'u') return 0;
                j->p += 2;
                if (!rt_u4(j, &lo) || lo < 0xDC00 || lo > 0xDFFF) return 0;
                cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
            } else if (cp >= 0xDC00 && cp < 0xE000) return 0;
            if (cp < 0x80) o[n++] = (char)cp;
            else if (cp < 0x800) { o[n++] = (char)(0xC0 | (cp >> 6)); o[n++] = (char)(0x80 | (cp & 0x3F)); }
            else if (cp < 0x10000) { o[n++] = (char)(0xE0 | (cp >> 12)); o[n++] = (char)(0x80 | ((cp >> 6) & 0x3F)); o[n++] = (char)(0x80 | (cp & 0x3F)); }
            else { o[n++] = (char)(0xF0 | (cp >> 18)); o[n++] = (char)(0x80 | ((cp >> 12) & 0x3F)); o[n++] = (char)(0x80 | ((cp >> 6) & 0x3F)); o[n++] = (char)(0x80 | (cp & 0x3F)); }
            break;
        }
        default: return 0;
        }
    }
    j->p++;
    o[n] = 0; *out = o; *len = (int64_t)n;
    return 1;
}
static int rt_value(RtP* j, RtJ* v) {
    rt_ws(j);
    memset(v, 0, sizeof *v);
    if (j->p >= j->end || ++j->depth > 256) return 0;
    char c = *j->p;
    if (c == '{' || c == '[') {
        int obj = c == '{';
        j->p++;
        size_t cap = 8, n = 0;
        RtJ* items = (RtJ*)malloc(cap * sizeof(RtJ));
        const char** keys = obj ? (const char**)malloc(cap * sizeof(char*)) : 0;
        int64_t* klens = obj ? (int64_t*)malloc(cap * sizeof(int64_t)) : 0;
        rt_ws(j);
        int ok = 1;
        if (j->p < j->end && *j->p == (obj ? '}' : ']')) j->p++;
        else for (;;) {
            if (n == cap) { cap *= 2; items = (RtJ*)realloc(items, cap * sizeof(RtJ)); if (obj) { keys = (const char**)realloc(keys, cap * sizeof(char*)); klens = (int64_t*)realloc(klens, cap * sizeof(int64_t)); } }
            if (obj) {
                rt_ws(j);
                if (j->p >= j->end || *j->p != '"' || !rt_pstr(j, &keys[n], &klens[n])) { ok = 0; break; }
                rt_ws(j);
                if (j->p >= j->end || *j->p != ':') { ok = 0; break; }
                j->p++;
            }
            if (!rt_value(j, &items[n])) { ok = 0; break; }
            n++;
            rt_ws(j);
            if (j->p < j->end && *j->p == ',') { j->p++; continue; }
            if (j->p < j->end && *j->p == (obj ? '}' : ']')) { j->p++; break; }
            ok = 0; break;
        }
        if (ok) {
            v->t = obj ? RJ_OBJ : RJ_ARR; v->len = (int64_t)n;
            v->items = (RtJ*)rt_alloc(n * sizeof(RtJ) + 1); memcpy(v->items, items, n * sizeof(RtJ));
            if (obj) {
                v->keys = (const char**)rt_alloc(n * sizeof(char*) + 1); memcpy(v->keys, keys, n * sizeof(char*));
                v->klens = (int64_t*)rt_alloc(n * sizeof(int64_t) + 1); memcpy(v->klens, klens, n * sizeof(int64_t));
            }
        }
        free(items); free(keys); free(klens);
        j->depth--;
        return ok;
    }
    if (c == '"') { v->t = RJ_STR; j->depth--; return rt_pstr(j, &v->s, &v->len); }
    if (c == '-' || (c >= '0' && c <= '9')) {
        const char* s = j->p;
        if (*j->p == '-') j->p++;
        if (j->p >= j->end || !isdigit((unsigned char)*j->p)) return 0;
        if (*j->p == '0') j->p++; else while (j->p < j->end && isdigit((unsigned char)*j->p)) j->p++;
        if (j->p < j->end && *j->p == '.') { j->p++; if (j->p >= j->end || !isdigit((unsigned char)*j->p)) return 0; while (j->p < j->end && isdigit((unsigned char)*j->p)) j->p++; }
        if (j->p < j->end && (*j->p == 'e' || *j->p == 'E')) { j->p++; if (j->p < j->end && (*j->p == '+' || *j->p == '-')) j->p++; if (j->p >= j->end || !isdigit((unsigned char)*j->p)) return 0; while (j->p < j->end && isdigit((unsigned char)*j->p)) j->p++; }
        v->t = RJ_NUM; v->s = s; v->len = j->p - s; j->depth--;
        return 1;
    }
    struct { const char* w; int t; } lits[] = {{"null", RJ_NULL}, {"true", RJ_TRUE}, {"false", RJ_FALSE}};
    for (int i = 0; i < 3; i++) {
        size_t l = strlen(lits[i].w);
        if ((size_t)(j->end - j->p) >= l && memcmp(j->p, lits[i].w, l) == 0) { j->p += l; v->t = lits[i].t; j->depth--; return 1; }
    }
    return 0;
}
static int rt_parse(const char* s, size_t n, RtJ* out) {
    RtP j = {s, s + n, 0};
    if (!rt_value(&j, out)) return 0;
    rt_ws(&j);
    return j.p == j.end;
}
static RtJ* rt_get(RtJ* o, const char* k) {
    if (!o || o->t != RJ_OBJ) return 0;
    size_t kl = strlen(k);
    for (int64_t i = 0; i < o->len; i++) if ((size_t)o->klens[i] == kl && memcmp(o->keys[i], k, kl) == 0) return &o->items[i];
    return 0;
}
static int rt_eq(RtJ* v, const char* s) { return v && v->t == RJ_STR && (size_t)v->len == strlen(s) && memcmp(v->s, s, (size_t)v->len) == 0; }
static void rt_write(RtJ* v, RtB* o) {
    switch (v->t) {
    case RJ_NULL: rt_s(o, "null"); break;
    case RJ_TRUE: rt_s(o, "true"); break;
    case RJ_FALSE: rt_s(o, "false"); break;
    case RJ_NUM: rt_put(o, v->s, (size_t)v->len); break;
    case RJ_STR: rt_jstr(o, v->s, (size_t)v->len); break;
    default:
        rt_s(o, v->t == RJ_OBJ ? "{" : "[");
        for (int64_t i = 0; i < v->len; i++) {
            if (i) rt_s(o, ",");
            if (v->t == RJ_OBJ) { rt_jstr(o, v->keys[i], (size_t)v->klens[i]); rt_s(o, ":"); }
            rt_write(&v->items[i], o);
        }
        rt_s(o, v->t == RJ_OBJ ? "}" : "]");
    }
}

static char rt_errbuf[1024];
static const char* rt_pathv[64];
static int rt_np;
static void rt_push(const char* s) { if (rt_np < 64) rt_pathv[rt_np] = s; rt_np++; }
static void rt_pop(void) { rt_np--; }
static void rt_pushi(int64_t i) { char* s = (char*)rt_alloc(24); snprintf(s, 24, "[%lld]", (long long)i); rt_push(s); }
static int rt_jerr(const char* want, RtJ* v) {
    char path[512]; size_t n = 0; path[0] = 0;
    for (int i = 0; i < rt_np && i < 64; i++) {
        const char* s = rt_pathv[i];
        n += (size_t)snprintf(path + n, sizeof path - n, "%s%s", (i && s[0] != '[') ? "." : "", s);
        if (n >= sizeof path) { n = sizeof path - 1; break; }
    }
    const char* got = !v ? "nothing" : v->t == RJ_NULL ? "null" : v->t == RJ_NUM ? "a number" : v->t == RJ_STR ? "a string" : v->t == RJ_ARR ? "an array" : v->t == RJ_OBJ ? "an object" : "a boolean";
    snprintf(rt_errbuf, sizeof rt_errbuf, "%s: expected %s, found %s", n ? path : "value", want, got);
    return 0;
}
static int rt_num_i64(RtJ* v, int64_t* out) {
    if (!v || v->t != RJ_NUM || v->len > 24) return 0;
    char t[32]; memcpy(t, v->s, (size_t)v->len); t[v->len] = 0;
    for (int64_t i = 0; i < v->len; i++) if (!(isdigit((unsigned char)t[i]) || (i == 0 && t[i] == '-'))) return 0;
    errno = 0; char* e; long long x = strtoll(t, &e, 10);
    if (errno || *e) return 0;
    *out = (int64_t)x; return 1;
}
static int rt_num_f64(RtJ* v, double* out) {
    if (!v || v->t != RJ_NUM || v->len > 400) return 0;
    char t[408]; memcpy(t, v->s, (size_t)v->len); t[v->len] = 0;
    *out = strtod(t, 0); return 1;
}
static void rt_wstr(Buf* b, const char* s, int64_t n) {
    buf_push(b, n);
    for (int64_t i = 0; i < n; i += 8) { int64_t w = 0; memcpy(&w, s + i, (size_t)(n - i < 8 ? n - i : 8)); buf_push(b, w); }
}
static int rt_utf8_ok(const char* s, int64_t n) {
    const unsigned char* p = (const unsigned char*)s;
    int64_t i = 0;
    while (i < n) {
        unsigned char c = p[i];
        int k = c < 0x80 ? 0 : (c & 0xE0) == 0xC0 ? 1 : (c & 0xF0) == 0xE0 ? 2 : (c & 0xF8) == 0xF0 ? 3 : -1;
        if (k < 0 || i + k >= n + (k == 0)) return 0;
        for (int j = 1; j <= k; j++) if ((p[i + j] & 0xC0) != 0x80) return 0;
        i += k + 1;
    }
    return 1;
}

static int64_t rt_digits(double x, char* dg, int* e10) {
    char t[48]; int p;
    for (p = 1; p <= 17; p++) { snprintf(t, sizeof t, "%.*e", p - 1, x); if (strtod(t, 0) == x) break; }
    int64_t n = 0; const char* q = t;
    while (*q && *q != 'e') { if (*q >= '0' && *q <= '9') dg[n++] = *q; q++; }
    *e10 = atoi(q + 1);
    while (n > 1 && dg[n - 1] == '0') n--;
    return n;
}
static int64_t rt_fmt(double x, char* out, int debug) {
    if (isnan(x)) return sprintf(out, "NaN");
    if (isinf(x)) return sprintf(out, x < 0 ? "-inf" : "inf");
    char* o = out;
    if (signbit(x)) { *o++ = '-'; x = -x; }
    if (x == 0) return (o - out) + sprintf(o, debug ? "0.0" : "0");
    char dg[24]; int e; int64_t n = rt_digits(x, dg, &e);
    if (debug && (e < -4 || e >= 16)) {
        *o++ = dg[0];
        if (n > 1) { *o++ = '.'; memcpy(o, dg + 1, (size_t)n - 1); o += n - 1; }
        o += sprintf(o, "e%d", e);
        return o - out;
    }
    if (e >= 0) {
        for (int i = 0; i <= e; i++) *o++ = i < n ? dg[i] : '0';
        if (n > e + 1) { *o++ = '.'; for (int64_t i = e + 1; i < n; i++) *o++ = dg[i]; }
        else if (debug) { *o++ = '.'; *o++ = '0'; }
    } else {
        *o++ = '0'; *o++ = '.';
        for (int i = 0; i < -e - 1; i++) *o++ = '0';
        memcpy(o, dg, (size_t)n); o += n;
    }
    return o - out;
}
static int64_t rt_fmt_dbg(double x, char* out) { return rt_fmt(x, out, 1); }
static int64_t rt_fmt_disp(double x, char* out) { return rt_fmt(x, out, 0); }
static int64_t rt_str_op(int64_t op, const char* p, int64_t len, char* out, int64_t cap) {
    int64_t n = 0;
    if (op == 3) {
        if (n < cap) out[n++] = '"';
        for (int64_t i = 0; i < len && n + 8 < cap; i++) {
            char c = p[i];
            if (c == '"' || c == '\\') { out[n++] = '\\'; out[n++] = c; }
            else if (c == '\n') { out[n++] = '\\'; out[n++] = 'n'; }
            else if (c == '\t') { out[n++] = '\\'; out[n++] = 't'; }
            else if (c == '\r') { out[n++] = '\\'; out[n++] = 'r'; }
            else if (c == 0) { out[n++] = '\\'; out[n++] = '0'; }
            else out[n++] = c;
        }
        if (n < cap) out[n++] = '"';
        return n;
    }
    for (int64_t i = 0; i < len && n < cap; i++) out[n++] = (char)(op == 1 ? tolower((unsigned char)p[i]) : toupper((unsigned char)p[i]));
    return n;
}
static int rt_alnum(unsigned char c) { return c >= 0x80 || isalnum(c); }
static int64_t rt_str_class(int64_t op, const char* p, int64_t len) {
    (void)op;
    if (!len) return 0;
    for (int64_t i = 0; i < len; i++) { unsigned char c = (unsigned char)p[i]; if (!(c >= 0x80 || isalpha(c))) return 0; }
    return 1;
}
static int64_t rt_str_spans(int64_t op, const char* p, int64_t len, int64_t* out, int64_t cap) {
    int64_t n = 0;
    if (op == 2) {
        int64_t a = 0, b = len;
        while (a < b && isspace((unsigned char)p[a])) a++;
        while (b > a && isspace((unsigned char)p[b - 1])) b--;
        if (cap >= 2) { out[0] = a; out[1] = b - a; n = 2; }
        return n;
    }
    for (int64_t i = 0; i < len;) {
        while (i < len && !rt_alnum((unsigned char)p[i])) i++;
        int64_t s = i;
        while (i < len && rt_alnum((unsigned char)p[i])) i++;
        if (i > s && n + 2 <= cap) { out[n++] = s; out[n++] = i - s; }
    }
    return n;
}
static void rt_log(const char* p, int64_t len) { fwrite(p, 1, (size_t)len, stdout); fputc('\n', stdout); fflush(stdout); }

static int64_t rt_b64(const char* s, int64_t n, char* out) {
    int64_t o = 0; unsigned v = 0; int bits = 0;
    for (int64_t i = 0; i < n; i++) {
        char c = s[i]; int d;
        if (c >= 'A' && c <= 'Z') d = c - 'A'; else if (c >= 'a' && c <= 'z') d = c - 'a' + 26; else if (c >= '0' && c <= '9') d = c - '0' + 52;
        else if (c == '+' || c == '-') d = 62; else if (c == '/' || c == '_') d = 63; else if (c == '=' || c == '\n' || c == '\r') continue; else return -1;
        v = (v << 6) | (unsigned)d; bits += 6;
        if (bits >= 8) { bits -= 8; out[o++] = (char)((v >> bits) & 0xFF); }
    }
    return o;
}

typedef int64_t (*RtEntry)(const int64_t* in, Status* st, int64_t** out, int64_t* out_len);
typedef struct { const char* sv; int (*dec)(RtJ* v, Buf* b); void (*enc)(const int64_t** p, RtB* o); RtEntry fn; } RtMig;
typedef struct { const char* name; void (*key)(const int64_t** p, RtB* o); void (*enc)(const int64_t** p, RtB* o); int (*dec)(RtJ* v, Buf* b); const char* type; const char* sv; const RtMig* migs; int nmigs; const RtMig* backs; int nbacks; } RtStore;
typedef struct {
    const char* name;
    const char* const* routes;
    int nroutes;
    int (*args)(RtJ* ev, Buf* b);
    int64_t (*validate)(const int64_t* in, Status* st, int64_t** out, int64_t* out_len);
    int64_t (*entry)(const int64_t* in, Status* st, int64_t** out, int64_t* out_len);
    void (*enc)(const int64_t** p, RtB* o);
    int kind;
    int ok;
} RtHandler;
static const RtStore* rt_store(const char* name);
static int rt_err_json(int64_t ty, const int64_t* p, RtB* o);
static const char* rt_refine_text(int64_t clause);
static const RtHandler* rt_handler_named(const char* name);
static const RtHandler* rt_handler_for(RtJ* route);

static struct { const char* api; const char* region; const char* endpoint; const char* akid; const char* secret; const char* token; char sig[96]; } rt_cfg;
static CURL* rt_lc;
static CURL* rt_dc;
static char rt_reqid[256];

static size_t rt_wr(char* d, size_t sz, size_t n, void* u) { rt_put((RtB*)u, d, sz * n); return sz * n; }
static size_t rt_hdr(char* d, size_t sz, size_t n, void* u) {
    (void)u;
    size_t len = sz * n; const char* k = "lambda-runtime-aws-request-id:"; size_t kl = strlen(k);
    if (len > kl && strncasecmp(d, k, kl) == 0) {
        const char* v = d + kl; size_t vl = len - kl;
        while (vl && (*v == ' ' || *v == '\t')) { v++; vl--; }
        while (vl && (v[vl - 1] == '\r' || v[vl - 1] == '\n' || v[vl - 1] == ' ')) vl--;
        if (vl >= sizeof rt_reqid) vl = sizeof rt_reqid - 1;
        memcpy(rt_reqid, v, vl); rt_reqid[vl] = 0;
    }
    return len;
}
static int rt_http(CURL* h, const char* url, const char* body, size_t n, struct curl_slist* hd, RtB* out, long* code, int sign, long timeout) {
    curl_easy_reset(h);
    curl_easy_setopt(h, CURLOPT_URL, url);
    curl_easy_setopt(h, CURLOPT_NOSIGNAL, 1L);
    curl_easy_setopt(h, CURLOPT_TIMEOUT, timeout);
    if (body) { curl_easy_setopt(h, CURLOPT_POST, 1L); curl_easy_setopt(h, CURLOPT_POSTFIELDS, body); curl_easy_setopt(h, CURLOPT_POSTFIELDSIZE_LARGE, (curl_off_t)n); }
    if (hd) curl_easy_setopt(h, CURLOPT_HTTPHEADER, hd);
    curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, rt_wr);
    curl_easy_setopt(h, CURLOPT_WRITEDATA, out);
    curl_easy_setopt(h, CURLOPT_HEADERFUNCTION, rt_hdr);
    if (sign) {
        curl_easy_setopt(h, CURLOPT_AWS_SIGV4, rt_cfg.sig);
        curl_easy_setopt(h, CURLOPT_USERNAME, rt_cfg.akid);
        curl_easy_setopt(h, CURLOPT_PASSWORD, rt_cfg.secret);
    }
    CURLcode rc = curl_easy_perform(h);
    *code = 0;
    curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, code);
    return rc == CURLE_OK;
}

static int rt_ddb(const char* op, RtB* req, RtB* resp, char* err, size_t cap) {
    if (!rt_cfg.akid || !rt_cfg.secret) { snprintf(err, cap, "no AWS credentials in the environment for DynamoDB %s", op); return 0; }
    char target[96], tok[2048];
    snprintf(target, sizeof target, "X-Amz-Target: DynamoDB_20120810.%s", op);
    struct curl_slist* hd = curl_slist_append(0, "Content-Type: application/x-amz-json-1.0");
    hd = curl_slist_append(hd, target);
    hd = curl_slist_append(hd, "Expect:");
    if (rt_cfg.token && *rt_cfg.token) { snprintf(tok, sizeof tok, "X-Amz-Security-Token: %s", rt_cfg.token); hd = curl_slist_append(hd, tok); }
    int ok = 0;
    for (int attempt = 0; attempt < 5; attempt++) {
        resp->len = 0;
        long code = 0;
        if (!rt_http(rt_dc, rt_cfg.endpoint, req->p, req->len, hd, resp, &code, 1, 10)) {
            snprintf(err, cap, "DynamoDB %s: cannot reach %s", op, rt_cfg.endpoint);
        } else if (code == 200) {
            ok = 1; break;
        } else {
            RtJ r; const char* type = "?"; const char* msg = "";
            if (resp->len && rt_parse(resp->p, resp->len, &r)) {
                RtJ* t = rt_get(&r, "__type"); RtJ* m = rt_get(&r, "message"); if (!m) m = rt_get(&r, "Message");
                if (t && t->t == RJ_STR) { type = t->s; const char* h = strrchr(type, '#'); if (h) type = h + 1; }
                if (m && m->t == RJ_STR) msg = m->s;
            }
            snprintf(err, cap, "DynamoDB %s failed (HTTP %ld): %s: %s", op, code, type, msg);
            int retry = code >= 500 || strstr(type, "Throttl") || strstr(type, "ProvisionedThroughputExceeded") || strstr(type, "RequestLimitExceeded");
            if (!retry) break;
        }
        usleep((useconds_t)(25000u << attempt));
    }
    curl_slist_free_all(hd);
    return ok;
}

static int64_t rt_dberr(int64_t** out, int64_t* on, const char* msg) {
    size_t n = strlen(msg);
    char* m = (char*)malloc(n + 1); memcpy(m, msg, n + 1);
    *out = (int64_t*)m; *on = (int64_t)n;
    return 1;
}
static void rt_trap_text(Status* st, char* out, size_t cap);
static int rt_run(RtEntry fn, Buf* in, Buf* ob, char* err, size_t cap, const char* what) {
    buf_push(in, 0);
    Status st; memset(&st, 0, sizeof st); st.limit = 10000;
    int64_t* o = 0; int64_t n = 0;
    int64_t c = fn(in->data, &st, &o, &n);
    if (c) { char m[512]; rt_trap_text(&st, m, sizeof m); snprintf(err, cap, "%s failed: %s", what, m); return 0; }
    for (int64_t i = 0; i < n; i++) buf_push(ob, o[i]);
    free(o);
    return 1;
}
static RtJ* rt_attr_doc(RtJ* item, const char* attr, RtJ* doc) {
    RtJ* v = rt_get(rt_get(item, attr), "S");
    if (!v || v->t != RJ_STR || !rt_parse(v->s, (size_t)v->len, doc)) return 0;
    return doc;
}
static int rt_migrate(const RtStore* s, const RtMig* m, RtJ* doc, Buf* ob, char* err, size_t cap) {
    Buf in = {0};
    rt_np = 0;
    if (!m->dec(doc, &in)) { snprintf(err, cap, "store %s holds an item of schema %s that does not decode (%s)", s->name, m->sv, rt_errbuf); free(in.data); return 0; }
    char what[160]; snprintf(what, sizeof what, "migration of a %s item from schema %s", s->name, m->sv);
    int ok = rt_run(m->fn, &in, ob, err, cap, what);
    free(in.data);
    return ok;
}
static int rt_item_val(const RtStore* s, RtJ* item, Buf* ob, char* err, size_t cap) {
    RtJ doc;
    char own[64]; snprintf(own, sizeof own, "v_%s", s->sv);
    int64_t mark = ob->len;
    if (rt_attr_doc(item, own, &doc)) { rt_np = 0; if (s->dec(&doc, ob)) return 1; ob->len = mark; }
    if (!rt_attr_doc(item, "v", &doc)) { snprintf(err, cap, "store %s holds an item without a JSON document", s->name); return 0; }
    RtJ* sv = rt_get(rt_get(item, "sv"), "S");
    if (sv && sv->t == RJ_STR && !rt_eq(sv, s->sv))
        for (int i = 0; i < s->nmigs; i++) if (rt_eq(sv, s->migs[i].sv)) return rt_migrate(s, &s->migs[i], &doc, ob, err, cap);
    rt_np = 0;
    if (s->dec(&doc, ob)) return 1;
    ob->len = mark;
    if (!sv) for (int i = 0; i < s->nmigs; i++) {
        Buf t = {0}; rt_np = 0;
        int fits = s->migs[i].dec(&doc, &t);
        free(t.data);
        if (fits) return rt_migrate(s, &s->migs[i], &doc, ob, err, cap);
    }
    snprintf(err, cap, "store %s holds an item that is not a %s (%s); a migration is needed", s->name, s->type, rt_errbuf);
    return 0;
}
static int rt_item_body(const RtStore* s, const int64_t* v, int64_t vn, RtB* req, char* err, size_t cap) {
    RtB val = {0};
    const int64_t* vp = v; s->enc(&vp, &val);
    rt_s(req, ",\"sv\":{\"S\":\""); rt_s(req, s->sv); rt_s(req, "\"},\"v\":{\"S\":"); rt_jstr(req, val.p, val.len); rt_s(req, "}");
    free(val.p);
    for (int i = 0; i < s->nbacks; i++) {
        Buf in = {0}, ob = {0};
        for (int64_t j = 0; j < vn; j++) buf_push(&in, v[j]);
        char what[160]; snprintf(what, sizeof what, "reverse migration of a %s item to schema %s", s->name, s->backs[i].sv);
        int ok = rt_run(s->backs[i].fn, &in, &ob, err, cap, what);
        free(in.data);
        if (!ok) { free(ob.data); return 0; }
        RtB old = {0};
        const int64_t* op = ob.data; s->backs[i].enc(&op, &old);
        rt_s(req, ",\"v_"); rt_s(req, s->backs[i].sv); rt_s(req, "\":{\"S\":"); rt_jstr(req, old.p, old.len); rt_s(req, "}");
        free(old.p); free(ob.data);
    }
    return 1;
}
static int64_t rt_db(int64_t op, const char* store, const int64_t* k, int64_t kn, const int64_t* v, int64_t vn, int64_t** out, int64_t* on) {
    (void)kn;
    char err[1024];
    const RtStore* s = rt_store(store);
    if (!s) { snprintf(err, sizeof err, "unknown store %s", store); return rt_dberr(out, on, err); }
    char envn[160]; snprintf(envn, sizeof envn, "SSPUR_TABLE_%s", store);
    const char* table = getenv(envn);
    if (!table || !*table) { snprintf(err, sizeof err, "store %s has no table: %s is not set (the handler's effect row does not grant it)", store, envn); return rt_dberr(out, on, err); }
    RtB req = {0}, resp = {0}, key = {0}, val = {0};
    Buf ob = {0};
    int ok = 1;
    if (op != 4) { const int64_t* kp = k; s->key(&kp, &key); }
    rt_s(&req, "{\"TableName\":"); rt_jstr(&req, table, strlen(table));
    if (op == 1 || op == 3) {
        rt_s(&req, ",\"Key\":{\"pk\":"); rt_put(&req, key.p, key.len); rt_s(&req, "}");
        rt_s(&req, op == 1 ? ",\"ConsistentRead\":true}" : ",\"ReturnValues\":\"ALL_OLD\"}");
        ok = rt_ddb(op == 1 ? "GetItem" : "DeleteItem", &req, &resp, err, sizeof err);
        RtJ r;
        if (ok && !rt_parse(resp.p, resp.len, &r)) { snprintf(err, sizeof err, "DynamoDB returned invalid JSON"); ok = 0; }
        if (ok && op == 1) {
            RtJ* item = rt_get(&r, "Item");
            if (!item) buf_push(&ob, 0); else { buf_push(&ob, 1); ok = rt_item_val(s, item, &ob, err, sizeof err); }
        } else if (ok) buf_push(&ob, rt_get(&r, "Attributes") ? 1 : 0);
    } else if (op == 2) {
        rt_s(&req, ",\"Item\":{\"pk\":"); rt_put(&req, key.p, key.len);
        ok = rt_item_body(s, v, vn, &req, err, sizeof err);
        rt_s(&req, "}}");
        if (ok) ok = rt_ddb("PutItem", &req, &resp, err, sizeof err);
        buf_push(&ob, 0);
    } else {
        buf_push(&ob, 0);
        int64_t count = 0;
        RtB start = {0};
        for (;;) {
            req.len = 0;
            rt_s(&req, "{\"TableName\":"); rt_jstr(&req, table, strlen(table)); rt_s(&req, ",\"ConsistentRead\":true");
            if (start.len) { rt_s(&req, ",\"ExclusiveStartKey\":"); rt_put(&req, start.p, start.len); }
            rt_s(&req, "}");
            if (!(ok = rt_ddb("Scan", &req, &resp, err, sizeof err))) break;
            RtJ r;
            if (!rt_parse(resp.p, resp.len, &r)) { snprintf(err, sizeof err, "DynamoDB returned invalid JSON"); ok = 0; break; }
            RtJ* items = rt_get(&r, "Items");
            for (int64_t i = 0; items && items->t == RJ_ARR && i < items->len && ok; i++) { ok = rt_item_val(s, &items->items[i], &ob, err, sizeof err); count++; }
            if (!ok) break;
            RtJ* last = rt_get(&r, "LastEvaluatedKey");
            if (!last) break;
            start.len = 0; rt_write(last, &start);
        }
        free(start.p);
        if (ok) ob.data[0] = count;
    }
    free(req.p); free(resp.p); free(key.p); free(val.p);
    if (!ok) { free(ob.data); return rt_dberr(out, on, err); }
    *out = ob.data; *on = ob.len;
    return 0;
}

static void rt_trap_text(Status* st, char* out, size_t cap) {
    static const char* names[] = {"", "integer overflow", "division by zero", "precondition failed", "postcondition failed", "bad argument", "negative exponent", "recursion too deep", "index out of range", "unwrapped none", "no match", "refinement violated", "", "bad repeat count", "confidence out of range", "out of memory"};
    if (st->code == 12 && st->rbuf) { snprintf(out, cap, "%.*s", (int)st->rlen, (const char*)st->rbuf); free(st->rbuf); st->rbuf = 0; return; }
    if (st->code == 11) { const char* t = rt_refine_text(st->clause); snprintf(out, cap, "contract violated: %s", t ? t : "refinement"); }
    else snprintf(out, cap, "%s", st->code > 0 && st->code < 16 ? names[st->code] : "trap");
    if (st->rbuf) { free(st->rbuf); st->rbuf = 0; }
}

static void rt_reply(RtB* out, int status, const char* body, size_t n) {
    rt_s(out, "{\"statusCode\":"); rt_i64(out, status);
    rt_s(out, ",\"headers\":{\"content-type\":\"application/json\"},\"body\":");
    rt_jstr(out, body ? body : "", body ? n : 0);
    rt_s(out, "}");
}
static void rt_reply_err(RtB* out, int status, const char* error, const char* detail) {
    RtB b = {0};
    rt_s(&b, "{\"error\":"); rt_jstr(&b, error, strlen(error));
    if (detail) { rt_s(&b, ",\"detail\":"); rt_jstr(&b, detail, strlen(detail)); }
    rt_s(&b, "}");
    rt_reply(out, status, b.p, b.len);
    free(b.p);
}

static int rt_body(RtJ* ev, RtJ** out) {
    RtJ* b = rt_get(ev, "body");
    if (!b || b->t != RJ_STR || b->len == 0) { *out = 0; return 1; }
    const char* s = b->s; int64_t n = b->len;
    RtJ* enc = rt_get(ev, "isBase64Encoded");
    if (enc && enc->t == RJ_TRUE) { char* d = (char*)rt_alloc((size_t)n + 1); n = rt_b64(s, n, d); if (n < 0) return 0; s = d; }
    RtJ* v = (RtJ*)rt_alloc(sizeof(RtJ));
    if (!rt_parse(s, (size_t)n, v)) return 0;
    *out = v;
    return 1;
}
static RtJ* rt_pathp(RtJ* ev, const char* name, int num) {
    RtJ* v = rt_get(rt_get(ev, "pathParameters"), name);
    if (!v || v->t != RJ_STR) return 0;
    if (!num) return v;
    RtJ* n = (RtJ*)rt_alloc(sizeof(RtJ)); *n = *v; n->t = RJ_NUM;
    return n;
}

static const RtStore* rt_bf;
static void rt_backfill(RtJ* ev, RtB* out) {
    const RtStore* s = rt_bf;
    char err[1024], envn[160];
    snprintf(envn, sizeof envn, "SSPUR_TABLE_%s", s->name);
    const char* table = getenv(envn);
    RtB req = {0}, resp = {0}, errs = {0};
    int64_t scanned = 0, migrated = 0, current = 0, skipped = 0, failed = 0, limit = 100;
    RtJ* lim = rt_get(ev, "limit");
    if (lim) rt_num_i64(lim, &limit);
    rt_s(&req, "{\"TableName\":"); rt_jstr(&req, table ? table : "", table ? strlen(table) : 0);
    rt_s(&req, ",\"ConsistentRead\":true,\"Limit\":"); rt_i64(&req, limit);
    RtJ* start = rt_get(ev, "start");
    if (start && start->t == RJ_OBJ) { rt_s(&req, ",\"ExclusiveStartKey\":"); rt_write(start, &req); }
    rt_s(&req, "}");
    RtJ r;
    if (!table || !rt_ddb("Scan", &req, &resp, err, sizeof err) || !rt_parse(resp.p, resp.len, &r)) {
        if (!table) snprintf(err, sizeof err, "%s is not set", envn);
        rt_s(out, "{\"error\":"); rt_jstr(out, err, strlen(err)); rt_s(out, "}");
        free(req.p); free(resp.p);
        return;
    }
    RtJ* items = rt_get(&r, "Items");
    for (int64_t i = 0; items && items->t == RJ_ARR && i < items->len; i++) {
        RtJ* item = &items->items[i];
        scanned++;
        RtJ* sv = rt_get(rt_get(item, "sv"), "S");
        int done = sv && rt_eq(sv, s->sv);
        for (int b = 0; done && b < s->nbacks; b++) { char a[64]; snprintf(a, sizeof a, "v_%s", s->backs[b].sv); done = rt_get(item, a) != 0; }
        if (done) { current++; continue; }
        Buf ob = {0};
        RtB put = {0}, pr = {0};
        int ok = rt_item_val(s, item, &ob, err, sizeof err);
        if (ok) {
            rt_s(&put, "{\"TableName\":"); rt_jstr(&put, table, strlen(table)); rt_s(&put, ",\"Item\":{\"pk\":"); rt_write(rt_get(item, "pk"), &put);
            ok = rt_item_body(s, ob.data, ob.len, &put, err, sizeof err);
        }
        if (ok) {
            rt_s(&put, "},\"ConditionExpression\":\"#v = :v\",\"ExpressionAttributeNames\":{\"#v\":\"v\"},\"ExpressionAttributeValues\":{\":v\":"); rt_write(rt_get(item, "v"), &put); rt_s(&put, "}}");
            ok = rt_ddb("PutItem", &put, &pr, err, sizeof err);
            if (ok) migrated++;
            else if (strstr(err, "ConditionalCheckFailed")) { skipped++; ok = 1; }
        }
        if (!ok) { failed++; if (failed <= 5) { rt_s(&errs, errs.len ? "," : ""); rt_jstr(&errs, err, strlen(err)); } }
        free(ob.data); free(put.p); free(pr.p);
    }
    rt_s(out, "{\"store\":"); rt_jstr(out, s->name, strlen(s->name));
    rt_s(out, ",\"schema\":\""); rt_s(out, s->sv);
    rt_s(out, "\",\"scanned\":"); rt_i64(out, scanned);
    rt_s(out, ",\"migrated\":"); rt_i64(out, migrated);
    rt_s(out, ",\"current\":"); rt_i64(out, current);
    rt_s(out, ",\"skipped\":"); rt_i64(out, skipped);
    rt_s(out, ",\"failed\":"); rt_i64(out, failed);
    rt_s(out, ",\"errors\":["); if (errs.len) rt_put(out, errs.p, errs.len); rt_s(out, "]");
    rt_s(out, ",\"next\":"); RtJ* last = rt_get(&r, "LastEvaluatedKey"); if (last) rt_write(last, out); else rt_s(out, "null");
    rt_s(out, "}");
    free(req.p); free(resp.p); free(errs.p);
}

static void rt_invoke(const RtHandler* only, const char* evs, size_t evn, RtB* out, int* status) {
    RtJ ev;
    *status = 500;
    if (!rt_parse(evs, evn, &ev)) { rt_reply_err(out, 400, "bad event", 0); *status = 400; return; }
    if (rt_bf) { rt_backfill(&ev, out); *status = 200; return; }
    RtJ* rk = rt_get(&ev, "routeKey");
    const RtHandler* h = rt_handler_for(rk);
    if (!h || (only && h != only)) { rt_reply_err(out, 404, "no route", rk && rk->t == RJ_STR ? rk->s : 0); *status = 404; return; }
    Buf in = {0};
    rt_np = 0;
    if (!h->args(&ev, &in)) { free(in.data); rt_reply_err(out, 400, "invalid request", rt_errbuf); *status = 400; return; }
    buf_push(&in, 0);
    char msg[1024];
    if (h->validate) {
        Status vs; memset(&vs, 0, sizeof vs); vs.limit = 10000;
        int64_t* vo = 0; int64_t vl = 0;
        int64_t c = h->validate(in.data, &vs, &vo, &vl);
        free(in.data);
        if (c) { rt_trap_text(&vs, msg, sizeof msg); rt_reply_err(out, 400, "invalid request", msg); *status = 400; return; }
        in.data = vo; in.len = vl; in.cap = vl;
        buf_push(&in, 0);
    }
    Status st; memset(&st, 0, sizeof st); st.limit = 10000;
    int64_t* ro = 0; int64_t rl = 0;
    int64_t c = h->entry(in.data, &st, &ro, &rl);
    free(in.data);
    if (c == 0) {
        const int64_t* p = ro;
        RtB b = {0};
        if (h->kind == 2) { *status = 204; rt_reply(out, 204, 0, 0); }
        else if (h->kind == 1 && !*p) { *status = 404; rt_reply_err(out, 404, "not found", 0); }
        else {
            if (h->kind == 1) p++;
            h->enc(&p, &b);
            *status = h->ok;
            rt_reply(out, h->ok, b.p, b.len);
        }
        free(b.p); free(ro);
        return;
    }
    if (c == 100) {
        RtB b = {0};
        rt_s(&b, "{\"error\":");
        *status = rt_err_json(st.err_type, st.rbuf, &b);
        rt_s(&b, "}");
        free(st.rbuf);
        rt_reply(out, *status, b.p, b.len);
        free(b.p);
        return;
    }
    rt_trap_text(&st, msg, sizeof msg);
    printf("ERROR %s %s: %s\n", rt_reqid, h->name, msg);
    rt_reply_err(out, 500, "internal error", 0);
}

static void rt_post(const char* path, const char* body, size_t n) {
    char url[512]; snprintf(url, sizeof url, "http://%s/2018-06-01/runtime/%s", rt_cfg.api, path);
    struct curl_slist* hd = curl_slist_append(0, "Content-Type: application/json");
    hd = curl_slist_append(hd, "Expect:");
    RtB r = {0}; long code;
    rt_http(rt_lc, url, body, n, hd, &r, &code, 0, 30);
    curl_slist_free_all(hd); free(r.p);
}

int main(void) {
    setvbuf(stdout, 0, _IOLBF, 0);
    curl_global_init(CURL_GLOBAL_DEFAULT);
    static HostApi host = {rt_fmt_dbg, rt_fmt_disp, rt_str_op, rt_str_class, rt_str_spans, rt_log};
    sspur_set_host(&host);
    sspur_db = rt_db;
    rt_cfg.api = getenv("AWS_LAMBDA_RUNTIME_API");
    rt_cfg.region = getenv("AWS_REGION");
    if (!rt_cfg.region || !*rt_cfg.region) rt_cfg.region = "us-east-1";
    rt_cfg.akid = getenv("AWS_ACCESS_KEY_ID");
    rt_cfg.secret = getenv("AWS_SECRET_ACCESS_KEY");
    rt_cfg.token = getenv("AWS_SESSION_TOKEN");
    static char ep[256];
    const char* e = getenv("AWS_ENDPOINT_URL_DYNAMODB");
    if (e && *e) snprintf(ep, sizeof ep, "%s", e); else snprintf(ep, sizeof ep, "https://dynamodb.%s.amazonaws.com", rt_cfg.region);
    rt_cfg.endpoint = ep;
    snprintf(rt_cfg.sig, sizeof rt_cfg.sig, "aws:amz:%s:dynamodb", rt_cfg.region);
    if (!rt_cfg.api) { fprintf(stderr, "AWS_LAMBDA_RUNTIME_API is not set\n"); return 2; }
    rt_lc = curl_easy_init(); rt_dc = curl_easy_init();
    const RtHandler* only = 0;
    const char* hn = getenv("SSPUR_HANDLER");
    const char* bf = getenv("SSPUR_BACKFILL");
    if (bf && *bf && !(rt_bf = rt_store(bf))) {
        char b[256]; int n = snprintf(b, sizeof b, "{\"errorMessage\":\"unknown store %s\",\"errorType\":\"Runtime.HandlerNotFound\"}", bf);
        rt_post("init/error", b, (size_t)n);
        return 1;
    }
    if (hn && *hn && !(only = rt_handler_named(hn))) {
        char b[256]; int n = snprintf(b, sizeof b, "{\"errorMessage\":\"unknown handler %s\",\"errorType\":\"Runtime.HandlerNotFound\"}", hn);
        rt_post("init/error", b, (size_t)n);
        return 1;
    }
    char url[512]; snprintf(url, sizeof url, "http://%s/2018-06-01/runtime/invocation/next", rt_cfg.api);
    for (;;) {
        RtB ev = {0}; long code = 0;
        rt_reqid[0] = 0;
        if (!rt_http(rt_lc, url, 0, 0, 0, &ev, &code, 0, 0) || code != 200 || !rt_reqid[0]) { free(ev.p); return 1; }
        struct timespec t0, t1; clock_gettime(CLOCK_MONOTONIC, &t0);
        RtB out = {0}; int status = 500;
        rt_invoke(only, ev.p ? ev.p : "", ev.len, &out, &status);
        clock_gettime(CLOCK_MONOTONIC, &t1);
        char path[400]; snprintf(path, sizeof path, "invocation/%s/response", rt_reqid);
        rt_post(path, out.p, out.len);
        printf("REQ %s %d %.2fms\n", rt_reqid, status, (double)(t1.tv_sec - t0.tv_sec) * 1e3 + (double)(t1.tv_nsec - t0.tv_nsec) / 1e6);
        free(ev.p); free(out.p);
        rt_arena_reset();
    }
}

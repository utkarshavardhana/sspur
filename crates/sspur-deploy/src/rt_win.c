/* Windows host build: a plain-HTTP subset of libcurl over Winsock, enough for the local Lambda and DynamoDB emulators */
#include <ctype.h>
#include <stdarg.h>
#define strncasecmp _strnicmp
#define strcasecmp _stricmp
typedef unsigned useconds_t;
#define usleep(us) Sleep((unsigned long)((us) / 1000))
__declspec(dllimport) int __stdcall WSAStartup(unsigned short ver, void* data);
__declspec(dllimport) uintptr_t __stdcall socket(int af, int type, int proto);
__declspec(dllimport) int __stdcall connect(uintptr_t s, const void* addr, int len);
__declspec(dllimport) int __stdcall send(uintptr_t s, const char* b, int n, int fl);
__declspec(dllimport) int __stdcall recv(uintptr_t s, char* b, int n, int fl);
__declspec(dllimport) int __stdcall closesocket(uintptr_t s);
__declspec(dllimport) int __stdcall setsockopt(uintptr_t s, int level, int opt, const char* v, int len);
typedef long long curl_off_t;
typedef int CURLcode;
#define CURLE_OK 0
#define CURLE_FAILED 7
enum { CURLOPT_URL = 1, CURLOPT_NOSIGNAL, CURLOPT_TIMEOUT, CURLOPT_POST, CURLOPT_POSTFIELDS, CURLOPT_POSTFIELDSIZE_LARGE, CURLOPT_HTTPHEADER, CURLOPT_WRITEFUNCTION, CURLOPT_WRITEDATA, CURLOPT_HEADERFUNCTION, CURLOPT_HEADERDATA, CURLOPT_AWS_SIGV4, CURLOPT_USERNAME, CURLOPT_PASSWORD };
#define CURLINFO_RESPONSE_CODE 1
#define CURL_GLOBAL_DEFAULT 3
struct curl_slist { char* data; struct curl_slist* next; };
typedef size_t (*rt_w_cb)(char*, size_t, size_t, void*);
typedef struct { const char* url; const char* body; long long n; int post; long timeout; struct curl_slist* hd; rt_w_cb wf, hf; void* wd; void* hdd; const char* sig; const char* user; long code; } CURL;
static CURLcode curl_global_init(long fl) { (void)fl; char d[1024]; return WSAStartup(0x0202, d) == 0 ? CURLE_OK : CURLE_FAILED; }
static CURL* curl_easy_init(void) { return (CURL*)calloc(1, sizeof(CURL)); }
static void curl_easy_reset(CURL* h) { memset(h, 0, sizeof *h); h->n = -1; }
static CURLcode curl_easy_setopt(CURL* h, int opt, ...) {
    va_list a; va_start(a, opt);
    switch (opt) {
    case CURLOPT_URL: h->url = va_arg(a, const char*); break;
    case CURLOPT_TIMEOUT: h->timeout = va_arg(a, long); break;
    case CURLOPT_POST: h->post = (int)va_arg(a, long); break;
    case CURLOPT_POSTFIELDS: h->body = va_arg(a, const char*); h->post = 1; break;
    case CURLOPT_POSTFIELDSIZE_LARGE: h->n = va_arg(a, curl_off_t); break;
    case CURLOPT_HTTPHEADER: h->hd = va_arg(a, struct curl_slist*); break;
    case CURLOPT_WRITEFUNCTION: h->wf = va_arg(a, rt_w_cb); break;
    case CURLOPT_WRITEDATA: h->wd = va_arg(a, void*); break;
    case CURLOPT_HEADERFUNCTION: h->hf = va_arg(a, rt_w_cb); break;
    case CURLOPT_HEADERDATA: h->hdd = va_arg(a, void*); break;
    case CURLOPT_AWS_SIGV4: h->sig = va_arg(a, const char*); break;
    case CURLOPT_USERNAME: h->user = va_arg(a, const char*); break;
    default: (void)va_arg(a, void*); break;
    }
    va_end(a); return CURLE_OK;
}
static CURLcode curl_easy_getinfo(CURL* h, int what, ...) { va_list a; va_start(a, what); long* p = va_arg(a, long*); if (what == CURLINFO_RESPONSE_CODE && p) *p = h->code; va_end(a); return CURLE_OK; }
static struct curl_slist* curl_slist_append(struct curl_slist* l, const char* s) {
    struct curl_slist* n = (struct curl_slist*)calloc(1, sizeof *n); size_t k = strlen(s); n->data = (char*)malloc(k + 1); memcpy(n->data, s, k + 1);
    if (!l) return n;
    struct curl_slist* t = l; while (t->next) t = t->next; t->next = n; return l;
}
static void curl_slist_free_all(struct curl_slist* l) { while (l) { struct curl_slist* n = l->next; free(l->data); free(l); l = n; } }
typedef struct { char* p; size_t len, cap; } rt_w_buf;
static void rt_w_add(rt_w_buf* b, const char* s, size_t n) {
    if (b->len + n + 1 > b->cap) { b->cap = (b->len + n + 1) * 2; b->p = (char*)realloc(b->p, b->cap); }
    memcpy(b->p + b->len, s, n); b->len += n; b->p[b->len] = 0;
}
static void rt_w_put(rt_w_buf* b, const char* s) { rt_w_add(b, s, strlen(s)); }
static CURLcode curl_easy_perform(CURL* h) {
    const char* u = h->url; if (strncmp(u, "http://", 7) == 0) u += 7;
    const char* slash = strchr(u, '/'); size_t hl = slash ? (size_t)(slash - u) : strlen(u);
    char host[256], hp[256]; if (hl >= sizeof host) return CURLE_FAILED; memcpy(host, u, hl); host[hl] = 0; memcpy(hp, host, hl + 1);
    const char* path = slash ? slash : "/";
    char* colon = strrchr(host, ':'); unsigned port = 80; if (colon) { *colon = 0; port = (unsigned)atoi(colon + 1); }
    unsigned a = 0, b = 0, c = 0, d = 0;
    if (!strcmp(host, "localhost")) { a = 127; d = 1; } else if (sscanf(host, "%u.%u.%u.%u", &a, &b, &c, &d) != 4) return CURLE_FAILED;
    struct { unsigned short fam, port; unsigned char ip[4]; char zero[8]; } sa = {2, (unsigned short)((port >> 8) | ((port & 0xff) << 8)), {(unsigned char)a, (unsigned char)b, (unsigned char)c, (unsigned char)d}, {0}};
    uintptr_t s = socket(2, 1, 6); if (s == (uintptr_t)-1) return CURLE_FAILED;
    if (h->timeout > 0) { unsigned long ms = (unsigned long)h->timeout * 1000; setsockopt(s, 0xffff, 0x1006, (const char*)&ms, sizeof ms); }
    if (connect(s, &sa, sizeof sa) != 0) { closesocket(s); return CURLE_FAILED; }
    size_t n = h->body ? (h->n >= 0 ? (size_t)h->n : strlen(h->body)) : 0;
    rt_w_buf rq = {0}; char t[512];
    rt_w_put(&rq, h->post ? "POST " : "GET "); rt_w_put(&rq, path);
    snprintf(t, sizeof t, " HTTP/1.1\r\nHost: %s\r\nConnection: close\r\n", hp); rt_w_put(&rq, t);
    if (h->post) { snprintf(t, sizeof t, "Content-Length: %zu\r\n", n); rt_w_put(&rq, t); }
    for (struct curl_slist* l = h->hd; l; l = l->next) { size_t k = strlen(l->data); if (k && l->data[k - 1] == ':') continue; rt_w_put(&rq, l->data); rt_w_put(&rq, "\r\n"); }
    if (h->sig && h->user) { snprintf(t, sizeof t, "Authorization: AWS4-HMAC-SHA256 Credential=%s/19700101/local/dynamodb/aws4_request, SignedHeaders=host, Signature=0\r\n", h->user); rt_w_put(&rq, t); }
    rt_w_put(&rq, "\r\n");
    if (n) rt_w_add(&rq, h->body, n);
    size_t off = 0; int ok = 1;
    while (off < rq.len) { int k = send(s, rq.p + off, (int)(rq.len - off), 0); if (k <= 0) { ok = 0; break; } off += (size_t)k; }
    free(rq.p);
    rt_w_buf rs = {0}; char buf[16384];
    while (ok) { int k = recv(s, buf, sizeof buf, 0); if (k < 0) ok = 0; if (k <= 0) break; rt_w_add(&rs, buf, (size_t)k); }
    closesocket(s);
    if (!ok || !rs.len) { free(rs.p); return CURLE_FAILED; }
    char* end = strstr(rs.p, "\r\n\r\n"); if (!end) { free(rs.p); return CURLE_FAILED; }
    h->code = 0; sscanf(rs.p, "HTTP/%*s %ld", &h->code);
    for (char* l = rs.p; l < end + 2;) { char* e = strstr(l, "\r\n"); if (!e) break; if (h->hf) h->hf(l, 1, (size_t)(e + 2 - l), h->hdd); l = e + 2; }
    size_t bl = rs.len - (size_t)(end + 4 - rs.p);
    if (h->wf && bl) h->wf(end + 4, 1, bl, h->wd);
    free(rs.p); return CURLE_OK;
}

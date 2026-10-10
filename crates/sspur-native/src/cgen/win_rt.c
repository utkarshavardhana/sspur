//@ threads
#include <time.h>
#include <errno.h>
#include <io.h>
#include <fcntl.h>
#include <wchar.h>
#define SS_W_API __declspec(dllimport)
SS_W_API void __stdcall AcquireSRWLockExclusive(void** l);
SS_W_API void __stdcall ReleaseSRWLockExclusive(void** l);
SS_W_API int __stdcall SleepConditionVariableSRW(void** c, void** l, unsigned long ms, unsigned long flags);
SS_W_API void __stdcall WakeConditionVariable(void** c);
SS_W_API void __stdcall WakeAllConditionVariable(void** c);
SS_W_API void* __stdcall CreateThread(void* sa, size_t stack, unsigned long (__stdcall* fn)(void*), void* arg, unsigned long flags, unsigned long* id);
SS_W_API unsigned long __stdcall WaitForSingleObject(void* h, unsigned long ms);
SS_W_API int __stdcall CloseHandle(void* h);
SS_W_API unsigned long __stdcall GetActiveProcessorCount(unsigned short group);
SS_W_API int __stdcall QueryPerformanceCounter(long long* c);
SS_W_API int __stdcall QueryPerformanceFrequency(long long* f);
SS_W_API void __stdcall GetSystemTimePreciseAsFileTime(unsigned long long* ft);
SS_W_API void __stdcall Sleep(unsigned long ms);
SS_W_API void* __stdcall VirtualAlloc(void* a, size_t n, unsigned long type, unsigned long prot);
SS_W_API int __stdcall VirtualFree(void* a, size_t n, unsigned long type);
SS_W_API unsigned long __stdcall GetLastError(void);
SS_W_API int __stdcall MultiByteToWideChar(unsigned cp, unsigned long fl, const char* s, int n, wchar_t* w, int wn);
SS_W_API int __stdcall WideCharToMultiByte(unsigned cp, unsigned long fl, const wchar_t* w, int wn, char* s, int n, const char* dflt, int* used);
typedef struct { void* p; } ss_w_mutex;
typedef struct { void* p; } ss_w_cond;
typedef struct { void* h; } ss_w_thread;
typedef struct { size_t stack; } ss_w_attr;
typedef struct { void* (*fn)(void*); void* arg; } ss_w_start;
#define pthread_mutex_t ss_w_mutex
#define pthread_cond_t ss_w_cond
#define pthread_t ss_w_thread
#define pthread_attr_t ss_w_attr
#define pthread_once_t volatile long
#define PTHREAD_MUTEX_INITIALIZER {0}
#define PTHREAD_COND_INITIALIZER {0}
#define PTHREAD_ONCE_INIT 0
static inline int ss_w_mutex_lock(ss_w_mutex* m) { AcquireSRWLockExclusive(&m->p); return 0; }
static inline int ss_w_mutex_unlock(ss_w_mutex* m) { ReleaseSRWLockExclusive(&m->p); return 0; }
static inline int ss_w_cond_init(ss_w_cond* c, const void* a) { (void)a; c->p = 0; return 0; }
static inline int ss_w_cond_destroy(ss_w_cond* c) { (void)c; return 0; }
static inline int ss_w_cond_wait(ss_w_cond* c, ss_w_mutex* m) { SleepConditionVariableSRW(&c->p, &m->p, 0xFFFFFFFFu, 0); return 0; }
static inline int ss_w_cond_signal(ss_w_cond* c) { WakeConditionVariable(&c->p); return 0; }
static inline int ss_w_cond_broadcast(ss_w_cond* c) { WakeAllConditionVariable(&c->p); return 0; }
static inline int ss_w_attr_init(ss_w_attr* a) { a->stack = 0; return 0; }
static inline int ss_w_attr_destroy(ss_w_attr* a) { (void)a; return 0; }
static inline int ss_w_attr_setstacksize(ss_w_attr* a, size_t n) { a->stack = n; return 0; }
static unsigned long __stdcall ss_w_tramp(void* p) { ss_w_start s = *(ss_w_start*)p; free(p); s.fn(s.arg); return 0; }
static inline int ss_w_create(ss_w_thread* t, const ss_w_attr* a, void* (*fn)(void*), void* arg) {
    ss_w_start* s = (ss_w_start*)malloc(sizeof *s); if (!s) return EAGAIN;
    s->fn = fn; s->arg = arg;
    t->h = CreateThread(0, a ? a->stack : 0, ss_w_tramp, s, 0x10000, 0);
    if (!t->h) { free(s); return EAGAIN; }
    return 0;
}
static inline int ss_w_join(ss_w_thread t, void** r) { (void)r; WaitForSingleObject(t.h, 0xFFFFFFFFu); CloseHandle(t.h); return 0; }
static inline int ss_w_once(volatile long* o, void (*fn)(void)) {
    if (__atomic_load_n(o, __ATOMIC_ACQUIRE) == 2) return 0;
    long z = 0;
    if (__atomic_compare_exchange_n(o, &z, 1, 0, __ATOMIC_ACQ_REL, __ATOMIC_ACQUIRE)) { fn(); __atomic_store_n(o, 2, __ATOMIC_RELEASE); return 0; }
    while (__atomic_load_n(o, __ATOMIC_ACQUIRE) != 2) Sleep(0);
    return 0;
}
#define pthread_mutex_lock(m) ss_w_mutex_lock(m)
#define pthread_mutex_unlock(m) ss_w_mutex_unlock(m)
#define pthread_cond_init(c, a) ss_w_cond_init(c, a)
#define pthread_cond_destroy(c) ss_w_cond_destroy(c)
#define pthread_cond_wait(c, m) ss_w_cond_wait(c, m)
#define pthread_cond_signal(c) ss_w_cond_signal(c)
#define pthread_cond_broadcast(c) ss_w_cond_broadcast(c)
#define pthread_attr_init(a) ss_w_attr_init(a)
#define pthread_attr_destroy(a) ss_w_attr_destroy(a)
#define pthread_attr_setstacksize(a, n) ss_w_attr_setstacksize(a, n)
#define pthread_create(t, a, f, x) ss_w_create(t, a, f, x)
#define pthread_join(t, r) ss_w_join(t, r)
#define pthread_once(o, f) ss_w_once(o, f)
#define _SC_NPROCESSORS_ONLN 0
#define sysconf(k) ((long)GetActiveProcessorCount(0xFFFF))
#ifndef CLOCK_REALTIME
#define CLOCK_REALTIME 0
#endif
#ifndef CLOCK_MONOTONIC
#define CLOCK_MONOTONIC 1
#endif
static int ss_w_clock_gettime(int clk, struct timespec* t) {
    if (clk == CLOCK_MONOTONIC) {
        long long c = 0, f = 1; QueryPerformanceCounter(&c); QueryPerformanceFrequency(&f);
        t->tv_sec = (time_t)(c / f); t->tv_nsec = (long)((c % f) * 1000000000LL / f); return 0;
    }
    unsigned long long ft = 0; GetSystemTimePreciseAsFileTime(&ft); ft -= 116444736000000000ULL;
    t->tv_sec = (time_t)(ft / 10000000ULL); t->tv_nsec = (long)(ft % 10000000ULL * 100); return 0;
}
static int ss_w_nanosleep(const struct timespec* r, struct timespec* rem) { (void)rem; Sleep((unsigned long)(r->tv_sec * 1000 + (r->tv_nsec + 999999) / 1000000)); return 0; }
#define clock_gettime(c, t) ss_w_clock_gettime(c, t)
#define nanosleep(r, m) ss_w_nanosleep(r, m)
typedef long long ss_w_ssize;
#define ssize_t ss_w_ssize
#define off_t long long
#define mode_t int
static long long ss_w_pread(int fd, void* b, size_t n, long long off) {
    long long cur = _lseeki64(fd, 0, SEEK_CUR);
    if (cur < 0 || _lseeki64(fd, off, SEEK_SET) < 0) return -1;
    int k = _read(fd, b, (unsigned)n); int e = errno;
    _lseeki64(fd, cur, SEEK_SET); errno = e; return k;
}
#define read(f, b, n) ((ss_w_ssize)_read(f, b, (unsigned)(n)))
#define write(f, b, n) ((ss_w_ssize)_write(f, b, (unsigned)(n)))
#define close(f) _close(f)
#define lseek(f, o, w) _lseeki64(f, o, w)
#define pread(f, b, n, o) ss_w_pread(f, b, n, o)
static __attribute__((constructor)) void ss_w_binary(void) { _setmode(0, _O_BINARY); _setmode(1, _O_BINARY); _setmode(2, _O_BINARY); }
//@ mman
#define PROT_READ 1
#define PROT_WRITE 2
#define MAP_PRIVATE 2
#define MAP_FIXED 0x10
#define MAP_ANON 0x20
#define MAP_NORESERVE 0x4000
#define MAP_FAILED ((void*)-1)
static void* ss_w_mmap(void* a, size_t n, int prot, int fl, int fd, long long off) {
    (void)prot; (void)fd; (void)off;
    if (fl & MAP_FIXED) { VirtualFree(a, n, 0x4000); return a; }
    void* p = VirtualAlloc(0, n, (fl & MAP_NORESERVE) ? 0x2000 : 0x3000, (fl & MAP_NORESERVE) ? 0x01 : 0x04);
    return p ? p : MAP_FAILED;
}
#define mmap(a, n, p, f, d, o) ss_w_mmap(a, n, p, f, d, o)
//@ commit
static size_t gc_take_pages(size_t n) {
    size_t i = gc_take_pages0(n);
    if (!VirtualAlloc(gc_lo + (i << GC_SHIFT), n << GC_SHIFT, 0x1000, 0x04)) sspur_trap((Status*)gc_root_ptr, 15, 0, 0, 0);
    return i;
}
//@ sys
#include <errno.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <direct.h>
#include <stdio.h>
SS_W_API void* __stdcall CreateFileW(const wchar_t* p, unsigned long acc, unsigned long share, void* sa, unsigned long disp, unsigned long fl, void* tmpl);
SS_W_API int __stdcall GetFileInformationByHandle(void* h, unsigned long* info);
SS_W_API void* __stdcall FindFirstFileW(const wchar_t* p, void* data);
SS_W_API int __stdcall FindNextFileW(void* h, void* data);
SS_W_API int __stdcall FindClose(void* h);
SS_W_API int __stdcall MoveFileExW(const wchar_t* a, const wchar_t* b, unsigned long fl);
SS_W_API unsigned long __stdcall GetFileAttributesW(const wchar_t* p);
SS_W_API unsigned char __stdcall CreateSymbolicLinkW(const wchar_t* link, const wchar_t* target, unsigned long fl);
SS_W_API int __stdcall DeviceIoControl(void* h, unsigned long code, void* in, unsigned long inn, void* out, unsigned long outn, unsigned long* got, void* ov);
#define SS_W_BAD ((void*)(intptr_t)-1)
static int ss_w_errno(unsigned long e) {
    switch (e) {
    case 2: case 3: case 15: case 123: case 161: return ENOENT;
    case 5: case 19: case 32: case 1314: return EACCES;
    case 80: case 183: return EEXIST;
    case 145: return ENOTEMPTY;
    case 267: return ENOTDIR;
    case 4390: return EINVAL;
    default: return EIO;
    }
}
static int ss_w_fail(void) { errno = ss_w_errno(GetLastError()); return -1; }
static wchar_t* ss_w_wide(const char* s) {
    int n = MultiByteToWideChar(65001, 8, s, -1, 0, 0); if (n <= 0) { errno = ENOENT; return 0; }
    wchar_t* w = (wchar_t*)malloc((size_t)n * sizeof(wchar_t)); MultiByteToWideChar(65001, 8, s, -1, w, n); return w;
}
static char* ss_w_narrow(const wchar_t* w, int wn, int* n) {
    int k = WideCharToMultiByte(65001, 0, w, wn, 0, 0, 0, 0); char* s = (char*)malloc((size_t)k + 1);
    WideCharToMultiByte(65001, 0, w, wn, s, k, 0, 0); s[k] = 0; if (n) *n = k; return s;
}
static int ss_w_open(const char* p, int fl, ...) {
    wchar_t* w = ss_w_wide(p); if (!w) return -1;
    int fd = _wopen(w, fl | _O_BINARY | _O_NOINHERIT, _S_IREAD | _S_IWRITE); free(w); return fd;
}
#define open(...) ss_w_open(__VA_ARGS__)
#ifndef O_CLOEXEC
#define O_CLOEXEC 0
#endif
struct ss_w_stat { int st_mode; long long st_size, st_dev, st_ino; struct timespec st_mtim; };
#undef S_ISDIR
#undef S_ISREG
#undef S_ISLNK
#define S_ISDIR(m) (((m) & 0170000) == 0040000)
#define S_ISREG(m) (((m) & 0170000) == 0100000)
#define S_ISLNK(m) (((m) & 0170000) == 0120000)
static int ss_w_hstat(void* h, struct ss_w_stat* sb, int link) {
    unsigned long i[13];
    if (!GetFileInformationByHandle(h, i)) return ss_w_fail();
    int ro = (i[0] & 1) != 0;
    sb->st_mode = (link && (i[0] & 0x400)) ? 0120777 : (i[0] & 0x10) ? (ro ? 0040555 : 0040755) : (ro ? 0100444 : 0100644);
    sb->st_size = (long long)(((unsigned long long)i[8] << 32) | i[9]);
    sb->st_dev = (long long)i[7];
    sb->st_ino = (long long)(((unsigned long long)i[11] << 32) | i[12]);
    unsigned long long t = (((unsigned long long)i[6] << 32) | i[5]) - 116444736000000000ULL;
    sb->st_mtim.tv_sec = (time_t)(t / 10000000ULL); sb->st_mtim.tv_nsec = (long)(t % 10000000ULL * 100);
    return 0;
}
static int ss_w_pstat(const char* p, struct ss_w_stat* sb, int link) {
    wchar_t* w = ss_w_wide(p); if (!w) return -1;
    void* h = CreateFileW(w, 0x80, 7, 0, 3, 0x02000000 | (link ? 0x00200000 : 0), 0); free(w);
    if (h == SS_W_BAD) return ss_w_fail();
    int r = ss_w_hstat(h, sb, link); CloseHandle(h); return r;
}
static int ss_w_fstat(int fd, struct ss_w_stat* sb) { void* h = (void*)_get_osfhandle(fd); if (h == SS_W_BAD) { errno = EBADF; return -1; } return ss_w_hstat(h, sb, 0); }
#define stat ss_w_stat
#define fstat(f, s) ss_w_fstat(f, s)
#define lstat(p, s) ss_w_pstat(p, s, 1)
static int ss_w_stat(const char* p, struct ss_w_stat* sb) { return ss_w_pstat(p, sb, 0); }
static int ss_w_mkdir(const char* p) { wchar_t* w = ss_w_wide(p); if (!w) return -1; int r = _wmkdir(w); free(w); return r; }
static int ss_w_rmdir(const char* p) { wchar_t* w = ss_w_wide(p); if (!w) return -1; int r = _wrmdir(w); free(w); return r; }
static int ss_w_unlink(const char* p) { wchar_t* w = ss_w_wide(p); if (!w) return -1; int r = _wunlink(w); free(w); return r; }
static int ss_w_chmod(const char* p, int m) { wchar_t* w = ss_w_wide(p); if (!w) return -1; int r = _wchmod(w, (m & 0200) ? (_S_IREAD | _S_IWRITE) : _S_IREAD); free(w); return r; }
static int ss_w_rename(const char* a, const char* b) {
    wchar_t* x = ss_w_wide(a); wchar_t* y = x ? ss_w_wide(b) : 0; if (!x || !y) { free(x); return -1; }
    int ok = MoveFileExW(x, y, 1); free(x); free(y); return ok ? 0 : ss_w_fail();
}
static int ss_w_symlink(const char* target, const char* link) {
    wchar_t* t = ss_w_wide(target); wchar_t* l = t ? ss_w_wide(link) : 0; if (!t || !l) { free(t); return -1; }
    for (wchar_t* q = t; *q; q++) if (*q == L'/') *q = L'\\';
    unsigned long a = GetFileAttributesW(t);
    int ok = CreateSymbolicLinkW(l, t, 2 | (a != 0xFFFFFFFFu && (a & 0x10) ? 1 : 0)) != 0; free(t); free(l); return ok ? 0 : ss_w_fail();
}
static ss_w_ssize ss_w_readlink(const char* p, char* buf, size_t cap) {
    wchar_t* w = ss_w_wide(p); if (!w) return -1;
    void* h = CreateFileW(w, 0x80, 7, 0, 3, 0x02000000 | 0x00200000, 0); free(w);
    if (h == SS_W_BAD) return ss_w_fail();
    unsigned char rb[16384]; unsigned long got = 0;
    int ok = DeviceIoControl(h, 0x000900A8, 0, 0, rb, sizeof rb, &got, 0); CloseHandle(h);
    if (!ok) { errno = GetLastError() == 4390 ? EINVAL : ss_w_errno(GetLastError()); return -1; }
    unsigned long tag; memcpy(&tag, rb, 4);
    if (tag != 0xA000000Cu) { errno = EINVAL; return -1; }
    unsigned short off, len; memcpy(&off, rb + 12, 2); memcpy(&len, rb + 14, 2);
    int n = 0; char* s = ss_w_narrow((const wchar_t*)(rb + 20 + off), len / 2, &n);
    size_t k = (size_t)n < cap ? (size_t)n : cap; memcpy(buf, s, k); free(s); return (ss_w_ssize)k;
}
#define mkdir(p, m) ss_w_mkdir(p)
#define rmdir(p) ss_w_rmdir(p)
#define unlink(p) ss_w_unlink(p)
#define chmod(p, m) ss_w_chmod(p, (int)(m))
#define fchmod(f, m) ((void)(f), (void)(m), 0)
#define rename(a, b) ss_w_rename(a, b)
#define symlink(t, l) ss_w_symlink(t, l)
#define readlink(p, b, n) ss_w_readlink(p, b, n)
typedef struct { unsigned long d[11]; wchar_t name[260]; wchar_t alt[14]; } ss_w_find;
struct ss_w_dirent { char d_name[1040]; };
typedef struct { void* h; int first, done; ss_w_find f; struct ss_w_dirent e; } ss_w_DIR;
static ss_w_DIR* ss_w_opendir(const char* p) {
    size_t n = strlen(p); char* q = (char*)malloc(n + 3); memcpy(q, p, n);
    if (n && q[n - 1] != '/' && q[n - 1] != '\\') q[n++] = '\\';
    q[n++] = '*'; q[n] = 0;
    wchar_t* w = ss_w_wide(q); free(q); if (!w) return 0;
    ss_w_DIR* d = (ss_w_DIR*)calloc(1, sizeof *d);
    d->h = FindFirstFileW(w, &d->f); free(w);
    if (d->h == SS_W_BAD) { unsigned long e = GetLastError(); if (e == 2) { d->done = 1; return d; } free(d); errno = ss_w_errno(e); return 0; }
    d->first = 1; return d;
}
static struct ss_w_dirent* ss_w_readdir(ss_w_DIR* d) {
    if (d->done) return 0;
    if (!d->first && !FindNextFileW(d->h, &d->f)) { d->done = 1; return 0; }
    d->first = 0;
    int n = WideCharToMultiByte(65001, 0, d->f.name, -1, d->e.d_name, (int)sizeof d->e.d_name, 0, 0);
    if (n <= 0) d->e.d_name[0] = 0;
    return &d->e;
}
static int ss_w_closedir(ss_w_DIR* d) { if (d->h && d->h != SS_W_BAD) FindClose(d->h); free(d); return 0; }
#define DIR ss_w_DIR
#define dirent ss_w_dirent
#define opendir(p) ss_w_opendir(p)
#define readdir(d) ss_w_readdir(d)
#define closedir(d) ss_w_closedir(d)
//@ proc fs
SS_W_API int __stdcall CreatePipe(void** r, void** w, void* sa, unsigned long size);
SS_W_API int __stdcall SetHandleInformation(void* h, unsigned long mask, unsigned long fl);
SS_W_API int __stdcall CreateProcessW(const wchar_t* app, wchar_t* cmd, void* psa, void* tsa, int inherit, unsigned long fl, void* env, const wchar_t* dir, void* si, void* pi);
SS_W_API int __stdcall GetExitCodeProcess(void* h, unsigned long* code);
SS_W_API int __stdcall ReadFile(void* h, void* b, unsigned long n, unsigned long* got, void* ov);
SS_W_API int __stdcall WriteFile(void* h, const void* b, unsigned long n, unsigned long* put, void* ov);
typedef struct { unsigned long n; void* sd; int inherit; } ss_w_sa;
typedef struct { unsigned long cb; wchar_t* r; wchar_t* desk; wchar_t* title; unsigned long x, y, xs, ys, xc, yc, fill, flags; unsigned short show, cbr2; unsigned char* r2; void* in; void* out; void* err; } ss_w_si;
typedef struct { void* hp; void* ht; unsigned long pid, tid; } ss_w_pi;
typedef struct { void* h; const char* p; int64_t n; char* buf; int64_t len, cap; } ss_w_io;
static Str ss_proc_why(Str prog, const char* w) { SB_INIT(b); sb_put(&b, prog.p, prog.len); sb_put(&b, ": ", 2); sb_put(&b, w, (int64_t)strlen(w)); return sb_done(&b); }
static unsigned long __stdcall ss_w_drain(void* a) {
    ss_w_io* io = (ss_w_io*)a; char t[65536]; unsigned long got;
    while (ReadFile(io->h, t, sizeof t, &got, 0) && got) {
        if (io->len + (int64_t)got > io->cap) { io->cap = (io->cap + (int64_t)got) * 2; io->buf = (char*)realloc(io->buf, (size_t)io->cap); }
        memcpy(io->buf + io->len, t, got); io->len += got;
    }
    return 0;
}
static unsigned long __stdcall ss_w_feed(void* a) {
    ss_w_io* io = (ss_w_io*)a; int64_t off = 0; unsigned long put;
    while (off < io->n) { unsigned long k = io->n - off > (1 << 20) ? (1 << 20) : (unsigned long)(io->n - off); if (!WriteFile(io->h, io->p + off, k, &put, 0)) break; off += put; }
    CloseHandle(io->h); return 0;
}
static void ss_w_quote(SB* b, Str a) {
    int plain = a.len > 0;
    for (int64_t i = 0; i < a.len; i++) if (a.p[i] == ' ' || a.p[i] == '\t' || a.p[i] == '"' || a.p[i] == '\n' || a.p[i] == '\v') plain = 0;
    if (plain) { sb_put(b, a.p, a.len); return; }
    sb_put(b, "\"", 1);
    for (int64_t i = 0; ; i++) {
        int64_t bs = 0; while (i < a.len && a.p[i] == '\\') { bs++; i++; }
        if (i == a.len) { for (int64_t k = 0; k < bs * 2; k++) sb_put(b, "\\", 1); break; }
        if (a.p[i] == '"') { for (int64_t k = 0; k < bs * 2 + 1; k++) sb_put(b, "\\", 1); sb_put(b, "\"", 1); }
        else { for (int64_t k = 0; k < bs; k++) sb_put(b, "\\", 1); sb_put(b, a.p + i, 1); }
    }
    sb_put(b, "\"", 1);
}
static int ss_run_cmd(Str prog, const Str* args, int64_t na, Str input, int64_t* code, Str* out, Str* errs, Str* why) {
    if (!ss_cpath(prog)) { *why = ss_why(prog, -1); return 0; }
    SB_INIT(cl); ss_w_quote(&cl, prog);
    for (int64_t i = 0; i < na; i++) { if (!ss_cpath(args[i])) { *why = ss_proc_why(prog, "invalid argument"); return 0; } sb_put(&cl, " ", 1); ss_w_quote(&cl, args[i]); }
    sb_put(&cl, "", 0); Str cs = sb_done(&cl); char* c = ss_cpath(cs);
    wchar_t* wcmd = ss_w_wide(c); if (!wcmd) { *why = ss_proc_why(prog, "invalid argument"); return 0; }
    ss_w_sa sa = {sizeof(ss_w_sa), 0, 1};
    void *ir, *iw, *or_, *ow, *er, *ew;
    if (!CreatePipe(&ir, &iw, &sa, 0)) { free(wcmd); *why = ss_why(prog, ss_w_errno(GetLastError())); return 0; }
    if (!CreatePipe(&or_, &ow, &sa, 0)) { CloseHandle(ir); CloseHandle(iw); free(wcmd); *why = ss_why(prog, ss_w_errno(GetLastError())); return 0; }
    if (!CreatePipe(&er, &ew, &sa, 0)) { CloseHandle(ir); CloseHandle(iw); CloseHandle(or_); CloseHandle(ow); free(wcmd); *why = ss_why(prog, ss_w_errno(GetLastError())); return 0; }
    SetHandleInformation(iw, 1, 0); SetHandleInformation(or_, 1, 0); SetHandleInformation(er, 1, 0);
    ss_w_si si; memset(&si, 0, sizeof si); si.cb = sizeof si; si.flags = 0x100; si.in = ir; si.out = ow; si.err = ew;
    ss_w_pi pi; memset(&pi, 0, sizeof pi);
    int ok = CreateProcessW(0, wcmd, 0, 0, 1, 0x08000000, 0, 0, &si, &pi);
    unsigned long ce = ok ? 0 : GetLastError();
    free(wcmd); CloseHandle(ir); CloseHandle(ow); CloseHandle(ew);
    if (!ok) { CloseHandle(iw); CloseHandle(or_); CloseHandle(er); *why = ss_why(prog, ss_w_errno(ce)); return 0; }
    CloseHandle(pi.ht);
    ss_w_io fi = {iw, input.p, input.len, 0, 0, 0}, oi = {or_, 0, 0, 0, 0, 0}, ei = {er, 0, 0, 0, 0, 0};
    void* tf = CreateThread(0, 0, ss_w_feed, &fi, 0, 0);
    if (!tf) CloseHandle(iw);
    void* te = CreateThread(0, 0, ss_w_drain, &ei, 0, 0);
    ss_w_drain(&oi);
    if (te) WaitForSingleObject(te, 0xFFFFFFFFu); else ss_w_drain(&ei);
    if (tf) { WaitForSingleObject(tf, 0xFFFFFFFFu); CloseHandle(tf); }
    if (te) CloseHandle(te);
    CloseHandle(or_); CloseHandle(er);
    WaitForSingleObject(pi.hp, 0xFFFFFFFFu);
    unsigned long ec = 0; GetExitCodeProcess(pi.hp, &ec); CloseHandle(pi.hp);
    *code = (int64_t)(int32_t)ec;
    SB_INIT(ob); SB_INIT(eb); sb_put(&ob, oi.buf, oi.len); sb_put(&eb, ei.buf, ei.len); free(oi.buf); free(ei.buf);
    *out = sb_done(&ob); *errs = sb_done(&eb);
    if (!ss_utf8_ok((const unsigned char*)out->p, out->len) || !ss_utf8_ok((const unsigned char*)errs->p, errs->len)) { *why = ss_why(prog, -2); return 0; }
    return 1;
}

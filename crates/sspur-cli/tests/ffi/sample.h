#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

struct point { int x, y; };
typedef unsigned int flags_t;

int add(int a, int b);
double scale(double x, float k);
size_t count_char(const char *s, char c);
const char *greeting(void);
char *dup_upper(const char *s);
int64_t sum64(const int64_t *xs, size_t n);
bool is_even(long n);
void reset(void);
unsigned char clamp_byte(int v);
flags_t set_flag(flags_t f, unsigned short bit);
int min(int a, int b);
int printf_like(const char *fmt, ...);
struct point make_point(int x, int y);
void fill(int *out, size_t n);
static inline int helper(int x) { return x; }

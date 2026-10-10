#include <stdint.h>

typedef struct point point;

double point_norm(double x, double y);
int32_t point_quadrant(double x, double y);
const char *point_label(int32_t quadrant);
void point_free(point *p);

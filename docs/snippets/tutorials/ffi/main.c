// sspur export-c geo.ssp -o libgeo && cc main.c -L. -lgeo -lm -o geo && ./geo
#include <stdio.h>
#include "libgeo.h"

int main(void) {
    double d = 0, side = 0;
    if (geo_dist(0, 0, 3, 4, &d) == GEO_OK) printf("dist = %g\n", d);
    if (geo_cube_side(-8, &side) == GEO_RAISED) printf("cube_side: %s\n", geo_last_error());
    return 0;
}

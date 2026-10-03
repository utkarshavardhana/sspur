// sspur export-c geo.ssp -o libgeo && cc main.c -L. -lgeo -lm -o geo && ./geo
#include <stdio.h>
#include "libgeo.h"

static void check(const char* what, int32_t rc) {
    if (rc != GEO_OK) printf("%s failed (%d): %s\n", what, rc, geo_last_error());
}

int main(void) {
    double d = 0, side = 0;
    int64_t n = 0;
    char* s = 0;
    bool home = false;
    check("dist", geo_dist(0, 0, 3, 4, &d));
    printf("dist = %g\n", d);
    check("cube_side", geo_cube_side(27, &side));
    printf("cube_side = %g\n", side);
    check("cube_side", geo_cube_side(-8, &side));
    check("parse_sum", geo_parse_sum("40", "2", &n));
    printf("parse_sum = %lld\n", (long long)n);
    if (geo_label("geo", -7, &s) == GEO_OK) {
        printf("label = %s\n", s);
        geo_free(s);
    }
    check("steps", geo_steps(-1, &n));
    check("fact", geo_fact(25, &n));
    check("fact", geo_fact(20, &n));
    printf("fact(20) = %lld\n", (long long)n);
    check("has_home", geo_has_home(&home));
    printf("has_home = %s\n", home ? "true" : "false");
    return 0;
}

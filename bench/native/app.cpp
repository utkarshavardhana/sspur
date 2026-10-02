#include <algorithm>
#include <cstdio>
#include <cstdlib>
#include <map>
#include <string>
#include <vector>
struct Sale { std::string region, product; long qty, cents; };
struct Stat { long orders = 0, units = 0, revenue = 0; };
static std::vector<std::string> split(const std::string& s, char sep) {
    std::vector<std::string> out; std::string cur;
    for (char c : s) { if (c == sep) { out.push_back(cur); cur.clear(); } else cur += c; }
    out.push_back(cur); return out;
}
static long to_int(const std::string& s) { char* e; long v = strtol(s.c_str(), &e, 10); if (*e || s.empty()) abort(); return v; }
int main() {
    const char* regions[] = {"north", "south", "east", "west", "central"};
    const char* products[] = {"apple", "pear", "fig", "kiwi", "plum", "lime", "date"};
    std::string text;
    for (long i = 0; i < 400000; i++) {
        if (i) text += '\n';
        text += regions[i % 5]; text += ','; text += products[(i * 7) % 7]; text += '-'; text += std::to_string(i % 13);
        text += ','; text += std::to_string(i % 9 + 1); text += ','; text += std::to_string((i * 37) % 1000 + 99);
    }
    std::map<std::string, Stat> stats;
    for (auto& line : split(text, '\n')) {
        auto p = split(line, ','); if (p.size() != 4) abort();
        Sale s{p[0], p[1], to_int(p[2]), to_int(p[3])};
        auto& st = stats[s.region + "/" + s.product];
        st.orders++; st.units += s.qty; st.revenue += s.qty * s.cents;
    }
    std::vector<std::pair<std::string, Stat>> v(stats.begin(), stats.end());
    std::stable_sort(v.begin(), v.end(), [](auto& a, auto& b) { return a.second.revenue != b.second.revenue ? a.second.revenue > b.second.revenue : a.first < b.first; });
    for (int i = 0; i < 3; i++) printf("%s: %ld orders, %ld units, $%ld\n", v[i].first.c_str(), v[i].second.orders, v[i].second.units, v[i].second.revenue / 100);
}

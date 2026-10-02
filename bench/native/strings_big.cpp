#include <algorithm>
#include <cctype>
#include <cstdio>
#include <string>
#include <unordered_map>
#include <vector>
int main() {
    const char* words[] = {"alpha", "beta", "gamma", "delta", "Alpha", "omega", "beta", "pi"};
    std::string text;
    for (long i = 0; i < 2000000; i++) { if (i) text += ' '; text += words[(i * 7 + i / 3) % 8]; }
    for (auto& c : text) c = std::tolower((unsigned char)c);
    std::unordered_map<std::string, long> counts; std::vector<std::string> order; std::string cur;
    for (char c : text + " ") { if (std::isalnum((unsigned char)c)) cur += c; else if (!cur.empty()) { if (counts[cur]++ == 0) order.push_back(cur); cur.clear(); } }
    std::vector<std::pair<std::string, long>> v; for (auto& w : order) v.push_back({w, counts[w]});
    std::stable_sort(v.begin(), v.end(), [](auto& a, auto& b) { return a.second != b.second ? a.second > b.second : a.first < b.first; });
    for (int i = 0; i < 3; i++) printf("%s %ld\n", v[i].first.c_str(), v[i].second);
}

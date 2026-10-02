#include <algorithm>
#include <cctype>
#include <string>
#include <unordered_map>
#include <utility>
#include <vector>

std::vector<std::pair<std::string, int>> top_words(const std::string& text, size_t k) {
    std::unordered_map<std::string, int> counts;
    std::string cur;
    for (char ch : text + " ") {
        char c = std::tolower(static_cast<unsigned char>(ch));
        if (c >= 'a' && c <= 'z') {
            cur += c;
        } else if (!cur.empty()) {
            ++counts[cur];
            cur.clear();
        }
    }
    std::vector<std::pair<std::string, int>> out(counts.begin(), counts.end());
    std::sort(out.begin(), out.end(), [](const auto& a, const auto& b) {
        return a.second != b.second ? a.second > b.second : a.first < b.first;
    });
    if (out.size() > k) out.resize(k);
    return out;
}

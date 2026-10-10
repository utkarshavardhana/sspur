#include <chrono>
#include <cpr/cpr.h>
#include <expected>
#include <nlohmann/json.hpp>
#include <string>
#include <thread>

struct User {
    std::string id;
    std::string name;
};

struct FetchError {
    std::string msg;
};

std::expected<User, FetchError> fetch_user(const std::string& url, int attempts = 5) {
    for (int n = 0; n < attempts; ++n) {
        cpr::Response r = cpr::Get(cpr::Url{url}, cpr::Timeout{2000});
        if (r.error.code == cpr::ErrorCode::OK && r.status_code < 500) {
            if (r.status_code >= 400) return std::unexpected(FetchError{"http " + std::to_string(r.status_code)});
            try {
                auto d = nlohmann::json::parse(r.text);
                return User{d.at("id").get<std::string>(), d.at("name").get<std::string>()};
            } catch (const nlohmann::json::exception& e) {
                return std::unexpected(FetchError{e.what()});
            }
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(100 << n));
    }
    return std::unexpected(FetchError{"gave up after " + std::to_string(attempts) + " attempts"});
}

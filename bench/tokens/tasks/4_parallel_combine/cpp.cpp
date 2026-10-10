#include <future>
#include <numeric>
#include <string>

struct Summary {
    std::string name;
    size_t item_count;
    long long total_cents;
};

Summary summary(Api& api, const std::string& user_id) {
    auto user_f = std::async(std::launch::async, [&] { return api.get_user(user_id); });
    auto cart_f = std::async(std::launch::async, [&] { return api.get_cart(user_id); });
    User user = user_f.get();
    Cart cart = cart_f.get();
    long long total = std::accumulate(cart.items.begin(), cart.items.end(), 0LL,
        [](long long s, const Item& i) { return s + i.price_cents * i.qty; });
    return Summary{user.name, cart.items.size(), total};
}

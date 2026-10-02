#include <expected>
#include <numeric>
#include <string>
#include <variant>
#include <vector>

struct Item {
    std::string sku;
    int qty;
    long long price_cents;
};

struct Order {
    std::string id;
    std::vector<Item> items;
};

struct EmptyOrder {};
struct BadQty { std::string sku; };
struct OutOfStock { std::string sku; };
struct StoreFailure { std::string msg; };
using OrderError = std::variant<EmptyOrder, BadQty, OutOfStock, StoreFailure>;

class Store {
public:
    virtual ~Store() = default;
    virtual std::expected<int, StoreFailure> stock(const std::string& sku) = 0;
    virtual std::expected<void, StoreFailure> put_order(const Order& order) = 0;
};

long long total(const std::vector<Item>& items) {
    return std::accumulate(items.begin(), items.end(), 0LL,
        [](long long s, const Item& i) { return s + i.price_cents * i.qty; });
}

std::expected<Order, OrderError> place(Store& store, Order order) {
    if (order.items.empty()) return std::unexpected(EmptyOrder{});
    for (const auto& i : order.items) {
        if (i.qty <= 0) return std::unexpected(BadQty{i.sku});
        auto n = store.stock(i.sku);
        if (!n) return std::unexpected(n.error());
        if (*n < i.qty) return std::unexpected(OutOfStock{i.sku});
    }
    if (auto r = store.put_order(order); !r) return std::unexpected(r.error());
    return order;
}

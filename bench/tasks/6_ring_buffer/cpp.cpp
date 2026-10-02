#include <cstddef>
#include <new>
#include <optional>
#include <utility>

template <typename T, std::size_t N>
class RingBuffer {
public:
    RingBuffer() = default;
    RingBuffer(const RingBuffer&) = delete;
    RingBuffer& operator=(const RingBuffer&) = delete;
    ~RingBuffer() { while (pop()) {} }

    std::size_t size() const { return len_; }

    bool push(T x) {
        if (len_ == N) return false;
        new (slot((head_ + len_) % N)) T(std::move(x));
        ++len_;
        return true;
    }

    std::optional<T> pop() {
        if (len_ == 0) return std::nullopt;
        T* p = slot(head_);
        std::optional<T> x(std::move(*p));
        p->~T();
        head_ = (head_ + 1) % N;
        --len_;
        return x;
    }

private:
    T* slot(std::size_t i) { return std::launder(reinterpret_cast<T*>(&storage_[i * sizeof(T)])); }

    alignas(T) std::byte storage_[N * sizeof(T)];
    std::size_t head_ = 0;
    std::size_t len_ = 0;
};

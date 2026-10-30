#include <mutex>
#include <vector>

#define ASSERT(x) check(x)

namespace shop {

int check(bool ok);

/// A shopping cart.
class Cart : public Base {
public:
    Cart(int n) : count_(n), Base(n) {
        reset();
    }

    ~Cart() {
        delete items_;
    }

    void reset() {
        this->count_ = 0;
        items_ = nullptr;
    }

    Cart& operator+=(const Cart& other);

    template <typename T>
    T first(const std::vector<T>& xs) {
        return xs[0];
    }

private:
    int count_;
    int* items_;
};

/** Adds another cart. */
Cart& Cart::operator+=(const Cart& other) {
    count_ += other.count_;
    return *this;
}

}  // namespace shop

void shop::Cart::run(int n, std::map<int, int>& m) {
    int x = 1, y(2), z{3};
    Foo f(x, y);
    std::vector<int> v;
    std::vector<int> w{1, 2, 3};
    int a[3] = {1, 2, 3};
    auto [p, q] = std::make_pair(1, 2);
    int* ptr = &x;
    std::lock_guard<std::mutex> guard(mutex_);
    if (auto it = m.find(n); it != m.end()) {
        x++;
    } else if (Foo* found = lookup(n)) {
        found->use();
    }
    for (const auto& e : v) {
        --x;
    }
    for (auto& [key, value] : m) {
        x += key * value;
    }
    switch (x) {
    case 1:
    case 2:
        x = 0;
        [[fallthrough]];
    case 3: {
        x = 3;
        break;
    }
    default:
        break;
    }
    try {
        risky();
    } catch (const std::exception& e) {
        throw;
    } catch (...) {
        printf("unknown\n");
    }
    auto twice = [](int k) { return k * 2; };
    ASSERT(x > 0);
    printf("%d\n", twice(x));
#if defined(DEBUG)
    trace(x);
#else
    x = x << 1;
#endif
    goto end;
end:
    return;
}

static int legacy(int argc, char** argv) {
    int i;
    for (i = 0; i < argc; ++i) {
        if (argv[i] == 0)
            continue;
    }
    while (true) {
        break;
    }
    struct Local { int v; };
    int broken = = 2;
    return 0;
}

Task<int> fetch(int id) {
    co_yield id;
    co_return id * 2;
}

void fractions(std::vector<int>& v) {
    for (double x = 0; x < 2.5; x++) {
        v.push_back(1);
    }
    try {
        v.reserve(1000);
    } catch (const std::bad_alloc&) {
        v.clear();
    }
}

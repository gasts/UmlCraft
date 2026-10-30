class Jumps {
public:
    void plainReturn(int a) {
        if (a > 0) {
            return;
        }
        work();
    }

    int loops(std::vector<int> values) {
        int sum = 0;
        for (int v : values) {
            if (v < 0) {
                continue;
            }
            if (v > 100) {
                break;
            }
            sum += v;
        }
        return sum;
    }

    void fail(std::string message) {
        throw new IllegalStateException(message);
    }

private:
    void work() {
    }
};

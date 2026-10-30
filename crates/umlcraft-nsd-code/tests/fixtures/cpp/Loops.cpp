class Loops {
public:
    int counting(int n) {
        int sum = 0;
        for (int i = 0; i < n; i++) {
            sum += i;
        }
        for (int i = 0; i <= n; i += 2) {
            sum += i;
        }
        for (int i = n; i > 0; i--) {
            sum -= i;
        }
        return sum;
    }

    int free(int n) {
        int sum = 0;
        for (int i = 0, j = n; i < j; i++, j--) {
            sum += j - i;
        }
        for (int i = 0; ; i++) {
            if (i > n) {
                break;
            }
        }
        return sum;
    }

    int enhanced(std::vector<int> values) {
        int sum = 0;
        for (int v : values) {
            sum += v;
        }
        return sum;
    }

    int whileLoop(int n) {
        while (n > 10) {
            n = n / 2;
        }
        return n;
    }

    int doWhile(int n) {
        do {
            n--;
        } while (n > 0);
        return n;
    }

    void endless() {
        while (true) {
            work();
        }
    }

    void endlessFor() {
        for (;;) {
            work();
        }
    }

    void work() {
    }
};

class Branches {
public:
    int simpleIf(int a) {
        if (a > 0) {
            a = 1;
        }
        return a;
    }

    int ifElse(int a) {
        if (a > 0) {
            a = 1;
        } else {
            a = -1;
        }
        return a;
    }

    std::string elseIfChain(int a) {
        if (a < 0) {
            return "negative";
        } else if (a == 0) {
            return "zero";
        } else if (a < 10) {
            return "small";
        } else {
            return "large";
        }
    }

    int nested(int a, int b) {
        if (a > 0) {
            if (b > 0) {
                return 1;
            }
            return 2;
        }
        return 3;
    }

    bool logic(int a, int b) {
        if (a > 0 && b > 0 || a < -10) {
            return true;
        }
        return false;
    }

    int ternary(int a) {
        int b = a > 0 ? a : -a;
        return b;
    }
};

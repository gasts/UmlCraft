class Statements {
public:
    int total;

    /** Declarations, assignments and expressions. */
    int declarations(int a) {
        int x;
        int y = a + 1;
        x = y * 2;
        y++;
        total += x;
        return x;
    }

    void calls(std::list<std::string> list) {
        list.clear();
        helper(1, 2);
        int n = list.size();
        std::string s = list.toString().trim();
        System.out.println(n + s);
        new StringBuilder();
    }

    void blocks() {
        {
            int inner = 1;
            helper(inner, 0);
        }
        ;
    }

    void helper(int a, int b) {
    }
};

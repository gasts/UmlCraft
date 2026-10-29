public class Statements {
    private int total;

    /** Declarations, assignments and expressions. */
    public int declarations(int a) {
        int x;
        int y = a + 1;
        x = y * 2;
        y++;
        total += x;
        return x;
    }

    public void calls(java.util.List<String> list) {
        list.clear();
        helper(1, 2);
        int n = list.size();
        String s = list.toString().trim();
        System.out.println(n + s);
        new StringBuilder();
    }

    public void blocks() {
        {
            int inner = 1;
            helper(inner, 0);
        }
        ;
    }

    private void helper(int a, int b) {
    }
}

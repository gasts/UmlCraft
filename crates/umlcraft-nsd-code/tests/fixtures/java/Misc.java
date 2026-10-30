import java.util.List;

public class Misc {
    private final Object lock = new Object();
    private int count;

    /**
     * Creates a counter.
     *
     * @param start the first value
     */
    public Misc(int start) {
        count = start;
    }

    public static int twice(int a) {
        return 2 * a;
    }

    public void localClass() {
        class Local {
            int value;
        }
        Local l = new Local();
        l.value = 1;
    }

    public void sync() {
        synchronized (lock) {
            count++;
        }
    }

    public void check(int a) {
        assert a > 0 : "positive";
        // Line comment before a statement
        count = a;
        /* Block comment before a statement */
        count++;
        count--; // trailing comment
    }
}

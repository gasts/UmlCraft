public class Jumps {
    public void plainReturn(int a) {
        if (a > 0) {
            return;
        }
        work();
    }

    public int loops(int[] values) {
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

    public int labels(int[][] grid) {
        int found = -1;
        outer:
        for (int i = 0; i < grid.length; i++) {
            for (int j = 0; j < grid[i].length; j++) {
                if (grid[i][j] == 0) {
                    found = i;
                    break outer;
                }
                if (grid[i][j] < 0) {
                    continue outer;
                }
            }
        }
        return found;
    }

    public void fail(String message) {
        throw new IllegalStateException(message);
    }

    private void work() {
    }
}

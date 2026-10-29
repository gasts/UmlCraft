public class Modern {
    public String arrow(int day) {
        String name;
        switch (day) {
            case 1 -> name = "Monday";
            case 2, 3 -> name = "Midweek";
            default -> name = "Other";
        }
        return name;
    }

    public int expression(int day) {
        int len = switch (day) {
            case 1 -> 6;
            case 2 -> {
                work();
                yield 7;
            }
            default -> 0;
        };
        return len;
    }

    private void work() {
    }
}

public class Switches {
    public String classic(int day) {
        String name;
        switch (day) {
            case 1:
                name = "Monday";
                break;
            case 2:
            case 3:
                name = "Midweek";
                break;
            default:
                name = "Other";
        }
        return name;
    }

    public int fallthrough(int a) {
        int r = 0;
        switch (a) {
            case 1:
                r += 1;
            case 2:
                r += 2;
                break;
            default:
                r = -1;
                break;
        }
        return r;
    }

    public int defaultInMiddle(int a) {
        int r;
        switch (a) {
            case 1:
                r = 10;
                break;
            default:
                r = 0;
                break;
            case 2:
                r = 20;
                break;
        }
        return r;
    }

    public void noDefault(int a) {
        switch (a) {
            case 1:
                work();
                break;
            case 2:
                work();
                break;
        }
    }

    private void work() {
    }
}

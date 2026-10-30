public class Edge {
    private int value;

    int kind(int day) {
        return switch (day) {
            case 6, 7 -> 0;
            case 1 -> {
                if (value > 0) {
                    yield 2;
                }
                yield 1;
            }
            default -> throw new IllegalArgumentException("day");
        };
    }

    void fractions() {
        for (double x = 0; x < 2.5; x++) {
            step(x);
        }
    }

    void calls() {
        this.reset();
        super.toString();
        reset();
        value = compute(1) + compute(2);
    }

    void comments() {
        // only a comment at the start
        while (value < 10) {
            value++;
            // a comment at the end of a block
        }
        if (value > 0) {
            // a comment in an otherwise empty branch
        }
    }

    void broken() {
        int a = 1;
        a = = 2;
        a++;
    }

    void caseDefaultWithLabel(int a) {
        switch (a) {
            case 1:
                value = 1;
                break;
            case 2:
            default:
                value = 0;
        }
    }

    String chain(StringBuilder sb) {
        sb.append("a")
          .append("b")
          .append("c");
        return sb.toString();
    }

    void reset() {
        value = 0;
    }

    int compute(int x) {
        return x;
    }

    /**/
    void braced(int a) {
        switch (a) {
            case 1: {
                value = 1;
                break;
            }
            case 2:
                value = 2;
                break;
        }
    }

    void onlyDefault(int a) {
        switch (a) {
            default:
                value = a;
        }
        switch (a) {
        }
        value = switch (a) {
            default -> 3;
        };
    }

    void unions(java.io.Reader r) {
        try {
            r.read();
        } catch (java.io.IOException|IllegalStateException e) {
            reset();
        } catch (RuntimeException e) {
            reset();
        }
    }

    static class Nested {
        enum Mode {
            ON, OFF;

            boolean on() {
                return this == ON;
            }
        }

        Nested() {
            this(1);
        }

        Nested(int x) {
        }
    }
}

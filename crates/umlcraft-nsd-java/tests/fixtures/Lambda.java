import java.util.List;

public class Lambda {
    private int count;

    public void lambda(List<String> list) {
        list.forEach(s -> System.out.println(s));
        Runnable r = new Runnable() {
            @Override
            public void run() {
                count++;
            }
        };
        r.run();
    }
}

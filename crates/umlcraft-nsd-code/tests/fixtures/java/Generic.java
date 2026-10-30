import java.util.List;

public class Generic {
    public <T> T first(List<T> list) {
        return list.get(0);
    }
}

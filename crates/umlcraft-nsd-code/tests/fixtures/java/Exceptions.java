import java.io.IOException;
import java.io.Reader;

public class Exceptions {
    public int tryCatch(String s) {
        try {
            return Integer.parseInt(s);
        } catch (NumberFormatException e) {
            return 0;
        }
    }

    public void multiCatch(Reader r) {
        try {
            r.read();
        } catch (IOException e) {
            log(e);
        } catch (RuntimeException e) {
            log(e);
        }
    }

    public void unionCatch(Reader r) {
        try {
            r.read();
        } catch (IOException | RuntimeException e) {
            log(e);
        }
    }

    public void tryFinally(Reader r) throws IOException {
        try {
            r.read();
        } catch (IOException e) {
            log(e);
        } finally {
            r.close();
        }
    }

    public void onlyFinally(Reader r) throws IOException {
        try {
            r.read();
        } finally {
            r.close();
        }
    }

    public void withResources(java.io.File f) throws IOException {
        try (Reader r = new java.io.FileReader(f)) {
            r.read();
        }
    }

    private void log(Exception e) {
    }
}

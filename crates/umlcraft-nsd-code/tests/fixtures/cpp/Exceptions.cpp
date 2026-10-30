#include <stdexcept>

class Exceptions {
public:
    int tryCatch(std::string s) {
        try {
            return Integer.parseInt(s);
        } catch (NumberFormatException e) {
            return 0;
        }
    }

    void multiCatch(Reader r) {
        try {
            r.read();
        } catch (IOException e) {
            log(e);
        } catch (RuntimeException e) {
            log(e);
        }
    }

private:
    void log(Exception e) {
    }
};

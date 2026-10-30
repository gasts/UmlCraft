class Exceptions:
    def tryCatch(self, s):
        try:
            return Integer.parseInt(s)
        except NumberFormatException as e:
            return 0

    def multiCatch(self, r):
        try:
            r.read()
        except IOException as e:
            log(e)
        except RuntimeException as e:
            log(e)

    def unionCatch(self, r):
        try:
            r.read()
        except (IOException, RuntimeException) as e:
            log(e)

    def tryFinally(self, r):
        try:
            r.read()
        except IOException as e:
            log(e)
        finally:
            r.close()

    def onlyFinally(self, r):
        try:
            r.read()
        finally:
            r.close()

    def log(self, e):
        pass

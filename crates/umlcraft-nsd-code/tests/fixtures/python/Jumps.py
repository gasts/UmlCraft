class Jumps:
    def plainReturn(self, a):
        if a > 0:
            return
        work()

    def loops(self, values):
        sum = 0
        for v in values:
            if v < 0:
                continue
            if v > 100:
                break
            sum += v
        return sum

    def work(self):
        pass

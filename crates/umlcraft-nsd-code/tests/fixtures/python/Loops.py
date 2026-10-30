class Loops:
    def counting(self, n):
        sum = 0
        for i in range(n):
            sum += i
        for i in range(0, n + 1, 2):
            sum += i
        for i in range(n, 0, -1):
            sum -= i
        return sum

    def enhanced(self, values):
        sum = 0
        for v in values:
            sum += v
        return sum

    def whileLoop(self, n):
        while n > 10:
            n = n / 2
        return n

    def endless(self):
        while True:
            work()

    def work(self):
        pass

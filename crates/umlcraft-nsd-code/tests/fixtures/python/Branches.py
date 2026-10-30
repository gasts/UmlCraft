class Branches:
    def simpleIf(self, a):
        if a > 0:
            a = 1
        return a

    def ifElse(self, a):
        if a > 0:
            a = 1
        else:
            a = -1
        return a

    def elseIfChain(self, a):
        if a < 0:
            return "negative"
        elif a == 0:
            return "zero"
        elif a < 10:
            return "small"
        else:
            return "large"

    def nested(self, a, b):
        if a > 0:
            if b > 0:
                return 1
            return 2
        return 3

    def logic(self, a, b):
        if a > 0 and b > 0 or a < -10:
            return True
        return False

    def ternary(self, a):
        b = a if a > 0 else -a
        return b

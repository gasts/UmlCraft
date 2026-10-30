class Switches:
    def classic(self, day):
        match day:
            case 1:
                name = "Monday"
            case 2 | 3:
                name = "Midweek"
            case _:
                name = "Other"
        return name

    def noDefault(self, a):
        match a:
            case 1:
                work()
            case 2:
                work()

    def work(self):
        pass

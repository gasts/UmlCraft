public class Switches
{
    public string classic(int day)
    {
        string name;
        switch (day)
        {
            case 1:
                name = "Monday";
                break;
            case 2:
            case 3:
                name = "Midweek";
                break;
            default:
                name = "Other";
                break;
        }
        return name;
    }

    public int defaultInMiddle(int a)
    {
        int r;
        switch (a)
        {
            case 1:
                r = 10;
                break;
            default:
                r = 0;
                break;
            case 2:
                r = 20;
                break;
        }
        return r;
    }

    public void noDefault(int a)
    {
        switch (a)
        {
            case 1:
                work();
                break;
            case 2:
                work();
                break;
        }
    }

    private void work()
    {
    }
}

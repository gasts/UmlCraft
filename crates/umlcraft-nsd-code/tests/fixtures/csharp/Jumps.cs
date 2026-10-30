public class Jumps
{
    public void plainReturn(int a)
    {
        if (a > 0)
        {
            return;
        }
        work();
    }

    public int loops(int[] values)
    {
        int sum = 0;
        foreach (int v in values)
        {
            if (v < 0)
            {
                continue;
            }
            if (v > 100)
            {
                break;
            }
            sum += v;
        }
        return sum;
    }

    public void fail(string message)
    {
        throw new IllegalStateException(message);
    }

    private void work()
    {
    }
}

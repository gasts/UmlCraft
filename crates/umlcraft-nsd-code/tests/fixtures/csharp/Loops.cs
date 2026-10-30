public class Loops
{
    public int counting(int n)
    {
        int sum = 0;
        for (int i = 0; i < n; i++)
        {
            sum += i;
        }
        for (int i = 0; i <= n; i += 2)
        {
            sum += i;
        }
        for (int i = n; i > 0; i--)
        {
            sum -= i;
        }
        return sum;
    }

    public int free(int n)
    {
        int sum = 0;
        for (int i = 0, j = n; i < j; i++, j--)
        {
            sum += j - i;
        }
        for (int i = 0; ; i++)
        {
            if (i > n)
            {
                break;
            }
        }
        return sum;
    }

    public int enhanced(int[] values)
    {
        int sum = 0;
        foreach (int v in values)
        {
            sum += v;
        }
        return sum;
    }

    public int whileLoop(int n)
    {
        while (n > 10)
        {
            n = n / 2;
        }
        return n;
    }

    public int doWhile(int n)
    {
        do
        {
            n--;
        } while (n > 0);
        return n;
    }

    public void endless()
    {
        while (true)
        {
            work();
        }
    }

    public void endlessFor()
    {
        for (;;)
        {
            work();
        }
    }

    private void work()
    {
    }
}

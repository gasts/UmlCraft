using System;
using System.IO;

public class Exceptions
{
    public int tryCatch(string s)
    {
        try
        {
            return Integer.parseInt(s);
        }
        catch (NumberFormatException e)
        {
            return 0;
        }
    }

    public void multiCatch(Reader r)
    {
        try
        {
            r.read();
        }
        catch (IOException e)
        {
            log(e);
        }
        catch (RuntimeException e)
        {
            log(e);
        }
    }

    public void tryFinally(Reader r)
    {
        try
        {
            r.read();
        }
        catch (IOException e)
        {
            log(e);
        }
        finally
        {
            r.close();
        }
    }

    public void onlyFinally(Reader r)
    {
        try
        {
            r.read();
        }
        finally
        {
            r.close();
        }
    }

    private void log(Exception e)
    {
    }
}

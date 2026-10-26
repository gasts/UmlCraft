using System;

namespace Modifiers;

class Defaults
{
    int field;
    void Method() { }
    class Inner { }
}

[Serializable]
public abstract class Visibilities
{
    public int a;
    protected int b;
    internal int c;
    private int d;
    protected internal int e;
    private protected int f;
    public const int Max = 10;
    public static readonly string Name = "v";
    public static int Count;

    public abstract void Run();
    public virtual void Step() { }
    public sealed override string ToString() => "";
    public static void Reset() { }
    [Obsolete("old")]
    public void Old() { }
}

public sealed class Leaf { }

public static class Helpers
{
    public static int Twice(this int x) => x * 2;
}

file class Local { }

public interface IService
{
    void Start();
    void Stop() { }
    static void Create() { }
    event EventHandler Started;
}

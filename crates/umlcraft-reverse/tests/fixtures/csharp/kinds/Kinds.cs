namespace Kinds;

public class Plain
{
    public class Nested
    {
    }
}

public interface IShape
{
    double Area();
}

public struct Point
{
    public int X;
    public int Y;
}

public enum Color
{
    Red,
    Green = 2,
    Blue,
}

public record Money(decimal Amount, string Currency);

public record struct Size(int Width, int Height);

public readonly record struct Range(int From, int To);

public class Account(string owner)
{
}

public delegate void Changed(object sender);

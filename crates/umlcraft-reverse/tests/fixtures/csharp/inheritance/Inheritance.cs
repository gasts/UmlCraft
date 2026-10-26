using System;
using System.Collections.Generic;

namespace Inheritance;

public interface INamed
{
    string Name { get; }
}

public interface ILabeled : INamed, IComparable<ILabeled>
{
}

public interface Shape
{
}

public abstract class Base : IDisposable
{
    public void Dispose() { }
}

// Base first, then interfaces.
public class Derived : Base, ILabeled, ICloneable
{
    public string Name => "";
    public int CompareTo(ILabeled other) => 0;
    public object Clone() => this;
}

// `Shape` is an interface of the sources: a realization, though it comes first.
public class Circle : Shape
{
}

// Unknown names: `IAudited` looks like an interface, `Entity` like a class.
public class Customer : Entity, IAudited
{
}

public class Admin : IUser
{
}

// Library types: `List<T>` is a class, `IEnumerable<T>` an interface.
public class Orders : List<Order>, IEnumerable<Order>
{
}

public class Order
{
}

// A struct's base list holds interfaces only.
public struct Point : IEquatable<Point>, Comparable
{
}

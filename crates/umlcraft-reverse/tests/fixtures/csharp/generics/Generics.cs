using System;
using System.Collections.Generic;

namespace Generics;

public class Box<T> where T : class, IComparable<T>, new()
{
    private T value;
    private Dictionary<string, List<T>> index;

    public U Map<U>(Func<T, U> f) where U : struct => default;
}

public interface IVariant<in TIn, out TOut>
{
    TOut Convert(TIn input);
}

public class Pair<TKey, TValue> : Box<TKey> where TKey : class, IComparable<TKey>, new()
{
}

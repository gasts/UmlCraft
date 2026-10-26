using System;
using System.Collections.Generic;
using System.Collections.Immutable;

namespace Shop;

public class Order
{
    private Customer customer;
    private Customer? payer;
    private List<Line> lines;
    private HashSet<Tag> tags;
    private SortedSet<Tag> sorted;
    private Dictionary<string, Line> byCode;
    private Line[] history;
    private ImmutableArray<Line> frozen;
    private Nullable<Discount> discount;
    public Customer Owner { get; set; }
    public IReadOnlyList<Line> Items { get; }
    private string note;
    private DateTime placed;
    private int count;
}

public class Customer
{
}

public class Line
{
}

public class Tag
{
}

public struct Discount
{
}

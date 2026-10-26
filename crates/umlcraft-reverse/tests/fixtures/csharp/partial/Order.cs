namespace Partial;

/// <summary>An order, in two files.</summary>
public partial class Order : IComparable<Order>
{
    private int number;

    public int CompareTo(Order other) => 0;
}

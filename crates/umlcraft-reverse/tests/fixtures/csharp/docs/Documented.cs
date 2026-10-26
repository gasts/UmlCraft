namespace Docs;

/// <summary>
/// An order.
/// </summary>
/// <remarks>Kept for a year.</remarks>
public class Order
{
    /// <summary>The number.</summary>
    public int Number;

    // A plain comment is no documentation.
    public int Plain;

    /** A block comment. */
    public void Ship() { }

    /// Stops here.

    public int Separated;

    /// <summary>The state.</summary>
    public State Current { get; set; }
}

public enum State
{
    /// Not paid yet.
    Open,
    Paid,
}

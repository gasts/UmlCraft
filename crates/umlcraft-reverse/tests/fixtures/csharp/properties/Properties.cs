namespace Properties;

public class Account
{
    private int balance;

    // Auto-properties: attributes.
    public int Id { get; set; } = 1;
    public string Owner { get; }
    public string Iban { get; init; }
    public string Label { get; private set; }
    public static int Opened { get; set; }

    // Properties with bodies: accessor operations.
    public int Balance
    {
        get { return balance; }
        protected set { balance = value; }
    }

    public bool IsEmpty => balance == 0;

    public int Doubled { get => balance * 2; }

    // Indexers: get_Item and set_Item.
    public int this[int index]
    {
        get => index;
        set { }
    }
}

public abstract class Shape
{
    public abstract double Area { get; }
    public virtual string Name { get; set; }
}

public interface IEntity
{
    int Key { get; set; }
    string Title { get => ""; }
    string this[string name] { get; }
}

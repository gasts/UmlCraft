namespace Model;

/// <summary>An order.</summary>
public class Order
{
    private long number;
    private Customer customer;
    private Customer? backup;

    public void Ship(int days, bool express) { }
    public void Ship(int days) { }
}

public class Customer
{
    private Missing missing;
}

public interface Kind
{
}

public class Missing
{
}

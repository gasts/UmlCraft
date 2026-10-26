namespace Shop
{
    public class Catalog
    {
    }

    namespace Orders
    {
        public class Order
        {
            private Catalog catalog;
        }
    }
}

namespace Shop.Billing
{
    public class Invoice
    {
        private Orders.Order order;
    }
}

public class Global
{
}

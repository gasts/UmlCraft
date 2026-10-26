using Acme.Lib;
using Fmt = Acme.Lib.Text.Formatter;
using Txt = Acme.Lib.Text;
using Ids = System.Collections.Generic.List<int>;
using static System.Math;

namespace Acme.App
{
    public class Shop
    {
        // From a using.
        private Money price;
        // Through an alias.
        private Fmt formatter;
        // Through a namespace alias.
        private Txt.Formatter aliased;
        // Nested type of a used type.
        private Customer.Address address;
        // From the namespace around the own one: `Acme.Lib` relative to `Acme`.
        private Lib.Text.Formatter second;
        // Fully qualified.
        private Acme.Lib.Money third;
        // From the implicit usings.
        private List<Money> prices;
        private DateTime opened;
        // Unknown: a placeholder in the root.
        private Missing missing;
        // Unknown qualified: under its namespace.
        private Other.Things.Widget widget;
    }

    namespace Inner
    {
        public class Clerk
        {
            // `Shop` lives in the namespace around this one.
            private Shop shop;
        }
    }
}

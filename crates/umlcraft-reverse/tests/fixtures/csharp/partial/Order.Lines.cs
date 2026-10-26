using System;

namespace Partial;

public partial class Order : IDisposable
{
    private string customer;

    public void Dispose() { }
}

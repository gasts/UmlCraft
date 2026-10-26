using System;

#region Types
namespace Conditional;

#if DEBUG
public class Diagnostics
{
}
#elif TRACE
public class Tracing
{
}
#else
public class Release
{
}
#endif

public class Service
{
#if NET8_0_OR_GREATER
    public int Modern;
#else
    public int Legacy;
#endif

#pragma warning disable CS0169
    private int unused;
#pragma warning restore CS0169
}
#endregion

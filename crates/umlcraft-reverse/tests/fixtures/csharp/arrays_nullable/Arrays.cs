namespace Arrays;

public class Grid
{
    private int[] cells;
    private int[,] matrix;
    private Item[][] jagged;
    private int? maybe;
    private Item? next;
    private Item?[] slots;
    private (int, string) pair;
    private unsafe int* raw;
    private dynamic anything;
    private global::System.String text;

    public void Fill(ref int start, out int end, in int step, params Item[] items)
    {
        end = 0;
    }

    public void Defaults(int count = 1, string name = "x") { }
}

public class Item
{
}

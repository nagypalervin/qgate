// Fixture: the scenarios of "C# qualified names" and "C# parameter
// lists in names". Asserted in src/csharp.rs.

namespace Shop
{
    class Cart
    {
        Cart(IClock c) { }
        decimal Total(int d) => 0;
        string Owner { get => ""; init { } }
        int this[int i] => 0;

        decimal Total(int discount = 0, string code = "") => 0;
        decimal Total(string code) => 0;
        decimal Total(Dictionary<string, int> m, out int n) { n = 0; return 0; }
        int Paid(List<Order> o) => o.Count(x => x.Paid && !x.Void);
    }

    class Outer
    {
        class Inner
        {
            T Map<T>(ref T x, params int[] ys)
            {
                int Add(int a, int b) => a + b;
                return x;
            }

            public static Inner operator +(Inner a, Inner b) => a;

            ~Inner() { }
        }
    }

    static class Ext
    {
        public static int Count(this string? s, in int start, scoped Span<int> buf) => 0;
    }
}

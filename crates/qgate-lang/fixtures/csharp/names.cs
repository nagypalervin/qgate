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

    class Impl : I, J
    {
        int I.P => 1;
        int J.P => 2;
        int I.this[int i] => 0;
        event EventHandler I.Ev { add { } remove { } }
        static Impl IAdd<Impl>.operator +(Impl a, Impl b) => a;
        public static Impl operator +(Impl a, Impl b) => a;
        public static Impl operator checked +(Impl a, Impl b) => a;
    }

    class Result { static Result Ok() => new(); }
    class Result<T> where T : notnull { static Result<T> Ok() => new(); }
}

// Fixture: one kind of edit per unit; After.cs makes them.
// Asserted in src/csharp.rs.
using System.Linq;

namespace Shop;

public class Cart
{
    private int count = 1;

    public int Layout(int a, int b)
    {
        return a + b;
    }

    public int Comments(int a)
    {
        return a * 2;
    }

    public int Literal() => 1;

    public string Text() => "a";

    public long Typed(int a)
    {
        int b = a;
        return b;
    }

    public int Renamed(int a)
    {
        var local = a;
        return local;
    }

    public int Outer()
    {
        return Inner() + 1;

        int Inner()
        {
            return 1;
        }
    }

    public int[] Query(int[] xs)
    {
        return xs.Where(x => x > 1).ToArray();
    }

    public int Guarded()
    {
#if DEBUG
        return 1;
#else
        return 2;
#endif
    }

    public int Directive()
    {
#if DEBUG
        return 1;
#endif
        return 0;
    }

    public int Regions()
    {
        #region Totals
        return 1;
        #endregion
    }

    public int Count
    {
        get { return count; }
    }
}

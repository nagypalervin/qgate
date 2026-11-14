// Fixture: one kind of edit per unit, made to Before.cs.
// Asserted in src/csharp.rs.
using System.Linq;

namespace Shop;

public class Cart
{
    private int count = 2;

    public int Layout(int a, int b) { return a  +  b; }

    public int Comments(int a)
    {
        // doubled
        return a /* twice */ * 2;
    }

    public int Literal() => 2;

    public string Text() => "b";

    public long Typed(int a)
    {
        long b = a;
        return b;
    }

    public int Renamed(int a)
    {
        var value = a;
        return value;
    }

    public int Outer()
    {
        return Inner() + 1;

        int Inner()
        {
            return 3;
        }
    }

    public int[] Query(int[] xs)
    {
        return xs.Where(x => x > 2).ToArray();
    }

    public int Guarded()
    {
#if DEBUG
        return 1;
#else
        return 3;
#endif
    }

    public int Directive()
    {
#if TRACE
        return 1;
#endif
        return 0;
    }

    public int Regions()
    {
        #region   Sums 
        return 1;
        #endregion
    }

    public int Count
    {
        get { return count; }
    }
}

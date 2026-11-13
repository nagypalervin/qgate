// Fixture: C# function units and their names.
// The expected (name, line, cyclomatic) triples are asserted in
// src/csharp.rs.

namespace Shop.Orders;

public class Cart
{
    public Cart(IClock clock) { }

    public decimal Total(int discount)
    {
        int Twice(int x) => x * 2;
        if (discount > 0) { return Twice(discount); }
        return 0;
    }

    public string Owner
    {
        get { return _owner ?? ""; }
        set { _owner = value; }
    }

    public int Count => _items.Count;

    public static Cart operator +(Cart a, Cart b) => a;

    private string? _owner;
    private readonly List<int> _items = new();
    private readonly Func<int, int> _square = x => x > 0 ? x * x : 0;
}

public interface IShop
{
    void Open();
    int Close(int code);
    void Log() { Console.WriteLine(); }
}

public abstract class Base
{
    public abstract void Run();
    [DllImport("x")] static extern int Native();
    partial void OnRun();
}

public partial class Part { public void A() { } }
public partial class Part { public void A(int x) { } }

public class Disposer : IDisposable
{
    void IDisposable.Dispose() { }
    static Disposer() { }
    public Disposer() { }
    ~Disposer() { }
    public event EventHandler Changed { add { } remove { } }
    public static implicit operator int(Disposer d) => 0;
}

public record Money(decimal Amount)
{
    public Money Add(Money other) => this with { Amount = Amount + other.Amount };
}

public struct Point
{
    public int Length() => 0;
}

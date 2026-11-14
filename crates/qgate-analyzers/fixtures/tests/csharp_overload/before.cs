// Fixture: an overload added next to an existing method.
// Asserted in src/tests.rs.
namespace Shop.Domain;

public class Cart
{
    private readonly List<int> _prices = new();

    public int Total()
    {
        return _prices.Sum();
    }
}

// Fixture: a new class, added by the change without a test.
// Asserted in src/tests.rs.
namespace Shop.Domain;

public record Invoice(decimal Net, decimal Rate)
{
    public decimal Total()
    {
        return Net * (1 + Rate);
    }
}

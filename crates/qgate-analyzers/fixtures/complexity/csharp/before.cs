namespace Shop;

public class Cart
{
    public decimal Total(int discount)
    {
        if (discount > 0) { return 1; }
        if (discount > 5) { return 2; }
        return 0;
    }

    public decimal Shipping(string country) => country == "HU" ? 0 : 5;
}

namespace Shop;

public class Cart
{
    public decimal Total(int discount)
    {
        if (discount > 0) { return 1; }
        if (discount > 10) { return 2; }
        return 0;
    }
}

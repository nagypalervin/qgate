namespace Shop;

public class Cart
{
    public decimal Total(int discount)
    {
        if (discount > 0) { return 1; }
        return 0;
    }
}

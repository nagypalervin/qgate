namespace Shop;

public class Cart
{
    // A new overload above Total(int): not reported (complexity 4), and
    // Total(int) keeps its name.
    public decimal Total(string code)
    {
        if (code == "A") { return 1; }
        if (code == "B") { return 2; }
        return code.Length > 3 ? 3 : 0;
    }

    public decimal Total(int discount)
    {
        if (discount > 0) { return 1; }
        if (discount > 5) { return 2; }
        if (discount > 10) { return 3; }
        if (discount > 20) { return 4; }
        return 0;
    }

    public decimal Shipping(string country) => country switch
    {
        "HU" => 0,
        "AT" => 1,
        "DE" => 2,
        "FR" => 3,
        "IT" => 4,
        _ => country.Length > 2 ? 9 : 5,
    };

    public string Report(int[] xs)
    {
        var text = "";
        foreach (var x in xs)
        {
            if (x > 0 && x < 10) { text += "a"; }
            else if (x > 10 || x < -10) { text += "b"; }
            text += x switch { 1 => "c", 2 => "d", 3 => "e", 4 => "g", _ => "f" };
            text ??= "";
        }
        return text;
    }
}

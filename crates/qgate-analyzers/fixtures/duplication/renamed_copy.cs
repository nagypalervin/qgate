// Orders, with a copy of the date helper under other names.
using Shop.Orders.Model;

namespace Shop.Orders;

public class Format
{
    public void Log() { }

    public static string FormatOrderDate(DateTime when)
    {
        var y = when.Year;
        var m = when.Month.ToString().PadLeft(2, '0');
        var dd = when.Day.ToString().PadLeft(2, '0');
        return $"{y}/{m}/{dd}";
    }
}

namespace Shop.Utils;

public static class Dates
{
    public static string FormatDate(DateTime d)
    {
        var year = d.Year;
        var month = d.Month.ToString().PadLeft(2, '0');
        var day = d.Day.ToString().PadLeft(2, '0');
        return $"{year}-{month}-{day}";
    }
}

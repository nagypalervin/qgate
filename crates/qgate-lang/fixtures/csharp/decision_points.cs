// Fixture: C# decision points, one kind per method, then the scenarios of
// "C# decision points" and "Conditional compilation". The expected
// complexities are asserted in src/csharp.rs.

class D
{
    int If(bool a) { if (a) { } else if (!a) { } else { } return 0; }
    void For() { for (int i = 0; i < 3; i++) { } }
    void Foreach(int[] xs) { foreach (var x in xs) { } }
    void While(int n) { while (n > 0) { n--; } }
    void Do(int n) { do { n--; } while (n > 0); }
    void Cases(int n) { switch (n) { case 1: case 2: break; case 3: break; default: break; } }
    void Guard(object o) { switch (o) { case int n when n > 0: break; } }
    int Arms(int d) => d switch { 0 => 1, 1 => 2, _ => 3 };
    void Catch() { try { } catch (IOException) { } catch (Exception e) when (e.Data != null) { } finally { } }
    int Ternary(bool a) => a ? 1 : 0;
    bool And(bool a, bool b) => a && b;
    bool Or(bool a, bool b) => a || b;
    string Coalesce(string? s) => s ?? "";
    void CoalesceAssign(string? s) { s ??= ""; }
    int? Access(A? a) => a?.B?.C;
    char? Element(string? s) => s?[0];
    bool AndPattern(int x) => x is > 0 and < 50;
    bool OrPattern(int x) => x is 1 or 2 or 3;
    int Optional(int x = 1) { return x; }
    bool NotNull(object? o) => o is not null && !(o is string);
    void Lambda(List<int> xs) { xs.ForEach(x => { if (x > 0) { } }); }
    void Anonymous(List<int> xs) { xs.ForEach(delegate (int x) { if (x > 0) { } }); }
    void Default(int n) { switch (n) { default: break; } }
    int Discard(int d) => d switch { _ => 0 };
    int Goto(int n) { if (n > 0) goto end; n++; end: return n; }
    int Property => _x > 0 ? 1 : 0;
    int Field = 1 > 0 ? 1 : 0;

    // The scenarios.
    void LoopsAndSwitch(int[] xs, int n)
    {
        foreach (var x in xs) { }
        do { n--; } while (n > 0);
        switch (n) { case 1: break; case 2: break; default: break; }
    }

    int F(int d) => d switch { 0 => 1, > 0 and < 50 => 2, _ => 3 };

    string Errors(A? a)
    {
        try { return a?.Name ?? ""; }
        catch (Exception e) when (e is IOException) { return ""; }
        finally { }
    }

    void Guarded(bool a)
    {
#if DEBUG
        if (a) { }
#endif
    }

    // Directives are not evaluated: their conditions do not count.
    void Directives(bool a)
    {
#if DEBUG && (TRACE || NET8)
        if (a) { }
#elif X || Y
        Log(1);
#else
        Log(2);
#endif
    }
}

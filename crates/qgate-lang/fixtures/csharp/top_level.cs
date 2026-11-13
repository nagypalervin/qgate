// Fixture: top-level statements. Only the local function is a unit.
// Asserted in src/csharp.rs.
using System;

if (args.Length > 0)
{
    Console.WriteLine(args[0] ?? "none");
}

int Twice(int x) => x > 0 ? 2 * x : 0;

Console.WriteLine(Twice(args.Length));

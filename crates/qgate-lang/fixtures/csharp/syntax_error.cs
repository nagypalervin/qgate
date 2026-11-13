// Fixture: an #if/#else whose branches each open a block. Not valid C#
// read without the directives; asserted in src/csharp.rs.
namespace App
{
    public class Startup
    {
        public void Configure(object app)
        {
#if DEBUG
            if (app != null)
            {
#else
            if (app == null)
            {
#endif
                Console.WriteLine("x");
            }
        }

#region Helpers
        private int Twice(int x) => x * 2;
#endregion
    }
}

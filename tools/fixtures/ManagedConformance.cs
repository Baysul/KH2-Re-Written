// Authored synthetic fixture. This contains no KH2 data or inferred game rules.
namespace RewriteResearchConformance
{
    public static class BranchFixture
    {
        public static int Classify(int value)
        {
            if (value < 0)
                return -1;
            if (value < 16)
                return (value ^ 0x5a) * 3;
            return (value & 0xff) + 7;
        }

        public static string Label(int value)
        {
            switch (value)
            {
                case -1: return "negative";
                case 0: return "zero";
                default: return "other";
            }
        }
    }
}

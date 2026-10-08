/* Authored native-reader fixture; no game code, data or inferred rules. */
typedef unsigned int uint;
static volatile uint authored_bias = 23u;

__declspec(dllexport) __declspec(noinline) uint FixtureBias(void)
{
    return authored_bias;
}

__declspec(dllexport) __declspec(noinline) uint FixtureTransform(uint value)
{
    if (value < 16u)
        return (value ^ 90u) * 3u;
    return (value & 255u) + 7u;
}

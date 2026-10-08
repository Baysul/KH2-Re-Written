// Owned DDS admission/file adapter; hash-pinned public scanline case statements
// are privately extracted by Build-BgraReference.ps1. Source is read-only and
// output CREATE_NEW. No target code or general DirectXTex importer executes.
#define NOMINMAX
#include <windows.h>
#include <dxgiformat.h>
#include <DirectXMath.h>
#include <DirectXPackedVector.h>
#include <cstdio>
#include <cstring>
#include <stdexcept>
#include <vector>
#include "BgraReferenceBranches.h"
struct Handle {
    HANDLE value;
    ~Handle() { if (value != INVALID_HANDLE_VALUE) CloseHandle(value); }
};
static void write(HANDLE handle, const void* bytes, DWORD length) {
    DWORD count = 0;
    if (!WriteFile(handle, bytes, length, &count, nullptr) || count != length)
        throw std::runtime_error("incomplete private output");
}
static int run(int argc, wchar_t** argv) {
    if (argc != 3) throw std::runtime_error("usage: BgraReference <private-dds> <new-private-rgba>");
    Handle input{CreateFileW(argv[1], GENERIC_READ, FILE_SHARE_READ, nullptr, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr)};
    LARGE_INTEGER length{};
    if (input.value == INVALID_HANDLE_VALUE || !GetFileSizeEx(input.value, &length)
        || length.QuadPart < 128 || length.QuadPart > 32 * 1024 * 1024)
        throw std::runtime_error("DDS input bounds");
    std::vector<BYTE> bytes(static_cast<size_t>(length.QuadPart));
    DWORD read = 0;
    if (!ReadFile(input.value, bytes.data(), static_cast<DWORD>(bytes.size()), &read, nullptr) || read != bytes.size())
        throw std::runtime_error("incomplete private input");
    auto word = [&bytes](size_t offset) {
        uint32_t result = 0; std::memcpy(&result, bytes.data() + offset, 4); return result;
    };
    UINT width = word(16), height = word(12);
    if (std::memcmp(bytes.data(), "DDS ", 4) || word(4) != 124 || word(76) != 32
        || (word(8) & 0x100f) != 0x100f || (word(8) & ~0x2100fu) != 0 || word(20) != width * 4
        || word(24) > 1 || word(28) > 1 || word(108) != 0x1000 || word(112) || word(116) || word(120)
        || word(80) != 0x41 || word(84) || word(88) != 32 || word(92) != 0x00ff0000
        || word(96) != 0x0000ff00 || word(100) != 0xff || word(104) != 0xff000000
        || !width || !height || width > 4096 || height > 4096
        || bytes.size() != 128 + static_cast<size_t>(width) * height * 4)
        throw std::runtime_error("outside single-level tightly packed BGRA8 profile");
    std::vector<DirectX::XMVECTOR> scanline(width);
    std::vector<BYTE> pixels(static_cast<size_t>(width) * height * 4);
    for (size_t y = 0; y < height; ++y) {
        if (!PublicLoadBgra(scanline.data(), width, bytes.data() + 128 + y * width * 4, width * 4)
            || !PublicStoreRgba(pixels.data() + y * width * 4, width * 4, scanline.data(), width))
            throw std::runtime_error("public scanline branch failed");
    }
    Handle output{CreateFileW(argv[2], GENERIC_WRITE, 0, nullptr, CREATE_NEW, FILE_ATTRIBUTE_NORMAL, nullptr)};
    if (output.value == INVALID_HANDLE_VALUE) throw std::runtime_error("output exists or cannot be created");
    write(output.value, "RGBA", 4); write(output.value, &width, 4); write(output.value, &height, 4);
    write(output.value, pixels.data(), static_cast<DWORD>(pixels.size()));
    if (!FlushFileBuffers(output.value)) throw std::runtime_error("private output flush failed");
    std::printf("Public BGRA/RGBA scanline branches: %u x %u, %zu bytes\n", width, height, pixels.size());
    return 0;
}
int wmain(int argc, wchar_t** argv) {
    try { return run(argc, argv); }
    catch (const std::exception& error) { std::fprintf(stderr, "%s\n", error.what()); return 1; }
}

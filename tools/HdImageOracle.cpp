// Owned Windows-only independent PNG/DDS pixel inspection helper.
// Uses explicitly selected native WIC codecs, never target executable code.
// Input is read-only; output is CREATE_NEW. Invoke only via a guarded private
// research workflow. RGBA file: ASCII RGBA, LE width/height, straight pixels.
#define NOMINMAX
#include <windows.h>
#include <wincodec.h>
#include <wrl/client.h>
#include <cstdio>
#include <cstring>
#include <stdexcept>
#include <vector>

using Microsoft::WRL::ComPtr;
struct Handle {
    HANDLE value;
    ~Handle() { if (value != INVALID_HANDLE_VALUE) CloseHandle(value); }
};
static void check(HRESULT hr, const char* operation) {
    if (FAILED(hr)) {
        std::fprintf(stderr, "%s failed: 0x%08lx\n", operation, static_cast<unsigned long>(hr));
        throw std::runtime_error("native codec error");
    }
}
static void write(HANDLE handle, const void* bytes, DWORD length) {
    DWORD written = 0;
    if (!WriteFile(handle, bytes, length, &written, nullptr) || written != length)
        throw std::runtime_error("incomplete private pixel output");
}
static int run(int argc, wchar_t** argv) {
    if (argc != 3) throw std::runtime_error("usage: HdImageOracle <private-image> <new-private-rgba>");
    Handle input{CreateFileW(argv[1], GENERIC_READ, FILE_SHARE_READ, nullptr, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr)};
    LARGE_INTEGER length{};
    if (input.value == INVALID_HANDLE_VALUE || !GetFileSizeEx(input.value, &length)
        || length.QuadPart < 8 || length.QuadPart > 32 * 1024 * 1024)
        throw std::runtime_error("invalid bounded input file");
    std::vector<BYTE> bytes(static_cast<size_t>(length.QuadPart));
    DWORD count = 0;
    if (!ReadFile(input.value, bytes.data(), static_cast<DWORD>(bytes.size()), &count, nullptr) || count != bytes.size())
        throw std::runtime_error("incomplete input read");
    const BYTE png[] = {137,80,78,71,13,10,26,10};
    const CLSID* codec = nullptr;
    if (!std::memcmp(bytes.data(), png, 8)) codec = &CLSID_WICPngDecoder;
    else if (!std::memcmp(bytes.data(), "DDS ", 4)) codec = &CLSID_WICDdsDecoder;
    else throw std::runtime_error("unsupported image signature");
    ComPtr<IWICImagingFactory> factory;
    check(CoCreateInstance(CLSID_WICImagingFactory, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&factory)), "factory");
    ComPtr<IWICStream> stream;
    check(factory->CreateStream(&stream), "stream");
    check(stream->InitializeFromMemory(bytes.data(), static_cast<DWORD>(bytes.size())), "memory stream");
    ComPtr<IWICBitmapDecoder> decoder;
    check(CoCreateInstance(*codec, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&decoder)), "native decoder");
    check(decoder->Initialize(stream.Get(), WICDecodeMetadataCacheOnDemand), "decoder initialize");
    UINT frames = 0;
    check(decoder->GetFrameCount(&frames), "frame count");
    if (frames != 1) throw std::runtime_error("only one image frame is admitted");
    ComPtr<IWICBitmapFrameDecode> frame;
    check(decoder->GetFrame(0, &frame), "frame");
    UINT width = 0, height = 0;
    check(frame->GetSize(&width, &height), "dimensions");
    if (!width || !height || width > 4096 || height > 4096)
        throw std::runtime_error("image dimensions exceed bounds");
    ComPtr<IWICFormatConverter> converter;
    check(factory->CreateFormatConverter(&converter), "converter");
    check(converter->Initialize(frame.Get(), GUID_WICPixelFormat32bppRGBA,
        WICBitmapDitherTypeNone, nullptr, 0, WICBitmapPaletteTypeCustom), "straight RGBA converter");
    std::vector<BYTE> pixels(static_cast<size_t>(width) * height * 4);
    check(converter->CopyPixels(nullptr, width * 4, static_cast<UINT>(pixels.size()), pixels.data()), "pixels");
    Handle output{CreateFileW(argv[2], GENERIC_WRITE, 0, nullptr, CREATE_NEW, FILE_ATTRIBUTE_NORMAL, nullptr)};
    if (output.value == INVALID_HANDLE_VALUE) throw std::runtime_error("private output cannot be created (never overwrite)");
    write(output.value, "RGBA", 4);
    write(output.value, &width, 4);
    write(output.value, &height, 4);
    write(output.value, pixels.data(), static_cast<DWORD>(pixels.size()));
    if (!FlushFileBuffers(output.value)) throw std::runtime_error("private output flush failed");
    std::printf("Native WIC straight RGBA: %u x %u, %zu bytes\n", width, height, pixels.size());
    return 0;
}
int wmain(int argc, wchar_t** argv) {
    HRESULT initialized = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
    if (FAILED(initialized)) return 1;
    int result = 1;
    try { result = run(argc, argv); }
    catch (const std::exception& error) { std::fprintf(stderr, "%s\n", error.what()); }
    CoUninitialize();
    return result;
}

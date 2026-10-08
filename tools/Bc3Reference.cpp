// Owned adapter for privately extracted, hash-pinned Microsoft DirectXTex
// DecodeBC1/D3DXDecodeBC3 methods. Public methods and BC.h stay outside Git.
// Never load target code. Input read-only; bounded output is CREATE_NEW.
#define NOMINMAX
#include <windows.h>
#include <algorithm>
#include <cassert>
#include <cstdio>
#include <cstring>
#include <stdexcept>
#include <vector>
#include "BC.h"
#include "Bc3ReferenceMethods.h"
#ifdef BC3_MIP_CHAINS
#include "Bc3PitchBranch.h"
#endif

struct Handle {
    HANDLE value;
    ~Handle() { if (value != INVALID_HANDLE_VALUE) CloseHandle(value); }
};
static void write(HANDLE handle, const void* bytes, DWORD length) {
    DWORD count = 0;
    if (!WriteFile(handle, bytes, length, &count, nullptr) || count != length)
        throw std::runtime_error("incomplete private pixel write");
}
static int run(int argc, wchar_t** argv) {
    if (argc != 3) throw std::runtime_error("usage: Bc3Reference <private-dds> <new-private-rgba>");
    Handle input{CreateFileW(argv[1], GENERIC_READ, FILE_SHARE_READ, nullptr, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr)};
    LARGE_INTEGER length{};
    if (input.value == INVALID_HANDLE_VALUE || !GetFileSizeEx(input.value, &length)
        || length.QuadPart < 128 || length.QuadPart > 32 * 1024 * 1024)
        throw std::runtime_error("invalid bounded DDS input");
    std::vector<BYTE> bytes(static_cast<size_t>(length.QuadPart));
    DWORD count = 0;
    if (!ReadFile(input.value, bytes.data(), static_cast<DWORD>(bytes.size()), &count, nullptr) || count != bytes.size())
        throw std::runtime_error("incomplete private input read");
    auto word = [&bytes](size_t offset) {
        uint32_t value = 0;
        std::memcpy(&value, bytes.data() + offset, 4);
        return value;
    };
    UINT width = word(16), height = word(12);
#ifdef BC3_MIP_CHAINS
    UINT levels = std::max(1u,word(28));
    if (std::memcmp(bytes.data(),"DDS ",4) || word(4)!=124 || word(76)!=32
        || word(80)!=4 || std::memcmp(bytes.data()+84,"DXT5",4)
        || (word(8)&0x81007)!=0x81007 || (word(8)&~0xa1007u)
        || word(24)>1 || word(112) || word(116) || word(120)
        || !width || !height || width>4096 || height>4096
        || (levels>1 && ((word(8)&0x20000)==0 || word(108)!=0x401008))
        || (levels==1 && word(108)!=0x1000))
        throw std::runtime_error("unsupported bounded BC3 mip header");
    UINT maximum=1;
    for(UINT edge=std::max(width,height);edge>1;edge/=2) ++maximum;
    if(levels>maximum) throw std::runtime_error("excessive mip count");
    struct Level { UINT width,height; uint64_t offset,length; };
    std::vector<Level> layout;
    size_t offset=128,rgbaBytes=0;
    for(UINT ordinal=0;ordinal<levels;++ordinal) {
        const size_t encoded=PublicBc3Pitch(width,height);
        if(!ordinal && word(20)!=encoded) throw std::runtime_error("base linear size differs");
        rgbaBytes+=static_cast<size_t>(width)*height*4;
        if(rgbaBytes>64*1024*1024) throw std::runtime_error("cumulative pixel budget exceeded");
        layout.push_back({width,height,static_cast<uint64_t>(offset),static_cast<uint64_t>(encoded)});
        offset+=encoded;width=std::max(1u,width/2);height=std::max(1u,height/2);
    }
    if(offset!=bytes.size()) throw std::runtime_error("complete mip payload length differs");
    // Validate the entire layout before creating the single new output.
    Handle output{CreateFileW(argv[2],GENERIC_WRITE,0,nullptr,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,nullptr)};
    if(output.value==INVALID_HANDLE_VALUE) throw std::runtime_error("private output exists or cannot be created");
    write(output.value,"MIPS",4);write(output.value,&levels,4);
    for(const auto& level:layout) {
        std::vector<BYTE> pixels(static_cast<size_t>(level.width)*level.height*4);
        const size_t columns=(static_cast<size_t>(level.width)+3)/4;
        for(size_t block=0;block<level.length/16;++block) {
            DirectX::XMVECTOR colors[16];
            DirectX::D3DXDecodeBC3(colors,bytes.data()+static_cast<size_t>(level.offset)+block*16);
            for(size_t p=0;p<16;++p) {
                const size_t x=(block%columns)*4+p%4,y=(block/columns)*4+p/4;
                if(x>=level.width || y>=level.height) continue;
                DirectX::PackedVector::XMUBYTEN4 packed;
                const auto biased=DirectX::XMVectorAdd(colors[p],DirectX::XMVectorReplicate(0.5f/255.f));
                DirectX::PackedVector::XMStoreUByteN4(&packed,biased);
                std::memcpy(pixels.data()+(y*level.width+x)*4,&packed,4);
            }
        }
        write(output.value,&level.width,4);write(output.value,&level.height,4);
        write(output.value,&level.offset,8);write(output.value,&level.length,8);
        write(output.value,pixels.data(),static_cast<DWORD>(pixels.size()));
    }
    if(!FlushFileBuffers(output.value)) throw std::runtime_error("private output flush failed");
    std::printf("Pinned public pitch/BC3/RGBA8: %u levels, %zu pixel bytes\n",levels,rgbaBytes);
#else
    if (std::memcmp(bytes.data(), "DDS ", 4) || word(4) != 124 || word(76) != 32
        || (word(8) & 0x1007) != 0x1007 || (word(80) & 4) == 0
        || std::memcmp(bytes.data() + 84, "DXT5", 4) || word(112) != 0
        || word(24) > 1 || word(28) > 1 || (word(108) & 0x1000) == 0
        || !width || !height || width > 4096 || height > 4096 || width % 4 || height % 4
        || bytes.size() != 128 + static_cast<size_t>(width / 4) * (height / 4) * 16)
        throw std::runtime_error("unsupported legacy DDS surface/header/length");
    std::vector<BYTE> pixels(static_cast<size_t>(width) * height * 4);
    for (size_t block = 0; block < (bytes.size() - 128) / 16; ++block) {
        DirectX::XMVECTOR colors[16];
        DirectX::D3DXDecodeBC3(colors, bytes.data() + 128 + block * 16);
        for (size_t p = 0; p < 16; ++p) {
            DirectX::PackedVector::XMUBYTEN4 packed;
            // Same explicit RGBA8 bias/store route as pinned StoreScanline.
            const auto biased = DirectX::XMVectorAdd(colors[p], DirectX::XMVectorReplicate(0.5f / 255.f));
            DirectX::PackedVector::XMStoreUByteN4(&packed, biased);
            const size_t x = (block % (width / 4)) * 4 + p % 4;
            const size_t y = (block / (width / 4)) * 4 + p / 4;
            std::memcpy(pixels.data() + (y * width + x) * 4, &packed, 4);
        }
    }
    Handle output{CreateFileW(argv[2], GENERIC_WRITE, 0, nullptr, CREATE_NEW, FILE_ATTRIBUTE_NORMAL, nullptr)};
    if (output.value == INVALID_HANDLE_VALUE) throw std::runtime_error("private output exists or cannot be created");
    write(output.value, "RGBA", 4);
    write(output.value, &width, 4);
    write(output.value, &height, 4);
    write(output.value, pixels.data(), static_cast<DWORD>(pixels.size()));
    if (!FlushFileBuffers(output.value)) throw std::runtime_error("private output flush failed");
    std::printf("Pinned public BC3/RGBA8: %u x %u, %zu bytes\n", width, height, pixels.size());
#endif
    return 0;
}
int wmain(int argc, wchar_t** argv) {
    try { return run(argc, argv); }
    catch (const std::exception& error) { std::fprintf(stderr, "%s\n", error.what()); return 1; }
}

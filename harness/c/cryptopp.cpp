#include <cstddef>
#include <string>

#include "lsh.h"

namespace {

using CryptoPP::byte;

// Crypto++ has no class for LSH-512-224. Its LSH512_Base computes it from
// KISA's algorithm type 0x1001C, as LSH512_256 does from 0x10020.
class LSH512_224 : public CryptoPP::LSH512_Base {
public:
    LSH512_224() : LSH512_Base(0x001001C, 28) { Restart(); }
    std::string AlgorithmName() const { return "LSH-512-224"; }
};

template <class H>
void digest(const byte *msg, size_t msglen, byte *out) {
    thread_local H hash;
    hash.CalculateDigest(out, msg, msglen);
}

const char *path(const std::string &provider) {
    if (provider == "AVX2") return "avx2";
    if (provider == "SSSE3") return "ssse3";
    if (provider == "C++") return "ref";
    return "unknown";
}

}

extern "C" int cryptopp_lsh256(const byte *msg, size_t msglen, byte *out, size_t outlen) {
    switch (outlen) {
    case 28: digest<CryptoPP::LSH224>(msg, msglen, out); return 0;
    case 32: digest<CryptoPP::LSH256>(msg, msglen, out); return 0;
    default: return -1;
    }
}

extern "C" int cryptopp_lsh512(const byte *msg, size_t msglen, byte *out, size_t outlen) {
    switch (outlen) {
    case 28: digest<LSH512_224>(msg, msglen, out); return 0;
    case 32: digest<CryptoPP::LSH512_256>(msg, msglen, out); return 0;
    case 48: digest<CryptoPP::LSH384>(msg, msglen, out); return 0;
    case 64: digest<CryptoPP::LSH512>(msg, msglen, out); return 0;
    default: return -1;
    }
}

extern "C" const char *cryptopp_lsh256_path() {
    return path(CryptoPP::LSH256().AlgorithmProvider());
}

extern "C" const char *cryptopp_lsh512_path() {
    return path(CryptoPP::LSH512().AlgorithmProvider());
}

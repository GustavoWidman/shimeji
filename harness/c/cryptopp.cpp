// Exports Crypto++'s LSH-256 and LSH-512 to the harness.

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

// Hashes the message with one object per thread, which CalculateDigest
// leaves ready for the next message.
template <class H>
void digest(const byte *msg, size_t msglen, byte *out) {
    thread_local H hash;
    hash.CalculateDigest(out, msg, msglen);
}

// The code path Crypto++ takes on this CPU, in the harness's names.
const char *path(const std::string &provider) {
    if (provider == "AVX2") return "avx2";
    if (provider == "SSSE3") return "ssse3";
    if (provider == "C++") return "ref";
    return "unknown";
}

}  // namespace

// Hashes the msglen-byte message at msg with LSH-256 and writes the
// outlen-byte digest (28 or 32) to out. Returns 0, or -1 for any other
// digest length.
extern "C" int cryptopp_lsh256(const byte *msg, size_t msglen, byte *out, size_t outlen) {
    switch (outlen) {
    case 28: digest<CryptoPP::LSH224>(msg, msglen, out); return 0;
    case 32: digest<CryptoPP::LSH256>(msg, msglen, out); return 0;
    default: return -1;
    }
}

// cryptopp_lsh256 for LSH-512, whose digests are 28, 32, 48 or 64 bytes.
extern "C" int cryptopp_lsh512(const byte *msg, size_t msglen, byte *out, size_t outlen) {
    switch (outlen) {
    case 28: digest<LSH512_224>(msg, msglen, out); return 0;
    case 32: digest<CryptoPP::LSH512_256>(msg, msglen, out); return 0;
    case 48: digest<CryptoPP::LSH384>(msg, msglen, out); return 0;
    case 64: digest<CryptoPP::LSH512>(msg, msglen, out); return 0;
    default: return -1;
    }
}

// The path ("ref", "ssse3" or "avx2") Crypto++'s LSH-256 takes on this CPU.
extern "C" const char *cryptopp_lsh256_path() {
    return path(CryptoPP::LSH256().AlgorithmProvider());
}

// The path Crypto++'s LSH-512 takes on this CPU.
extern "C" const char *cryptopp_lsh512_path() {
    return path(CryptoPP::LSH512().AlgorithmProvider());
}

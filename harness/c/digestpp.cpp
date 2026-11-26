// Exports digestpp's LSH-256 and LSH-512 to the harness.

#include <cstddef>

#include "algorithm/lsh.hpp"

// Hashes the msglen-byte message at msg with LSH-256 and writes the
// outlen-byte digest (28 or 32) to out, with a new hasher for each message
// as in digestpp::lsh256(256).absorb(msg, msglen).digest(out, 32). Returns 0,
// or -1 for any other digest length.
extern "C" int digestpp_lsh256(const unsigned char *msg, size_t msglen, unsigned char *out,
                               size_t outlen) {
    if (outlen != 28 && outlen != 32) return -1;
    digestpp::lsh256(8 * outlen).absorb(msg, msglen).digest(out, outlen);
    return 0;
}

// digestpp_lsh256 for LSH-512, whose digests are 28, 32, 48 or 64 bytes.
extern "C" int digestpp_lsh512(const unsigned char *msg, size_t msglen, unsigned char *out,
                               size_t outlen) {
    if (outlen != 28 && outlen != 32 && outlen != 48 && outlen != 64) return -1;
    digestpp::lsh512(8 * outlen).absorb(msg, msglen).digest(out, outlen);
    return 0;
}

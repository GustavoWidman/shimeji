#include <cstddef>

#include "algorithm/lsh.hpp"

extern "C" int digestpp_lsh256(const unsigned char *msg, size_t msglen, unsigned char *out,
                               size_t outlen) {
    if (outlen != 28 && outlen != 32) return -1;
    digestpp::lsh256(8 * outlen).absorb(msg, msglen).digest(out, outlen);
    return 0;
}

extern "C" int digestpp_lsh512(const unsigned char *msg, size_t msglen, unsigned char *out,
                               size_t outlen) {
    if (outlen != 28 && outlen != 32 && outlen != 48 && outlen != 64) return -1;
    digestpp::lsh512(8 * outlen).absorb(msg, msglen).digest(out, outlen);
    return 0;
}

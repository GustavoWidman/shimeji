// Package main exports go-krypto's LSH-256 and LSH-512 to C. build.rs
// builds it with -buildmode=c-archive and links it into the harness.
package main

/*
#include <stddef.h>
#include <stdint.h>
*/
import "C"

import (
	"unsafe"

	"github.com/RyuaNerin/go-krypto/lsh256"
	"github.com/RyuaNerin/go-krypto/lsh512"
)

// bytes views n bytes at p as a slice.
func bytes(p *C.uchar, n C.size_t) []byte {
	return unsafe.Slice((*byte)(unsafe.Pointer(p)), int(n))
}

// gokrypto_lsh256 hashes the msglen-byte message at msg iters times with
// LSH-256 and writes the outlen-byte digest (28 or 32) to out. It returns
// 0, or -1 for any other digest length.
//
//export gokrypto_lsh256
func gokrypto_lsh256(msg *C.uchar, msglen C.size_t, out *C.uchar, outlen C.size_t, iters C.uint64_t) C.int {
	m, o := bytes(msg, msglen), bytes(out, outlen)
	switch outlen {
	case lsh256.Size224:
		for i := C.uint64_t(0); i < iters; i++ {
			d := lsh256.Sum224(m)
			copy(o, d[:])
		}
	case lsh256.Size:
		for i := C.uint64_t(0); i < iters; i++ {
			d := lsh256.Sum256(m)
			copy(o, d[:])
		}
	default:
		return -1
	}
	return 0
}

// gokrypto_lsh512 is gokrypto_lsh256 for LSH-512, whose digests are 28, 32,
// 48 or 64 bytes.
//
//export gokrypto_lsh512
func gokrypto_lsh512(msg *C.uchar, msglen C.size_t, out *C.uchar, outlen C.size_t, iters C.uint64_t) C.int {
	m, o := bytes(msg, msglen), bytes(out, outlen)
	switch outlen {
	case lsh512.Size224:
		for i := C.uint64_t(0); i < iters; i++ {
			d := lsh512.Sum224(m)
			copy(o, d[:])
		}
	case lsh512.Size256:
		for i := C.uint64_t(0); i < iters; i++ {
			d := lsh512.Sum256(m)
			copy(o, d[:])
		}
	case lsh512.Size384:
		for i := C.uint64_t(0); i < iters; i++ {
			d := lsh512.Sum384(m)
			copy(o, d[:])
		}
	case lsh512.Size:
		for i := C.uint64_t(0); i < iters; i++ {
			d := lsh512.Sum512(m)
			copy(o, d[:])
		}
	default:
		return -1
	}
	return 0
}

func main() {}

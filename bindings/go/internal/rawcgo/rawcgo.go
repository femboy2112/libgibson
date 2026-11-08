// Package rawcgo is a test-only helper: raw cgo calls that the _test.go files of this module
// can't make themselves (cgo is not allowed in _test.go). Nothing outside tests imports it.
package rawcgo

/*
#cgo pkg-config: libgibson
#include <gibson.h>
*/
import "C"

// PolluteLastError makes the calling OS thread's last-error text "Null pointer provided for out
// context" (a NULL out-pointer to gibson_create_context). It is deliberately a different text from
// the one the wrapper's own failing call produces, so a foreign message is recognizable.
func PolluteLastError() {
	C.gibson_create_context(C.int32_t(C.GIBSON_MODE_INLINE), nil)
}

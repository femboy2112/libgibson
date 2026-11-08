// LibGibson v0.8 parity slice -- Go consumer.
//
// Interprets bindings/parity/v08/script.txt against the INSTALLED SDK (cgo, `#cgo pkg-config:
// libgibson`, `#include <gibson.h>`) and writes the canonical transcript to stdout. Format:
// docs/V08_PARITY_SLICE.md.
//
//	parity_v08_go SCRIPT [REPS]
//
// Surface used: the bindings/go wrapper (package gibson) gates the run with gibson.CheckABI().
// The wrapper has NO headless context, no push/poll, no frame readback and no styled text node,
// and its handles are unexported, so a wrapper Node cannot be mounted on a context created
// elsewhere: every script operation therefore goes through this package's own cgo calls against
// the installed header. That split is itself a measured result (docs/V08_PARITY_SLICE.md, "Go").
//
// gibson_last_error_message is thread-local, and a goroutine may move between OS threads between
// two cgo calls, so main locks the OS thread before any library call.
//
// Negative controls (never on by default): PARITY_V08_PERTURB=cell | events (see parity_v08.c).
package main

/*
#cgo pkg-config: libgibson
#include <gibson.h>
#include <stdlib.h>

static uint8_t cell_trunc(const gibson_cell_t *c) { return c->_reserved[0]; }
*/
import "C"

import (
	"bufio"
	"encoding/hex"
	"fmt"
	"os"
	"reflect"
	"runtime"
	"sort"
	"strconv"
	"strings"
	"syscall"
	"time"
	"unsafe"

	"github.com/femboy2112/libgibson/bindings/go/gibson"
)

const (
	fnvOffset uint64 = 0xcbf29ce484222325
	fnvPrime  uint64 = 0x100000001b3
)

func fnv(h uint64, b []byte) uint64 {
	for _, c := range b {
		h ^= uint64(c)
		h *= fnvPrime
	}
	return h
}

// rec maps a key to its already-rendered JSON value; emit sorts the keys.
type rec map[string]string

func safe(s string) string {
	for i := 0; i < len(s); i++ {
		c := s[i]
		if c < 0x20 || c > 0x7e || c == '"' || c == '\\' {
			return "hex:" + hex.EncodeToString([]byte(s))
		}
	}
	return s
}
func (r rec) i(k string, v int64) rec  { r[k] = strconv.FormatInt(v, 10); return r }
func (r rec) s(k string, v string) rec { r[k] = "\"" + safe(v) + "\""; return r }
func (r rec) a4(k string, a, b, c, d int64) rec {
	r[k] = fmt.Sprintf("[%d,%d,%d,%d]", a, b, c, d)
	return r
}
func (r rec) emit(out *strings.Builder) {
	keys := make([]string, 0, len(r))
	for k := range r {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	out.WriteString("{")
	for n, k := range keys {
		if n > 0 {
			out.WriteString(",")
		}
		out.WriteString("\"" + k + "\":" + r[k])
	}
	out.WriteString("}\n")
}

func stName(s C.gibson_status_t) string {
	switch int(s) {
	case 0:
		return "OK"
	case -1:
		return "INVALID_PARAM"
	case -2:
		return "IO"
	case -3:
		return "PANIC"
	case -4:
		return "NOT_INITIALIZED"
	case -5:
		return "QUEUE_FULL"
	}
	return "UNKNOWN"
}
func kindName(k C.uint32_t) string {
	switch k {
	case C.GIBSON_KEY_KIND_CHAR:
		return "CHAR"
	case C.GIBSON_KEY_KIND_SPECIAL:
		return "SPECIAL"
	}
	return "NONE"
}
func typeName(t C.int32_t) string {
	switch t {
	case C.GIBSON_EVENT_KEY:
		return "KEY"
	case C.GIBSON_EVENT_PASTE:
		return "PASTE"
	case C.GIBSON_EVENT_RESIZE:
		return "RESIZE"
	}
	return "NONE"
}
func lastErr() string {
	var buf [512]C.char
	C.gibson_last_error_message(&buf[0], 512)
	return C.GoString(&buf[0])
}

var special = map[string]uint32{
	"ENTER": C.GIBSON_KEY_ENTER, "LEFT": C.GIBSON_KEY_LEFT, "RIGHT": C.GIBSON_KEY_RIGHT,
	"UP": C.GIBSON_KEY_UP, "DOWN": C.GIBSON_KEY_DOWN, "HOME": C.GIBSON_KEY_HOME,
	"END": C.GIBSON_KEY_END, "BACKSPACE": C.GIBSON_KEY_BACKSPACE, "DELETE": C.GIBSON_KEY_DELETE,
	"ESC": C.GIBSON_KEY_ESC, "TAB": C.GIBSON_KEY_TAB, "PAGE_UP": C.GIBSON_KEY_PAGE_UP,
	"PAGE_DOWN": C.GIBSON_KEY_PAGE_DOWN,
}

func loadScript(path string) ([][]string, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()
	var steps [][]string
	sc := bufio.NewScanner(f)
	for sc.Scan() {
		line := strings.TrimRight(sc.Text(), "\r")
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		steps = append(steps, strings.Split(line, " "))
	}
	return steps, sc.Err()
}

func applyPerturbation(steps [][]string) {
	switch os.Getenv("PARITY_V08_PERTURB") {
	case "cell":
		for _, s := range steps {
			if s[0] == "s06" {
				h := s[len(s)-1]
				s[len(s)-1] = h[:len(h)-2] + "3f"
			}
		}
	case "events":
		a, b := -1, -1
		for i, s := range steps {
			if s[0] == "e01" {
				a = i
			}
			if s[0] == "e02" {
				b = i
			}
		}
		if a >= 0 && b >= 0 {
			steps[a], steps[b] = steps[b], steps[a]
		}
	}
}

func atoi(s string) int { n, _ := strconv.Atoi(s); return n }
func unhex(s string) []byte { b, _ := hex.DecodeString(s); return b }
func cpOf(u string) uint32 { n, _ := strconv.ParseUint(u[2:], 16, 32); return uint32(n) }

func parseColor(spec string) C.gibson_color_t {
	var c C.gibson_color_t
	switch {
	case spec == "reset":
		c.color_type = C.GIBSON_COLOR_RESET
	case strings.HasPrefix(spec, "rgb:"):
		p := strings.Split(spec[4:], ",")
		c.color_type = C.GIBSON_COLOR_RGB
		c.r, c.g, c.b = C.uint8_t(atoi(p[0])), C.uint8_t(atoi(p[1])), C.uint8_t(atoi(p[2]))
	case strings.HasPrefix(spec, "ansi:"):
		c.color_type = C.GIBSON_COLOR_ANSI256
		c.r = C.uint8_t(atoi(spec[5:]))
	case strings.HasPrefix(spec, "named:"):
		c.color_type = C.int32_t(atoi(spec[6:]))
	}
	return c
}

func wrapOf(w string) C.int32_t {
	switch w {
	case "CHAR":
		return C.GIBSON_WRAP_CHAR
	case "WORD":
		return C.GIBSON_WRAP_WORD
	}
	return C.GIBSON_WRAP_NONE
}

func attrs(s *C.gibson_style_t) string {
	b := []byte("00000")
	for i, v := range []C.uint8_t{s.bold, s.dim, s.italic, s.underline, s.reverse} {
		if v != 0 {
			b[i] = '1'
		}
	}
	return string(b)
}

type interp struct {
	out    *strings.Builder
	ctx    *C.gibson_context_t
	root   *C.gibson_node_t
	styles map[string]*C.gibson_style_t
}

func (in *interp) fullGlyph(c *C.gibson_cell_t, x, y C.uint16_t) []byte {
	if C.cell_trunc(c) == 0 {
		return C.GoBytes(unsafe.Pointer(&c.glyph[0]), C.int(c.glyph_len))
	}
	var n C.size_t
	C.gibson_get_frame_cell_glyph(in.ctx, x, y, nil, 0, &n)
	buf := C.malloc(n + 1)
	defer C.free(buf)
	C.gibson_get_frame_cell_glyph(in.ctx, x, y, (*C.uint8_t)(buf), n, &n)
	return C.GoBytes(buf, C.int(n))
}

func (in *interp) cellLine(c *C.gibson_cell_t, x, y C.uint16_t) string {
	s := &c.style
	return fmt.Sprintf("%s;%d;%d;%d;%d,%d,%d,%d;%d,%d,%d,%d;%s\n",
		hex.EncodeToString(in.fullGlyph(c, x, y)), c.width, c.is_continuation, c.transparent,
		s.fg.color_type, s.fg.r, s.fg.g, s.fg.b, s.bg.color_type, s.bg.r, s.bg.g, s.bg.b, attrs(s))
}

func (in *interp) emitEvent(id string, st C.gibson_status_t, ev *C.gibson_event_t, kind string) {
	key, rs := ev.event_type == C.GIBSON_EVENT_KEY, ev.event_type == C.GIBSON_EVENT_RESIZE
	r := rec{}
	var cols, rows, code, mods int64
	if rs {
		cols, rows = int64(ev.cols), int64(ev.rows)
	}
	if key {
		code, mods = int64(ev.key_code), int64(ev.modifiers)
	}
	paste := ""
	if ev.event_type == C.GIBSON_EVENT_PASTE && ev.paste_str != nil {
		paste = hex.EncodeToString([]byte(C.GoString(ev.paste_str)))
	}
	r.i("cols", cols).s("id", id).s("k", "event").i("key_code", code).s("kind", kind).i("mods", mods).
		s("paste", paste).i("rows", rows).s("st", stName(st)).s("type", typeName(ev.event_type)).emit(in.out)
}

func (in *interp) probe(id, name string) {
	r := rec{}
	r.s("id", id).s("k", "probe").s("name", name)
	var cols, rows C.uint16_t
	C.gibson_get_frame_size(in.ctx, &cols, &rows)
	var c2 *C.gibson_context_t
	var cell C.gibson_cell_t
	var ev C.gibson_event_t
	var st C.gibson_status_t = 999
	withErr := false
	switch name {
	case "create_bad_mode":
		st, withErr = C.gibson_create_headless_context(7, 40, 12, &c2), true
	case "create_zero_cols":
		st, withErr = C.gibson_create_headless_context(C.GIBSON_MODE_FULLSCREEN, 0, 12, &c2), true
	case "create_null_out":
		st = C.gibson_create_headless_context(C.GIBSON_MODE_FULLSCREEN, 40, 12, nil)
	case "render_null_ctx":
		st = C.gibson_render(nil)
	case "cell_oob_x":
		st = C.gibson_get_frame_cell(in.ctx, cols, 0, &cell)
	case "cell_oob_y":
		st = C.gibson_get_frame_cell(in.ctx, 0, rows, &cell)
	case "cells_oob":
		var written C.size_t = 99
		st = C.gibson_get_frame_cells(in.ctx, 0, rows, &cell, 1, &written)
		r.i("written", int64(written))
	case "cell_null_out":
		st = C.gibson_get_frame_cell(in.ctx, 0, 0, nil)
	case "text_bad_utf8":
		cs := C.CString("\xff\xfe")
		var n *C.gibson_node_t
		st, withErr = C.gibson_node_text(cs, nil, C.GIBSON_WRAP_NONE, &n), true
		C.free(unsafe.Pointer(cs))
		if st == C.GIBSON_OK {
			C.gibson_node_free(n)
		}
	case "push_null_event":
		st = C.gibson_push_event(in.ctx, nil)
	case "push_surrogate":
		ev.event_type, ev.key_code = C.GIBSON_EVENT_KEY, 0xD800
		st, withErr = C.gibson_push_event_kind(in.ctx, &ev, C.GIBSON_KEY_KIND_CHAR), true
	case "push_special_not_key":
		ev.event_type, ev.key_code = C.GIBSON_EVENT_KEY, 'a'
		st = C.gibson_push_event_kind(in.ctx, &ev, C.GIBSON_KEY_KIND_SPECIAL)
	case "push_bad_kind":
		ev.event_type, ev.key_code = C.GIBSON_EVENT_KEY, 'a'
		st = C.gibson_push_event_kind(in.ctx, &ev, 9)
	case "push_bad_mods":
		ev.event_type, ev.key_code, ev.modifiers = C.GIBSON_EVENT_KEY, 'a', 0x80
		st = C.gibson_push_event_kind(in.ctx, &ev, C.GIBSON_KEY_KIND_CHAR)
	case "push_resize_zero":
		ev.event_type, ev.cols, ev.rows = C.GIBSON_EVENT_RESIZE, 0, 12
		st = C.gibson_push_event(in.ctx, &ev)
	case "paste_bad_utf8":
		cs := C.CString("\xc3\x28")
		ev.event_type, ev.paste_str = C.GIBSON_EVENT_PASTE, cs
		st, withErr = C.gibson_push_event(in.ctx, &ev), true
		C.free(unsafe.Pointer(cs))
	case "paste_null":
		ev.event_type, ev.paste_str = C.GIBSON_EVENT_PASTE, nil
		st = C.gibson_push_event(in.ctx, &ev)
	case "poll_null_out":
		st = C.gibson_poll_event(in.ctx, 0, nil)
	case "poll_kind_null":
		st = C.gibson_poll_event_kind(in.ctx, 0, &ev, nil)
	case "color_depth_bad":
		st = C.gibson_set_color_depth(in.ctx, 9)
	case "queue_full":
		var accepted, drained int64
		ev.event_type, ev.key_code = C.GIBSON_EVENT_KEY, 'q'
		for i := 0; i < C.GIBSON_EVENT_QUEUE_CAPACITY; i++ {
			if C.gibson_push_event_kind(in.ctx, &ev, C.GIBSON_KEY_KIND_CHAR) == C.GIBSON_OK {
				accepted++
			}
		}
		st = C.gibson_push_event_kind(in.ctx, &ev, C.GIBSON_KEY_KIND_CHAR)
		e := lastErr()
		for i := 0; i < 2*C.GIBSON_EVENT_QUEUE_CAPACITY; i++ {
			var got C.gibson_event_t
			var kind C.uint32_t
			if C.gibson_poll_event_kind(in.ctx, 0, &got, &kind) != C.GIBSON_OK || got.event_type == C.GIBSON_EVENT_NONE {
				break
			}
			drained++
		}
		r.i("accepted", accepted).i("drained", drained).s("err", e)
	case "frame_before_render":
		var c, rr C.uint16_t = 7, 7
		C.gibson_create_headless_context(C.GIBSON_MODE_FULLSCREEN, 8, 2, &c2)
		sst := C.gibson_get_frame_size(c2, &c, &rr)
		st = C.gibson_get_frame_cell(c2, 0, 0, &cell)
		C.gibson_destroy_context(c2)
		c2 = nil
		r.i("cols", int64(c)).i("rows", int64(rr)).s("size_st", stName(sst))
	}
	if withErr {
		r.s("err", lastErr())
	}
	if c2 != nil {
		C.gibson_destroy_context(c2)
	}
	if st == 999 {
		r.s("st", "UNKNOWN_PROBE")
	} else {
		r.s("st", stName(st))
	}
	r.emit(in.out)
}

func (in *interp) step(s []string) {
	id, op, a := s[0], s[2], s[3:]
	out := in.out
	switch {
	case op == "create":
		var mode C.int32_t = C.GIBSON_MODE_FULLSCREEN
		if a[0] == "INLINE" {
			mode = C.GIBSON_MODE_INLINE
		}
		st := C.gibson_create_headless_context(mode, C.uint16_t(atoi(a[1])), C.uint16_t(atoi(a[2])), &in.ctx)
		rec{}.i("cols", int64(atoi(a[1]))).s("id", id).s("k", "create").s("mode", a[0]).
			i("rows", int64(atoi(a[2]))).s("st", stName(st)).emit(out)
	case op == "style":
		st := (*C.gibson_style_t)(C.calloc(1, C.size_t(unsafe.Sizeof(C.gibson_style_t{}))))
		st.fg, st.bg = parseColor(a[1]), parseColor(a[2])
		f := a[3]
		b := func(i int) C.uint8_t {
			if f[i] == '1' {
				return 1
			}
			return 0
		}
		st.bold, st.dim, st.italic, st.underline, st.reverse = b(0), b(1), b(2), b(3), b(4)
		in.styles[a[0]] = st
	case op == "col":
		st := C.gibson_node_box_col(&in.root)
		rec{}.s("id", id).s("k", "node").s("st", stName(st)).emit(out)
	case op == "text":
		cs := C.CString(string(unhex(a[2])))
		var n *C.gibson_node_t
		st := C.gibson_node_text(cs, in.styles[a[0]], wrapOf(a[1]), &n)
		C.free(unsafe.Pointer(cs))
		if st == C.GIBSON_OK {
			st = C.gibson_node_add_child(in.root, n)
		}
		rec{}.s("id", id).s("k", "node").s("st", stName(st)).emit(out)
	case op == "mount":
		st := C.gibson_set_root_node(in.ctx, in.root) // consumes the tree
		in.root = nil
		rec{}.s("id", id).s("k", "mount").s("st", stName(st)).emit(out)
	case op == "render":
		rec{}.s("id", id).s("k", "render").s("st", stName(C.gibson_render(in.ctx))).emit(out)
	case op == "geom":
		var c, r C.uint16_t
		st := C.gibson_get_frame_size(in.ctx, &c, &r)
		rec{}.i("cols", int64(c)).s("id", id).s("k", "geom").i("rows", int64(r)).s("st", stName(st)).emit(out)
	case op == "frame":
		var c, r C.uint16_t
		st := C.gibson_get_frame_size(in.ctx, &c, &r)
		total := int(c) * int(r)
		cells := make([]C.gibson_cell_t, total+1)
		var written C.size_t
		if st == C.GIBSON_OK {
			st = C.gibson_get_frame_cells(in.ctx, 0, 0, &cells[0], C.size_t(total), &written)
		}
		h := fnvOffset
		for k := 0; k < int(written); k++ {
			h = fnv(h, []byte(in.cellLine(&cells[k], C.uint16_t(k%int(c)), C.uint16_t(k/int(c)))))
		}
		var wl, got C.size_t
		C.gibson_get_rendered_output(in.ctx, nil, 0, &wl)
		wire := C.malloc(wl + 1)
		C.gibson_get_rendered_output(in.ctx, (*C.uint8_t)(wire), wl, &got)
		wb := C.GoBytes(wire, C.int(got))
		C.free(wire)
		rec{}.i("cells", int64(written)).s("digest", fmt.Sprintf("%016x", h)).s("id", id).s("k", "frame").
			s("st", stName(st)).s("wire_digest", fmt.Sprintf("%016x", fnv(fnvOffset, wb))).
			i("wire_len", int64(got)).emit(out)
	case op == "cell":
		x, y := C.uint16_t(atoi(a[0])), C.uint16_t(atoi(a[1]))
		var c C.gibson_cell_t
		st := C.gibson_get_frame_cell(in.ctx, x, y, &c)
		var g []byte
		if st == C.GIBSON_OK {
			g = in.fullGlyph(&c, x, y)
		}
		sy := &c.style
		rec{}.s("attrs", attrs(sy)).
			a4("bg", int64(sy.bg.color_type), int64(sy.bg.r), int64(sy.bg.g), int64(sy.bg.b)).
			i("cont", int64(c.is_continuation)).
			a4("fg", int64(sy.fg.color_type), int64(sy.fg.r), int64(sy.fg.g), int64(sy.fg.b)).
			s("glyph", hex.EncodeToString(g)).s("id", id).s("k", "cell").s("st", stName(st)).
			i("transp", int64(c.transparent)).i("w", int64(c.width)).i("x", int64(x)).i("y", int64(y)).emit(out)
	case op == "glyph":
		x, y := C.uint16_t(atoi(a[0])), C.uint16_t(atoi(a[1]))
		var n, n2 C.size_t
		st := C.gibson_get_frame_cell_glyph(in.ctx, x, y, nil, 0, &n)
		buf := C.malloc(n + 1)
		if st == C.GIBSON_OK {
			st = C.gibson_get_frame_cell_glyph(in.ctx, x, y, (*C.uint8_t)(buf), n, &n2)
		}
		g := C.GoBytes(buf, C.int(n2))
		C.free(buf)
		rec{}.s("full_hex", hex.EncodeToString(g)).i("full_len", int64(n2)).s("id", id).s("k", "glyph").
			s("st", stName(st)).i("x", int64(x)).i("y", int64(y)).emit(out)
	case op == "glyphcap":
		x, y := C.uint16_t(atoi(a[0])), C.uint16_t(atoi(a[1]))
		var c C.gibson_cell_t
		st := C.gibson_get_frame_cell(in.ctx, x, y, &c)
		rec{}.s("id", id).i("inline_len", int64(c.glyph_len)).s("k", "glyphcap").s("st", stName(st)).
			i("trunc", int64(C.cell_trunc(&c))).i("x", int64(x)).i("y", int64(y)).emit(out)
	case strings.HasPrefix(op, "push_"):
		var ev C.gibson_event_t
		var st C.gibson_status_t
		switch op {
		case "push_char", "push_legacy":
			ev.event_type, ev.key_code, ev.modifiers = C.GIBSON_EVENT_KEY, C.uint32_t(cpOf(a[0])), C.uint8_t(atoi(a[1]))
			if op == "push_char" {
				st = C.gibson_push_event_kind(in.ctx, &ev, C.GIBSON_KEY_KIND_CHAR)
			} else {
				st = C.gibson_push_event(in.ctx, &ev)
			}
		case "push_special":
			ev.event_type, ev.key_code, ev.modifiers = C.GIBSON_EVENT_KEY, C.uint32_t(special[a[0]]), C.uint8_t(atoi(a[1]))
			st = C.gibson_push_event_kind(in.ctx, &ev, C.GIBSON_KEY_KIND_SPECIAL)
		case "push_paste":
			cs := C.CString(string(unhex(a[0])))
			ev.event_type, ev.paste_str = C.GIBSON_EVENT_PASTE, cs
			st = C.gibson_push_event(in.ctx, &ev)
			C.free(unsafe.Pointer(cs))
		default:
			ev.event_type, ev.cols, ev.rows = C.GIBSON_EVENT_RESIZE, C.uint16_t(atoi(a[0])), C.uint16_t(atoi(a[1]))
			st = C.gibson_push_event(in.ctx, &ev)
		}
		rec{}.s("id", id).s("k", "push").s("st", stName(st)).emit(out)
	case op == "poll":
		for k := 1; k <= atoi(a[0]); k++ {
			var ev C.gibson_event_t
			var kind C.uint32_t
			st := C.gibson_poll_event_kind(in.ctx, 0, &ev, &kind)
			in.emitEvent(fmt.Sprintf("%s.%d", id, k), st, &ev, kindName(kind))
		}
	case op == "poll_legacy":
		var ev C.gibson_event_t
		st := C.gibson_poll_event(in.ctx, 0, &ev)
		in.emitEvent(id, st, &ev, "ABSENT")
	case op == "probe":
		in.probe(id, a[0])
	case op == "lifecycle":
		var st C.gibson_status_t
		switch a[0] {
		case "node_free_unattached":
			cs := C.CString("unattached")
			var n *C.gibson_node_t
			st = C.gibson_node_text(cs, nil, C.GIBSON_WRAP_NONE, &n)
			C.free(unsafe.Pointer(cs))
			C.gibson_node_free(n)
		case "destroy_null":
			C.gibson_destroy_context(nil)
		}
		rec{}.s("id", id).s("k", "lifecycle").s("name", a[0]).s("st", stName(st)).emit(out)
	case op == "destroy":
		C.gibson_destroy_context(in.ctx)
		in.ctx = nil
		rec{}.s("id", id).s("k", "destroy").s("st", "OK").emit(out)
	default:
		rec{}.s("id", id).s("k", "unknown_op").s("st", "UNSUPPORTED").emit(out)
	}
}

func runScript(steps [][]string) string {
	var out strings.Builder
	in := &interp{out: &out, styles: map[string]*C.gibson_style_t{}}
	for _, s := range steps {
		in.step(s)
	}
	if in.root != nil {
		C.gibson_node_free(in.root)
	}
	if in.ctx != nil {
		C.gibson_destroy_context(in.ctx)
	}
	for _, st := range in.styles {
		C.free(unsafe.Pointer(st))
	}
	return out.String()
}

func maxrssKB() int64 {
	var ru syscall.Rusage
	syscall.Getrusage(syscall.RUSAGE_SELF, &ru)
	return int64(ru.Maxrss)
}

func main() {
	runtime.LockOSThread() // gibson_last_error_message is per OS thread
	if len(os.Args) < 2 {
		fmt.Fprintln(os.Stderr, "usage: parity_v08_go SCRIPT [REPS]")
		os.Exit(2)
	}
	if err := gibson.CheckABI(); err != nil { // the bindings/go wrapper's ABI gate
		fmt.Fprintln(os.Stderr, err)
		os.Exit(2)
	}
	reps := 1
	if len(os.Args) > 2 {
		if reps = atoi(os.Args[2]); reps < 1 {
			reps = 1
		}
	}
	steps, err := loadScript(os.Args[1])
	if err != nil || len(steps) == 0 {
		fmt.Fprintln(os.Stderr, "parity_v08_go: cannot read", os.Args[1])
		os.Exit(2)
	}
	applyPerturbation(steps)

	// Wrapper-surface observation (stderr, not part of the comparable transcript): which methods
	// does the bindings/go *gibson.Context offer for this script's operations?
	{
		ct := reflect.TypeOf(&gibson.Context{})
		var b strings.Builder
		r := rec{}.s("k", "wrapper").s("lang", "go")
		for _, m := range []string{"PollEvent", "PollEventKind", "PushEvent", "PushEventKind", "FrameSize", "FrameCells", "RenderedOutput"} {
			has := int64(0)
			if _, ok := ct.MethodByName(m); ok {
				has = 1
			}
			r.i("ctx_has_"+m, has)
		}
		r.emit(&b)
		os.Stderr.WriteString(b.String())
	}

	t0 := time.Now()
	first := runScript(steps)
	rss1 := maxrssKB()
	rssMid := rss1
	identical := 1
	for rep := 2; rep <= reps; rep++ {
		if runScript(steps) != first {
			identical = 0
		}
		if rep == (reps+1)/2 {
			rssMid = maxrssKB()
		}
	}
	wall := time.Since(t0).Microseconds()
	os.Stdout.WriteString(first)
	if reps > 1 {
		var b strings.Builder
		rec{}.i("iter_identical", int64(identical)).s("k", "resource").s("lang", "go").i("maxrss_kb_1", rss1).
			i("maxrss_kb_mid", rssMid).i("maxrss_kb_n", maxrssKB()).i("per_rep_us", wall/int64(reps)).i("reps", int64(reps)).
			i("wall_ms", wall/1000).emit(&b)
		os.Stderr.WriteString(b.String())
	}
}

// LibGibson v0.8 parity slice -- Go consumer.
//
// Interprets bindings/parity/v08/script.txt against the INSTALLED SDK (cgo, `#cgo pkg-config:
// libgibson`, `#include <gibson.h>`) and writes the canonical transcript to stdout. Format:
// docs/V08_PARITY_SLICE.md.
//
//	parity_v08_go SCRIPT [REPS]
//
// Surface used: the bindings/go wrapper (package gibson): CheckABI, NewHeadlessContext, NewColNode,
// NewStyledTextNode, SetRootNode, Render, PushEvent, PollEventKind / PollEvent, FrameSize,
// FrameCell(s), FrameCellGlyph / FullGlyph, RenderedOutput, Close, with *gibson.Error carrying the
// status and last-error text. What the wrapper cannot express still goes through this package's own
// cgo (each marked "raw cgo"): NULL pointers, destroy(NULL), set_color_depth, and the cells_oob
// out-parameter. Those calls use Context.UnsafeHandle() for the context. That residual split is
// itself a measured result (docs/V08_PARITY_SLICE.md, "Go").
//
// gibson_last_error_message is thread-local, and a goroutine may move between OS threads between
// two cgo calls, so main locks the OS thread before any library call (the raw-cgo probes need it;
// the wrapper reads the last error inside the failing call itself).
//
// Negative controls (never on by default): PARITY_V08_PERTURB=cell | events (see parity_v08.c).
package main

/*
#cgo pkg-config: libgibson
#include <gibson.h>
#include <stdlib.h>

*/
import "C"

import (
	"bufio"
	"encoding/hex"
	"errors"
	"fmt"
	"os"
	"reflect"
	"runtime"
	"sort"
	"strconv"
	"strings"
	"syscall"
	"time"

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

func stName(s C.gibson_status_t) string { return statusName(int(s)) }

func statusName(s int) string {
	switch s {
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

// errName is the transcript status of a wrapper call: OK, the C status of a *gibson.Error, or
// WRAPPER_RAISED for any other error (a wrapper-side refusal that never reached the ABI).
func errName(err error) string {
	if err == nil {
		return "OK"
	}
	var ge *gibson.Error
	if errors.As(err, &ge) {
		return statusName(int(ge.Status))
	}
	return "WRAPPER_RAISED"
}

// errText is the last-error text a *gibson.Error carried ("" otherwise).
func errText(err error) string {
	var ge *gibson.Error
	if errors.As(err, &ge) {
		return ge.LastError
	}
	return ""
}

func kindName(k gibson.KeyKind) string {
	switch k {
	case gibson.KeyKindChar:
		return "CHAR"
	case gibson.KeyKindSpecial:
		return "SPECIAL"
	}
	return "NONE"
}
func typeName(t gibson.EventType) string {
	switch t {
	case gibson.EventKey:
		return "KEY"
	case gibson.EventPaste:
		return "PASTE"
	case gibson.EventResize:
		return "RESIZE"
	}
	return "NONE"
}

var special = map[string]uint32{
	"ENTER": gibson.KeyEnter, "LEFT": gibson.KeyLeft, "RIGHT": gibson.KeyRight,
	"UP": gibson.KeyUp, "DOWN": gibson.KeyDown, "HOME": gibson.KeyHome,
	"END": gibson.KeyEnd, "BACKSPACE": gibson.KeyBackspace, "DELETE": gibson.KeyDelete,
	"ESC": gibson.KeyEsc, "TAB": gibson.KeyTab, "PAGE_UP": gibson.KeyPageUp,
	"PAGE_DOWN": gibson.KeyPageDown,
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

func atoi(s string) int     { n, _ := strconv.Atoi(s); return n }
func unhex(s string) []byte { b, _ := hex.DecodeString(s); return b }
func cpOf(u string) uint32  { n, _ := strconv.ParseUint(u[2:], 16, 32); return uint32(n) }

func parseColor(spec string) gibson.Color {
	var c gibson.Color
	switch {
	case spec == "reset":
		c.Type = gibson.ColorReset
	case strings.HasPrefix(spec, "rgb:"):
		p := strings.Split(spec[4:], ",")
		c.Type = gibson.ColorRGB
		c.R, c.G, c.B = uint8(atoi(p[0])), uint8(atoi(p[1])), uint8(atoi(p[2]))
	case strings.HasPrefix(spec, "ansi:"):
		c.Type = gibson.ColorANSI256
		c.R = uint8(atoi(spec[5:]))
	case strings.HasPrefix(spec, "named:"):
		c.Type = gibson.ColorType(atoi(spec[6:]))
	}
	return c
}

func wrapOf(w string) gibson.WrapMode {
	switch w {
	case "CHAR":
		return gibson.WrapChar
	case "WORD":
		return gibson.WrapWord
	}
	return gibson.WrapNone
}

func attrs(s gibson.Style) string {
	b := []byte("00000")
	for i, v := range []bool{s.Bold, s.Dim, s.Italic, s.Underline, s.Reverse} {
		if v {
			b[i] = '1'
		}
	}
	return string(b)
}

func b01(v bool) int64 {
	if v {
		return 1
	}
	return 0
}

type interp struct {
	out    *strings.Builder
	ctx    *gibson.Context
	root   *gibson.Node
	styles map[string]*gibson.Style
}

// handle is the native context pointer, for the raw-cgo probes only.
func (in *interp) handle() *C.gibson_context_t {
	return (*C.gibson_context_t)(in.ctx.UnsafeHandle())
}

func (in *interp) cellLine(c gibson.Cell, x, y uint16) string {
	g, _ := in.ctx.FullGlyph(c, x, y)
	s := c.Style
	return fmt.Sprintf("%s;%d;%d;%d;%d,%d,%d,%d;%d,%d,%d,%d;%s\n",
		hex.EncodeToString(g), c.Width, b01(c.IsContinuation), b01(c.Transparent),
		s.Fg.Type, s.Fg.R, s.Fg.G, s.Fg.B, s.Bg.Type, s.Bg.R, s.Bg.G, s.Bg.B, attrs(s))
}

func (in *interp) emitEvent(id, st string, ev gibson.Event, kind string) {
	key, rs := ev.Type == gibson.EventKey, ev.Type == gibson.EventResize
	r := rec{}
	var cols, rows, code, mods int64
	if rs {
		cols, rows = int64(ev.Cols), int64(ev.Rows)
	}
	if key {
		code, mods = int64(ev.KeyCode), int64(ev.Modifiers)
	}
	paste := ""
	if ev.Type == gibson.EventPaste && ev.HasPaste {
		paste = hex.EncodeToString([]byte(ev.Paste))
	}
	r.i("cols", cols).s("id", id).s("k", "event").i("key_code", code).s("kind", kind).i("mods", mods).
		s("paste", paste).i("rows", rows).s("st", st).s("type", typeName(ev.Type)).emit(in.out)
}

func (in *interp) probe(id, name string) {
	r := rec{}
	r.s("id", id).s("k", "probe").s("name", name)
	cols, rows, _ := in.ctx.FrameSize()
	var cell C.gibson_cell_t
	st := "UNKNOWN_PROBE"
	errStr := ""
	withErr := false
	push := func(ev gibson.Event) error { return in.ctx.PushEvent(ev) }
	// Probes marked "raw cgo" pass something the wrapper cannot express (NULL pointers, a call with
	// no wrapper); the rest go through the gibson package and read the status from *gibson.Error.
	switch name {
	case "create_bad_mode":
		c, err := gibson.NewHeadlessContext(gibson.RenderMode(7), 40, 12)
		if c != nil {
			c.Close()
		}
		st, errStr, withErr = errName(err), errText(err), true
	case "create_zero_cols":
		c, err := gibson.NewHeadlessContext(gibson.ModeFullscreen, 0, 12)
		if c != nil {
			c.Close()
		}
		st, errStr, withErr = errName(err), errText(err), true
	case "create_null_out": // raw cgo: NULL out pointer
		st = stName(C.gibson_create_headless_context(C.GIBSON_MODE_FULLSCREEN, 40, 12, nil))
	case "render_null_ctx": // raw cgo: NULL context
		st = stName(C.gibson_render(nil))
	case "cell_oob_x":
		_, err := in.ctx.FrameCell(cols, 0)
		st = errName(err)
	case "cell_oob_y":
		_, err := in.ctx.FrameCell(0, rows)
		st = errName(err)
	case "cells_oob": // raw cgo: the C out-parameter `written` is the probed value
		var written C.size_t = 99
		st = stName(C.gibson_get_frame_cells(in.handle(), 0, C.uint16_t(rows), &cell, 1, &written))
		r.i("written", int64(written))
	case "cell_null_out": // raw cgo: NULL out pointer
		st = stName(C.gibson_get_frame_cell(in.handle(), 0, 0, nil))
	case "text_bad_utf8":
		n, err := gibson.NewStyledTextNode("\xff\xfe", nil, gibson.WrapNone)
		if n != nil {
			n.Free()
		}
		st, errStr, withErr = errName(err), errText(err), true
	case "push_null_event": // raw cgo: the wrapper always passes a struct
		st = stName(C.gibson_push_event(in.handle(), nil))
	case "push_surrogate":
		err := push(gibson.CharKey(0xD800, 0))
		st, errStr, withErr = errName(err), errText(err), true
	case "push_special_not_key":
		st = errName(push(gibson.SpecialKey('a', 0)))
	case "push_bad_kind":
		ev := gibson.CharKey('a', 0)
		ev.KeyKind = 9
		st = errName(push(ev))
	case "push_bad_mods":
		st = errName(push(gibson.CharKey('a', 0x80)))
	case "push_resize_zero":
		st = errName(push(gibson.ResizeEvent(0, 12)))
	case "paste_bad_utf8":
		err := push(gibson.PasteText("\xc3\x28"))
		st, errStr, withErr = errName(err), errText(err), true
	case "paste_null":
		st = errName(push(gibson.Event{Type: gibson.EventPaste})) // HasPaste false: NULL paste_str
	case "poll_null_out": // raw cgo: NULL out pointer
		st = stName(C.gibson_poll_event(in.handle(), 0, nil))
	case "poll_kind_null": // raw cgo: NULL kind pointer
		var ev C.gibson_event_t
		st = stName(C.gibson_poll_event_kind(in.handle(), 0, &ev, nil))
	case "color_depth_bad": // raw cgo: no wrapper call for set_color_depth
		st = stName(C.gibson_set_color_depth(in.handle(), 9))
	case "queue_full":
		var accepted, drained int64
		q := gibson.CharKey('q', 0)
		for i := 0; i < gibson.EventQueueCapacity; i++ {
			if push(q) == nil {
				accepted++
			}
		}
		err := push(q)
		st, errStr = errName(err), errText(err)
		for i := 0; i < 2*gibson.EventQueueCapacity; i++ {
			got, perr := in.ctx.PollEventKind(0)
			if perr != nil || got.Type == gibson.EventNone {
				break
			}
			drained++
		}
		r.i("accepted", accepted).i("drained", drained).s("err", errStr)
	case "frame_before_render":
		c2, err := gibson.NewHeadlessContext(gibson.ModeFullscreen, 8, 2)
		if err == nil {
			// The C probe seeds the out-params with 7; the wrapper returns (0, 0) for "no frame yet".
			c, rr, serr := c2.FrameSize()
			_, cerr := c2.FrameCell(0, 0)
			st = errName(cerr)
			r.i("cols", int64(c)).i("rows", int64(rr)).s("size_st", errName(serr))
			c2.Close()
		}
	}
	if withErr {
		r.s("err", errStr)
	}
	r.s("st", st)
	r.emit(in.out)
}

func (in *interp) step(s []string) {
	id, op, a := s[0], s[2], s[3:]
	out := in.out
	switch {
	case op == "create":
		mode := gibson.ModeFullscreen
		if a[0] == "INLINE" {
			mode = gibson.ModeInline
		}
		ctx, err := gibson.NewHeadlessContext(mode, uint16(atoi(a[1])), uint16(atoi(a[2])))
		if err == nil {
			in.ctx = ctx
		}
		rec{}.i("cols", int64(atoi(a[1]))).s("id", id).s("k", "create").s("mode", a[0]).
			i("rows", int64(atoi(a[2]))).s("st", errName(err)).emit(out)
	case op == "style":
		f := a[3]
		in.styles[a[0]] = &gibson.Style{
			Fg: parseColor(a[1]), Bg: parseColor(a[2]),
			Bold: f[0] == '1', Dim: f[1] == '1', Italic: f[2] == '1', Underline: f[3] == '1', Reverse: f[4] == '1',
		}
	case op == "col":
		in.root = gibson.NewColNode() // the constructor returns no status: an invalid node is the signal
		st := "OK"
		if !in.root.Valid() {
			st = "WRAPPER_NULL"
		}
		rec{}.s("id", id).s("k", "node").s("st", st).emit(out)
	case op == "text":
		n, err := gibson.NewStyledTextNode(string(unhex(a[2])), in.styles[a[0]], wrapOf(a[1]))
		if err == nil {
			in.root.AddChild(n) // AddChild returns no status
		}
		rec{}.s("id", id).s("k", "node").s("st", errName(err)).emit(out)
	case op == "mount":
		err := in.ctx.SetRootNode(in.root) // consumes the tree
		rec{}.s("id", id).s("k", "mount").s("st", errName(err)).emit(out)
	case op == "render":
		st := "OK" // Context.Render returns a plain error without the C status
		if err := in.ctx.Render(); err != nil {
			st = "WRAPPER_RAISED"
		}
		rec{}.s("id", id).s("k", "render").s("st", st).emit(out)
	case op == "geom":
		c, r, err := in.ctx.FrameSize()
		rec{}.i("cols", int64(c)).s("id", id).s("k", "geom").i("rows", int64(r)).s("st", errName(err)).emit(out)
	case op == "frame":
		c, _, err := in.ctx.FrameSize()
		var cells []gibson.Cell
		if err == nil {
			cells, err = in.ctx.FrameCells(0, 0, -1)
		}
		h := fnvOffset
		for k := range cells {
			h = fnv(h, []byte(in.cellLine(cells[k], uint16(k%int(c)), uint16(k/int(c)))))
		}
		wb, _ := in.ctx.RenderedOutput()
		rec{}.i("cells", int64(len(cells))).s("digest", fmt.Sprintf("%016x", h)).s("id", id).s("k", "frame").
			s("st", errName(err)).s("wire_digest", fmt.Sprintf("%016x", fnv(fnvOffset, wb))).
			i("wire_len", int64(len(wb))).emit(out)
	case op == "cell":
		x, y := uint16(atoi(a[0])), uint16(atoi(a[1]))
		c, err := in.ctx.FrameCell(x, y)
		var g []byte
		if err == nil {
			g, _ = in.ctx.FullGlyph(c, x, y)
		}
		sy := c.Style
		rec{}.s("attrs", attrs(sy)).
			a4("bg", int64(sy.Bg.Type), int64(sy.Bg.R), int64(sy.Bg.G), int64(sy.Bg.B)).
			i("cont", b01(c.IsContinuation)).
			a4("fg", int64(sy.Fg.Type), int64(sy.Fg.R), int64(sy.Fg.G), int64(sy.Fg.B)).
			s("glyph", hex.EncodeToString(g)).s("id", id).s("k", "cell").s("st", errName(err)).
			i("transp", b01(c.Transparent)).i("w", int64(c.Width)).i("x", int64(x)).i("y", int64(y)).emit(out)
	case op == "glyph":
		x, y := uint16(atoi(a[0])), uint16(atoi(a[1]))
		g, err := in.ctx.FrameCellGlyph(x, y)
		rec{}.s("full_hex", hex.EncodeToString(g)).i("full_len", int64(len(g))).s("id", id).s("k", "glyph").
			s("st", errName(err)).i("x", int64(x)).i("y", int64(y)).emit(out)
	case op == "glyphcap":
		x, y := uint16(atoi(a[0])), uint16(atoi(a[1]))
		c, err := in.ctx.FrameCell(x, y)
		rec{}.s("id", id).i("inline_len", int64(len(c.Glyph))).s("k", "glyphcap").s("st", errName(err)).
			i("trunc", b01(c.GlyphTruncated)).i("x", int64(x)).i("y", int64(y)).emit(out)
	case strings.HasPrefix(op, "push_"):
		var ev gibson.Event
		switch op {
		case "push_char":
			ev = gibson.CharKey(cpOf(a[0]), uint8(atoi(a[1])))
		case "push_legacy":
			ev = gibson.LegacyKey(cpOf(a[0]), uint8(atoi(a[1]))) // the kind-less call
		case "push_special":
			ev = gibson.SpecialKey(special[a[0]], uint8(atoi(a[1])))
		case "push_paste":
			ev = gibson.PasteText(string(unhex(a[0])))
		default:
			ev = gibson.ResizeEvent(uint16(atoi(a[0])), uint16(atoi(a[1])))
		}
		rec{}.s("id", id).s("k", "push").s("st", errName(in.ctx.PushEvent(ev))).emit(out)
	case op == "poll":
		for k := 1; k <= atoi(a[0]); k++ {
			ev, err := in.ctx.PollEventKind(0)
			in.emitEvent(fmt.Sprintf("%s.%d", id, k), errName(err), ev, kindName(ev.KeyKind))
		}
	case op == "poll_legacy":
		ev, err := in.ctx.PollEvent(0) // the kind-less call
		in.emitEvent(id, errName(err), ev, "ABSENT")
	case op == "probe":
		in.probe(id, a[0])
	case op == "lifecycle":
		st := "OK"
		switch a[0] {
		case "node_free_unattached":
			n, err := gibson.NewStyledTextNode("unattached", nil, gibson.WrapNone)
			st = errName(err)
			if n != nil {
				n.Free()
			}
		case "destroy_null":
			C.gibson_destroy_context(nil) // raw cgo: NULL context
		}
		rec{}.s("id", id).s("k", "lifecycle").s("name", a[0]).s("st", st).emit(out)
	case op == "destroy":
		in.ctx.Close()
		in.ctx = nil
		rec{}.s("id", id).s("k", "destroy").s("st", "OK").emit(out)
	default:
		rec{}.s("id", id).s("k", "unknown_op").s("st", "UNSUPPORTED").emit(out)
	}
}

func runScript(steps [][]string) string {
	var out strings.Builder
	in := &interp{out: &out, styles: map[string]*gibson.Style{}}
	for _, s := range steps {
		in.step(s)
	}
	if in.root != nil {
		in.root.Free() // no-op once mounted
	}
	if in.ctx != nil {
		in.ctx.Close()
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
		for _, m := range []string{"PollEvent", "PollEventKind", "PushEvent", "FrameSize", "FrameCells", "RenderedOutput", "TakeRenderedOutput"} {
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

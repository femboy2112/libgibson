package gibson

/*
#cgo pkg-config: libgibson
#include <gibson.h>
#include <stdlib.h>

// Every helper below makes the ABI call and, when it fails, reads the thread-local last error in
// the SAME cgo call. Go may move a goroutine to another OS thread between two cgo calls, and the
// native last error belongs to the thread (see gibson_go_create_context in gibson.go).

static int32_t gibson_go_headless(int32_t mode, uint16_t cols, uint16_t rows, gibson_context_t **out, char *err, size_t n) {
	int32_t st = gibson_create_headless_context(mode, cols, rows, out);
	if (st != GIBSON_OK) gibson_last_error_message(err, n);
	return st;
}
static int32_t gibson_go_push(gibson_context_t *ctx, const gibson_event_t *ev, int has_kind, uint32_t kind, char *err, size_t n) {
	int32_t st = has_kind ? gibson_push_event_kind(ctx, ev, kind) : gibson_push_event(ctx, ev);
	if (st != GIBSON_OK) gibson_last_error_message(err, n);
	return st;
}
static int32_t gibson_go_poll(gibson_context_t *ctx, uint32_t timeout_ms, gibson_event_t *ev, int with_kind, uint32_t *kind, char *err, size_t n) {
	int32_t st = with_kind ? gibson_poll_event_kind(ctx, timeout_ms, ev, kind) : gibson_poll_event(ctx, timeout_ms, ev);
	if (st != GIBSON_OK) gibson_last_error_message(err, n);
	return st;
}
static int32_t gibson_go_frame_size(gibson_context_t *ctx, uint16_t *c, uint16_t *r, char *err, size_t n) {
	int32_t st = gibson_get_frame_size(ctx, c, r);
	if (st != GIBSON_OK) gibson_last_error_message(err, n);
	return st;
}
static int32_t gibson_go_frame_cell(gibson_context_t *ctx, uint16_t x, uint16_t y, gibson_cell_t *out, char *err, size_t n) {
	int32_t st = gibson_get_frame_cell(ctx, x, y, out);
	if (st != GIBSON_OK) gibson_last_error_message(err, n);
	return st;
}
static int32_t gibson_go_frame_cells(gibson_context_t *ctx, uint16_t x, uint16_t y, gibson_cell_t *out, size_t max, size_t *written, char *err, size_t n) {
	int32_t st = gibson_get_frame_cells(ctx, x, y, out, max, written);
	if (st != GIBSON_OK) gibson_last_error_message(err, n);
	return st;
}
static int32_t gibson_go_cell_glyph(gibson_context_t *ctx, uint16_t x, uint16_t y, uint8_t *buf, size_t cap, size_t *len, char *err, size_t n) {
	int32_t st = gibson_get_frame_cell_glyph(ctx, x, y, buf, cap, len);
	if (st != GIBSON_OK) gibson_last_error_message(err, n);
	return st;
}
static int32_t gibson_go_get_output(gibson_context_t *ctx, uint8_t *buf, size_t cap, size_t *len, char *err, size_t n) {
	int32_t st = gibson_get_rendered_output(ctx, buf, cap, len);
	if (st != GIBSON_OK) gibson_last_error_message(err, n);
	return st;
}
static int32_t gibson_go_take_output(gibson_context_t *ctx, uint8_t *buf, size_t cap, size_t *len, char *err, size_t n) {
	int32_t st = gibson_take_rendered_output(ctx, buf, cap, len);
	if (st != GIBSON_OK) gibson_last_error_message(err, n);
	return st;
}
static int32_t gibson_go_clear_output(gibson_context_t *ctx, size_t *removed, char *err, size_t n) {
	int32_t st = gibson_clear_rendered_output(ctx, removed);
	if (st != GIBSON_OK) gibson_last_error_message(err, n);
	return st;
}
static int32_t gibson_go_node_text(const char *text, const gibson_style_t *style, int32_t wrap, gibson_node_t **out, char *err, size_t n) {
	int32_t st = gibson_node_text(text, style, wrap, out);
	if (st != GIBSON_OK) gibson_last_error_message(err, n);
	return st;
}
static int32_t gibson_go_set_root(gibson_context_t *ctx, gibson_node_t *node, char *err, size_t n) {
	int32_t st = gibson_set_root_node(ctx, node);
	if (st != GIBSON_OK) gibson_last_error_message(err, n);
	return st;
}
static uint8_t gibson_go_cell_truncated(const gibson_cell_t *c) { return c->_reserved[0]; }
*/
import "C"
import (
	"errors"
	"fmt"
	"strings"
	"unsafe"
)

const errCap = 512

// Status is a gibson_status_t.
type Status int32

const (
	StatusOK             Status = C.GIBSON_OK
	StatusInvalidParam   Status = C.GIBSON_ERR_INVALID_PARAM
	StatusIO             Status = C.GIBSON_ERR_IO
	StatusPanic          Status = C.GIBSON_ERR_PANIC
	StatusNotInitialized Status = C.GIBSON_ERR_NOT_INITIALIZED
	StatusQueueFull      Status = C.GIBSON_ERR_QUEUE_FULL
)

// Error is a non-OK status from the headless, event, readback and capture calls. LastError is the
// native library's last-error text, read in the same cgo call as the failing ABI call.
type Error struct {
	Op        string
	Status    Status
	LastError string
}

func (e *Error) Error() string {
	m := fmt.Sprintf("gibson: %s failed: status %d", e.Op, int32(e.Status))
	if e.LastError != "" {
		m += ": " + e.LastError
	}
	return m
}

// ErrQueueFull matches (errors.Is) the *Error of a push onto a full event queue.
var ErrQueueFull = errors.New("gibson: event queue is full")

// Is makes errors.Is(err, ErrQueueFull) true for a StatusQueueFull error.
func (e *Error) Is(target error) bool {
	return target == ErrQueueFull && e.Status == StatusQueueFull
}

func newError(op string, st C.int32_t, buf *[errCap]C.char) error {
	return &Error{Op: op, Status: Status(st), LastError: C.GoString(&buf[0])}
}

// EventType is a gibson_event_type_t.
type EventType int32

const (
	EventNone   EventType = C.GIBSON_EVENT_NONE
	EventKey    EventType = C.GIBSON_EVENT_KEY
	EventPaste  EventType = C.GIBSON_EVENT_PASTE
	EventResize EventType = C.GIBSON_EVENT_RESIZE
)

// KeyKind says how to read a KEY event's KeyCode. GIBSON_KEY_* occupy U+1001-U+100D, which are also
// real Unicode scalars (Myanmar letters); only the kind tells them apart.
type KeyKind uint32

const (
	KeyKindNone    KeyKind = C.GIBSON_KEY_KIND_NONE
	KeyKindChar    KeyKind = C.GIBSON_KEY_KIND_CHAR
	KeyKindSpecial KeyKind = C.GIBSON_KEY_KIND_SPECIAL
)

// Special keys (GIBSON_KEY_*), the KeyCode of a KeyKindSpecial event.
const (
	KeyEnter     uint32 = C.GIBSON_KEY_ENTER
	KeyLeft      uint32 = C.GIBSON_KEY_LEFT
	KeyRight     uint32 = C.GIBSON_KEY_RIGHT
	KeyUp        uint32 = C.GIBSON_KEY_UP
	KeyDown      uint32 = C.GIBSON_KEY_DOWN
	KeyHome      uint32 = C.GIBSON_KEY_HOME
	KeyEnd       uint32 = C.GIBSON_KEY_END
	KeyBackspace uint32 = C.GIBSON_KEY_BACKSPACE
	KeyDelete    uint32 = C.GIBSON_KEY_DELETE
	KeyEsc       uint32 = C.GIBSON_KEY_ESC
	KeyTab       uint32 = C.GIBSON_KEY_TAB
	KeyPageUp    uint32 = C.GIBSON_KEY_PAGE_UP
	KeyPageDown  uint32 = C.GIBSON_KEY_PAGE_DOWN
)

// Key modifier bits (GIBSON_MOD_*).
const (
	ModShift   uint8 = C.GIBSON_MOD_SHIFT
	ModControl uint8 = C.GIBSON_MOD_CONTROL
	ModAlt     uint8 = C.GIBSON_MOD_ALT
)

// EventQueueCapacity is the most undelivered events one headless context holds.
const EventQueueCapacity = C.GIBSON_EVENT_QUEUE_CAPACITY

// Event is one input event, as pushed to or polled from a headless context. Fields the type does
// not use are zero. KeyKind is meaningful only when HasKind: PollEventKind sets it (KeyKindNone for
// non-key events), PollEvent leaves HasKind false. PushEvent calls gibson_push_event_kind iff
// HasKind, else gibson_push_event (where U+1001-U+100D are always the special keys). Paste holds
// raw bytes and is read only when HasPaste. An Event returned by PollEventKind or PollEvent can be
// passed to PushEvent unchanged (an empty-queue EventNone cannot: the C ABI refuses it).
type Event struct {
	Type      EventType
	KeyCode   uint32
	Modifiers uint8
	Cols      uint16
	Rows      uint16
	HasPaste  bool
	Paste     string
	HasKind   bool
	KeyKind   KeyKind
}

// CharKey is a typed character (KeyKindChar); any Unicode scalar value, U+1001-U+100D included.
func CharKey(scalar uint32, mods uint8) Event {
	return Event{Type: EventKey, KeyCode: scalar, Modifiers: mods, HasKind: true, KeyKind: KeyKindChar}
}

// SpecialKey is a special key (KeyKindSpecial); key is a Key* constant.
func SpecialKey(key uint32, mods uint8) Event {
	return Event{Type: EventKey, KeyCode: key, Modifiers: mods, HasKind: true, KeyKind: KeyKindSpecial}
}

// LegacyKey is a key pushed with the kind-less gibson_push_event.
func LegacyKey(code uint32, mods uint8) Event {
	return Event{Type: EventKey, KeyCode: code, Modifiers: mods}
}

// PasteText is a paste event; text is raw bytes (the C ABI refuses invalid UTF-8).
func PasteText(text string) Event {
	return Event{Type: EventPaste, HasPaste: true, Paste: text}
}

// ResizeEvent is a resize event (both dimensions must be non-zero).
func ResizeEvent(cols, rows uint16) Event {
	return Event{Type: EventResize, Cols: cols, Rows: rows}
}

// Color types (gibson_color_type_t).
type ColorType int32

const (
	ColorReset         ColorType = C.GIBSON_COLOR_RESET
	ColorBlack         ColorType = C.GIBSON_COLOR_BLACK
	ColorRed           ColorType = C.GIBSON_COLOR_RED
	ColorGreen         ColorType = C.GIBSON_COLOR_GREEN
	ColorYellow        ColorType = C.GIBSON_COLOR_YELLOW
	ColorBlue          ColorType = C.GIBSON_COLOR_BLUE
	ColorMagenta       ColorType = C.GIBSON_COLOR_MAGENTA
	ColorCyan          ColorType = C.GIBSON_COLOR_CYAN
	ColorWhite         ColorType = C.GIBSON_COLOR_WHITE
	ColorBrightBlack   ColorType = C.GIBSON_COLOR_BRIGHT_BLACK
	ColorBrightRed     ColorType = C.GIBSON_COLOR_BRIGHT_RED
	ColorBrightGreen   ColorType = C.GIBSON_COLOR_BRIGHT_GREEN
	ColorBrightYellow  ColorType = C.GIBSON_COLOR_BRIGHT_YELLOW
	ColorBrightBlue    ColorType = C.GIBSON_COLOR_BRIGHT_BLUE
	ColorBrightMagenta ColorType = C.GIBSON_COLOR_BRIGHT_MAGENTA
	ColorBrightCyan    ColorType = C.GIBSON_COLOR_BRIGHT_CYAN
	ColorBrightWhite   ColorType = C.GIBSON_COLOR_BRIGHT_WHITE
	ColorANSI256       ColorType = C.GIBSON_COLOR_ANSI256
	ColorRGB           ColorType = C.GIBSON_COLOR_RGB
)

// WrapMode is a gibson_wrap_mode_t.
type WrapMode int32

const (
	WrapNone WrapMode = C.GIBSON_WRAP_NONE
	WrapChar WrapMode = C.GIBSON_WRAP_CHAR
	WrapWord WrapMode = C.GIBSON_WRAP_WORD
)

// Color mirrors gibson_color_t. For ColorANSI256, R holds the palette index.
type Color struct {
	Type    ColorType
	R, G, B uint8
}

// Style mirrors gibson_style_t without exposing a cgo type, so callers need no cgo.
type Style struct {
	Fg, Bg                                Color
	Bold, Dim, Italic, Underline, Reverse bool
}

func b2u(b bool) C.uint8_t {
	if b {
		return 1
	}
	return 0
}

func (s *Style) toC() C.gibson_style_t {
	var c C.gibson_style_t
	c.fg.color_type, c.fg.r, c.fg.g, c.fg.b = C.int32_t(s.Fg.Type), C.uint8_t(s.Fg.R), C.uint8_t(s.Fg.G), C.uint8_t(s.Fg.B)
	c.bg.color_type, c.bg.r, c.bg.g, c.bg.b = C.int32_t(s.Bg.Type), C.uint8_t(s.Bg.R), C.uint8_t(s.Bg.G), C.uint8_t(s.Bg.B)
	c.bold, c.dim, c.italic, c.underline, c.reverse = b2u(s.Bold), b2u(s.Dim), b2u(s.Italic), b2u(s.Underline), b2u(s.Reverse)
	return c
}

func styleFromC(c *C.gibson_style_t) Style {
	return Style{
		Fg:        Color{ColorType(c.fg.color_type), uint8(c.fg.r), uint8(c.fg.g), uint8(c.fg.b)},
		Bg:        Color{ColorType(c.bg.color_type), uint8(c.bg.r), uint8(c.bg.g), uint8(c.bg.b)},
		Bold:      c.bold != 0,
		Dim:       c.dim != 0,
		Italic:    c.italic != 0,
		Underline: c.underline != 0,
		Reverse:   c.reverse != 0,
	}
}

// Cell is a resolved frame cell. Glyph holds the inline UTF-8 bytes; when GlyphTruncated is set it
// is a codepoint-boundary prefix and Context.FullGlyph / FrameCellGlyph return the whole cluster.
type Cell struct {
	Glyph          []byte
	Width          uint8
	IsContinuation bool
	Transparent    bool
	GlyphTruncated bool
	Style          Style
}

func cellFromC(c *C.gibson_cell_t) Cell {
	return Cell{
		Glyph:          C.GoBytes(unsafe.Pointer(&c.glyph[0]), C.int(c.glyph_len)),
		Width:          uint8(c.width),
		IsContinuation: c.is_continuation != 0,
		Transparent:    c.transparent != 0,
		GlyphTruncated: C.gibson_go_cell_truncated(c) != 0,
		Style:          styleFromC(&c.style),
	}
}

// NewHeadlessContext creates a context that renders into an in-memory buffer with deterministic,
// TTY-independent semantics (fixed cols x rows, truecolor, no terminal lease). Events are injected
// with PushEvent; read the frame back with FrameSize / FrameCell(s) / RenderedOutput. cols and rows
// must be non-zero. Contexts are not safe for concurrent use.
func NewHeadlessContext(mode RenderMode, cols, rows uint16) (*Context, error) {
	if err := CheckABI(); err != nil {
		return nil, err
	}
	var ptr *C.gibson_context_t
	var buf [errCap]C.char
	st := C.gibson_go_headless(C.int32_t(mode), C.uint16_t(cols), C.uint16_t(rows), &ptr, &buf[0], errCap)
	if st != C.GIBSON_OK {
		return nil, newError("NewHeadlessContext", st, &buf)
	}
	return &Context{ptr: ptr}, nil
}

// PushEvent queues ev on a HEADLESS context. It returns an *Error (errors.Is(err, ErrQueueFull) for
// a full queue; StatusInvalidParam for a terminal context or an event the ABI refuses) carrying the
// last-error text. The only check made here is a NUL byte in Paste, which would cut the C string
// short; everything else is the C ABI's to judge.
func (c *Context) PushEvent(ev Event) error {
	var ce C.gibson_event_t
	ce.event_type = C.int32_t(ev.Type)
	ce.key_code = C.uint32_t(ev.KeyCode)
	ce.modifiers = C.uint8_t(ev.Modifiers)
	ce.cols = C.uint16_t(ev.Cols)
	ce.rows = C.uint16_t(ev.Rows)
	if ev.HasPaste {
		if strings.IndexByte(ev.Paste, 0) >= 0 {
			return errors.New("gibson: Event.Paste contains a NUL byte")
		}
		cs := C.CString(ev.Paste)
		defer C.free(unsafe.Pointer(cs))
		ce.paste_str = cs
	}
	var hasKind C.int
	if ev.HasKind {
		hasKind = 1
	}
	var buf [errCap]C.char
	st := C.gibson_go_push(c.ptr, &ce, hasKind, C.uint32_t(ev.KeyKind), &buf[0], errCap)
	if st != C.GIBSON_OK {
		return newError("PushEvent", st, &buf)
	}
	return nil
}

func eventFromC(ce *C.gibson_event_t, hasKind bool, kind C.uint32_t) Event {
	ev := Event{
		Type:      EventType(ce.event_type),
		KeyCode:   uint32(ce.key_code),
		Modifiers: uint8(ce.modifiers),
		Cols:      uint16(ce.cols),
		Rows:      uint16(ce.rows),
		HasKind:   hasKind,
		KeyKind:   KeyKind(kind),
	}
	// paste_str is valid only until the next poll: copy it out now.
	if ev.Type == EventPaste && ce.paste_str != nil {
		ev.HasPaste = true
		ev.Paste = C.GoString(ce.paste_str)
	}
	return ev
}

func (c *Context) poll(op string, timeoutMs uint32, withKind bool) (Event, error) {
	var ce C.gibson_event_t
	var kind C.uint32_t
	var buf [errCap]C.char
	var wk C.int
	if withKind {
		wk = 1
	}
	st := C.gibson_go_poll(c.ptr, C.uint32_t(timeoutMs), &ce, wk, &kind, &buf[0], errCap)
	if st != C.GIBSON_OK {
		return Event{}, newError(op, st, &buf)
	}
	return eventFromC(&ce, withKind, kind), nil
}

// PollEventKind returns the next event with its key kind (gibson_poll_event_kind): HasKind is true,
// so a typed U+1001 stays distinct from KeyEnter. On a headless context it never waits and returns
// an EventNone on an empty queue; a polled resize has already set the headless size.
func (c *Context) PollEventKind(timeoutMs uint32) (Event, error) {
	return c.poll("PollEventKind", timeoutMs, true)
}

// PollEvent is the kind-less poll (gibson_poll_event): a typed U+1001 and KeyEnter look the same
// and HasKind is false.
func (c *Context) PollEvent(timeoutMs uint32) (Event, error) {
	return c.poll("PollEvent", timeoutMs, false)
}

// FrameSize returns (cols, rows) of the most recently rendered frame, (0, 0) if there is none yet.
func (c *Context) FrameSize() (cols, rows uint16, err error) {
	var cc, rr C.uint16_t
	var buf [errCap]C.char
	if st := C.gibson_go_frame_size(c.ptr, &cc, &rr, &buf[0], errCap); st != C.GIBSON_OK {
		return 0, 0, newError("FrameSize", st, &buf)
	}
	return uint16(cc), uint16(rr), nil
}

// FrameCell returns the resolved styled cell at (x, y); an *Error if there is no frame or (x, y) is
// out of bounds.
func (c *Context) FrameCell(x, y uint16) (Cell, error) {
	var cell C.gibson_cell_t
	var buf [errCap]C.char
	if st := C.gibson_go_frame_cell(c.ptr, C.uint16_t(x), C.uint16_t(y), &cell, &buf[0], errCap); st != C.GIBSON_OK {
		return Cell{}, newError("FrameCell", st, &buf)
	}
	return cellFromC(&cell), nil
}

// FrameCells reads cells row-major from (x, y) to the end of the frame, at most maxCells (a negative
// maxCells means no limit). The C ABI judges every call, including one that returns no cells, so a
// missing frame or out-of-bounds (x, y) is an *Error even when maxCells is 0.
func (c *Context) FrameCells(x, y uint16, maxCells int) ([]Cell, error) {
	cols, rows, err := c.FrameSize()
	if err != nil {
		return nil, err
	}
	total := int(cols) * int(rows)
	start := int(y)*int(cols) + int(x)
	avail := 0
	if total > start {
		avail = total - start
	}
	want := avail
	if maxCells >= 0 && maxCells < avail {
		want = maxCells
	}
	var buf [errCap]C.char
	var written C.size_t
	if want == 0 {
		var probe C.gibson_cell_t
		if st := C.gibson_go_frame_cells(c.ptr, C.uint16_t(x), C.uint16_t(y), &probe, 0, &written, &buf[0], errCap); st != C.GIBSON_OK {
			return nil, newError("FrameCells", st, &buf)
		}
		return []Cell{}, nil
	}
	raw := make([]C.gibson_cell_t, want)
	if st := C.gibson_go_frame_cells(c.ptr, C.uint16_t(x), C.uint16_t(y), &raw[0], C.size_t(want), &written, &buf[0], errCap); st != C.GIBSON_OK {
		return nil, newError("FrameCells", st, &buf)
	}
	out := make([]Cell, int(written))
	for i := range out {
		out[i] = cellFromC(&raw[i])
	}
	return out, nil
}

// FrameCellGlyph returns the full (untruncated) grapheme-cluster UTF-8 bytes at (x, y).
func (c *Context) FrameCellGlyph(x, y uint16) ([]byte, error) {
	var buf [errCap]C.char
	var n C.size_t
	if st := C.gibson_go_cell_glyph(c.ptr, C.uint16_t(x), C.uint16_t(y), nil, 0, &n, &buf[0], errCap); st != C.GIBSON_OK {
		return nil, newError("FrameCellGlyph", st, &buf)
	}
	if n == 0 {
		return []byte{}, nil
	}
	g := make([]byte, int(n))
	var got C.size_t
	if st := C.gibson_go_cell_glyph(c.ptr, C.uint16_t(x), C.uint16_t(y), (*C.uint8_t)(unsafe.Pointer(&g[0])), n, &got, &buf[0], errCap); st != C.GIBSON_OK {
		return nil, newError("FrameCellGlyph", st, &buf)
	}
	return g[:int(got)], nil
}

// FullGlyph returns cell's complete glyph bytes: the inline bytes, or FrameCellGlyph(x, y) if they
// were truncated.
func (c *Context) FullGlyph(cell Cell, x, y uint16) ([]byte, error) {
	if cell.GlyphTruncated {
		return c.FrameCellGlyph(x, y)
	}
	return cell.Glyph, nil
}

// drainer is a size-then-copy call on the capture.
func (c *Context) capture(op string, take bool) ([]byte, error) {
	for {
		var buf [errCap]C.char
		var need C.size_t
		call := func(p *C.uint8_t, capacity C.size_t, out *C.size_t) C.int32_t {
			if take {
				return C.gibson_go_take_output(c.ptr, p, capacity, out, &buf[0], errCap)
			}
			return C.gibson_go_get_output(c.ptr, p, capacity, out, &buf[0], errCap)
		}
		if st := call(nil, 0, &need); st != C.GIBSON_OK {
			return nil, newError(op, st, &buf)
		}
		if need == 0 {
			return []byte{}, nil
		}
		data := make([]byte, int(need))
		var avail C.size_t
		if st := call((*C.uint8_t)(unsafe.Pointer(&data[0])), need, &avail); st != C.GIBSON_OK {
			return nil, newError(op, st, &buf)
		}
		if avail > need {
			continue // grew between the two calls; nothing was copied or removed
		}
		return data[:int(avail)], nil
	}
}

// RenderedOutput returns the ANSI frame bytes captured since creation or the last take/clear. It
// copies; it never drains.
func (c *Context) RenderedOutput() ([]byte, error) { return c.capture("RenderedOutput", false) }

// TakeRenderedOutput drains the capture: it returns its bytes and empties it (all-or-nothing, so a
// result never splits an escape sequence).
func (c *Context) TakeRenderedOutput() ([]byte, error) { return c.capture("TakeRenderedOutput", true) }

// ClearRenderedOutput discards the capture and returns the number of bytes removed.
func (c *Context) ClearRenderedOutput() (int, error) {
	var buf [errCap]C.char
	var n C.size_t
	if st := C.gibson_go_clear_output(c.ptr, &n, &buf[0], errCap); st != C.GIBSON_OK {
		return 0, newError("ClearRenderedOutput", st, &buf)
	}
	return int(n), nil
}

// SetRootNode assigns the root node tree (taking ownership, as SetRoot does) and reports the status.
// On a closed (or nil) Context it returns an error and leaves the node with the caller, who can
// still Free it: the native call would refuse the context before taking the node.
func (c *Context) SetRootNode(n *Node) error {
	if n == nil || n.ptr == nil {
		return errors.New("invalid node")
	}
	if c == nil || c.ptr == nil {
		return &Error{Op: "SetRootNode", Status: StatusInvalidParam, LastError: "closed context"}
	}
	ptr := n.ptr
	n.ptr = nil
	var buf [errCap]C.char
	if st := C.gibson_go_set_root(c.ptr, ptr, &buf[0], errCap); st != C.GIBSON_OK {
		return newError("SetRootNode", st, &buf)
	}
	return nil
}

// UnsafeHandle returns the native gibson_context_t* (nil after Close), for cgo code that must make
// an ABI call this package does not wrap. The Context still owns it: do not destroy it.
func (c *Context) UnsafeHandle() unsafe.Pointer { return unsafe.Pointer(c.ptr) }

// NewStyledTextNode creates a text node with a style (nil for the default) and a wrap mode, and
// reports the status and last-error text. text is passed as a C string, so it must not contain NUL.
func NewStyledTextNode(text string, style *Style, wrap WrapMode) (*Node, error) {
	if strings.IndexByte(text, 0) >= 0 {
		return nil, errors.New("gibson: text contains a NUL byte")
	}
	cs := C.CString(text)
	defer C.free(unsafe.Pointer(cs))
	var cstyle C.gibson_style_t
	var sp *C.gibson_style_t
	if style != nil {
		cstyle = style.toC()
		sp = &cstyle
	}
	var ptr *C.gibson_node_t
	var buf [errCap]C.char
	if st := C.gibson_go_node_text(cs, sp, C.int32_t(wrap), &ptr, &buf[0], errCap); st != C.GIBSON_OK {
		return nil, newError("NewStyledTextNode", st, &buf)
	}
	return &Node{ptr: ptr}, nil
}

// Valid reports whether the node still holds a native node (false after it was attached, mounted
// or freed, or if creation failed).
func (n *Node) Valid() bool { return n != nil && n.ptr != nil }

// AddStyledSpan appends a span with a Go-side Style (nil for the default). The existing AddSpan
// keeps its cgo-typed signature.
func (l *Line) AddStyledSpan(text string, style *Style) *Line {
	if l.ptr == nil {
		return l
	}
	cs := C.CString(text)
	defer C.free(unsafe.Pointer(cs))
	if style == nil {
		C.gibson_line_add_span(l.ptr, cs, nil)
		return l
	}
	cstyle := style.toC()
	C.gibson_line_add_span(l.ptr, cs, &cstyle)
	return l
}

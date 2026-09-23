// UNVERIFIED: Go compiler not available in current test environment
package gibson

/*
#cgo CFLAGS: -I../../../include
#cgo LDFLAGS: -L../../../target/release -lgibson
#include "gibson.h"
#include <stdlib.h>
*/
import "C"
import (
	"errors"
	"unsafe"
)

// RenderMode defines inline vs fullscreen mode.
type RenderMode int

const (
	ModeInline     RenderMode = C.GIBSON_MODE_INLINE
	ModeFullscreen RenderMode = C.GIBSON_MODE_FULLSCREEN
)

// BorderType defines border styling.
type BorderType int

const (
	BorderSingle  BorderType = C.GIBSON_BORDER_SINGLE
	BorderDouble  BorderType = C.GIBSON_BORDER_DOUBLE
	BorderRounded BorderType = C.GIBSON_BORDER_ROUNDED
	BorderThick   BorderType = C.GIBSON_BORDER_THICK
	BorderAscii   BorderType = C.GIBSON_BORDER_ASCII
)

// Stats holds render performance metrics (mirrors the versioned C struct).
type Stats struct {
	StructSize               uint32
	AbiVersion               uint32
	Frames                   uint64
	SkippedFrames            uint64
	DirtyCells               uint64
	TotalCells               uint64
	FrameBytes               uint64
	FullRepaints             uint64
	LastRenderDurationMicros uint64
	HistoryInsertions        uint64
	InsertionRepaints        uint64
	FastInsertions           uint64
	InsertionBytes           uint64
	AnchorResyncs            uint64
	CommitBytes              uint64
	ControlBytes             uint64
}

// AbiVersion returns the runtime ABI version of the linked library.
func AbiVersion() uint32 {
	return uint32(C.gibson_abi_version())
}

// Context wraps a LibGibson native terminal context.
type Context struct {
	ptr *C.gibson_context_t
}

// NewContext creates a new LibGibson context.
func NewContext(mode RenderMode) (*Context, error) {
	var ptr *C.gibson_context_t
	status := C.gibson_create_context(C.int32_t(mode), &ptr)
	if status != C.GIBSON_OK {
		var buf [256]C.char
		C.gibson_last_error_message(&buf[0], 256)
		return nil, errors.New(C.GoString(&buf[0]))
	}
	return &Context{ptr: ptr}, nil
}

// Close destroys the context and restores the terminal.
func (c *Context) Close() {
	if c.ptr != nil {
		C.gibson_destroy_context(c.ptr)
		c.ptr = nil
	}
}

// SetSyncUpdates enables or disables atomic terminal updates (DECSM 2026).
func (c *Context) SetSyncUpdates(enabled bool) error {
	var en C.int32_t
	if enabled {
		en = 1
	}
	status := C.gibson_set_sync_updates(c.ptr, en)
	if status != C.GIBSON_OK {
		return errors.New("failed to set sync updates")
	}
	return nil
}

// SetMaxFps configures the maximum frame rate limit for frame budget throttling.
func (c *Context) SetMaxFps(fps uint32) error {
	status := C.gibson_set_max_fps(c.ptr, C.uint32_t(fps))
	if status != C.GIBSON_OK {
		return errors.New("failed to set max fps")
	}
	return nil
}

// Commit writes finalized text to immutable terminal scrollback and clears active live region.
func (c *Context) Commit(text string) error {
	cStr := C.CString(text)
	defer C.free(unsafe.Pointer(cStr))

	status := C.gibson_commit(c.ptr, cStr)
	if status != C.GIBSON_OK {
		return errors.New("failed to commit text to scrollback")
	}
	return nil
}

// InsertTextBeforeLive inserts safe, width-aware plain text into scrollback
// ABOVE the active live region, preserving live state. Terminal control
// characters are neutralized by the engine.
func (c *Context) InsertTextBeforeLive(text string) error {
	cStr := C.CString(text)
	defer C.free(unsafe.Pointer(cStr))

	status := C.gibson_insert_text_before_live(c.ptr, cStr)
	if status != C.GIBSON_OK {
		return errors.New("failed to insert text before live region")
	}
	return nil
}

// InsertRawLinesBeforeLiveUnchecked inserts raw text into scrollback ABOVE the
// active live region. The text is treated as a terminal byte stream and is NOT
// sanitized: embedded escape/OSC/CSI sequences reach the terminal. Prefer
// InsertTextBeforeLive for untrusted text.
func (c *Context) InsertRawLinesBeforeLiveUnchecked(text string) error {
	cStr := C.CString(text)
	defer C.free(unsafe.Pointer(cStr))

	status := C.gibson_insert_raw_lines_before_live_unchecked(c.ptr, cStr)
	if status != C.GIBSON_OK {
		return errors.New("failed to insert raw lines before live region")
	}
	return nil
}

// InsertBeforeLive is retained for compatibility and aliases the SAFE text path
// (InsertTextBeforeLive). It never uses the unchecked raw path.
//
// Deprecated: use InsertTextBeforeLive (or InsertRawLinesBeforeLiveUnchecked).
func (c *Context) InsertBeforeLive(text string) error {
	return c.InsertTextBeforeLive(text)
}

// CommitNode renders a UI node into scrollback and clears active live region.
func (c *Context) CommitNode(n *Node) error {
	if n == nil || n.ptr == nil {
		return errors.New("invalid node")
	}
	ptr := n.ptr
	n.ptr = nil
	status := C.gibson_commit_node(c.ptr, ptr)
	if status != C.GIBSON_OK {
		return errors.New("failed to commit node to scrollback")
	}
	return nil
}

// InsertNodeBeforeLive renders a UI node into scrollback ABOVE the active live region.
func (c *Context) InsertNodeBeforeLive(n *Node) error {
	if n == nil || n.ptr == nil {
		return errors.New("invalid node")
	}
	ptr := n.ptr
	n.ptr = nil
	status := C.gibson_insert_node_before_live(c.ptr, ptr)
	if status != C.GIBSON_OK {
		return errors.New("failed to insert node before live region")
	}
	return nil
}

// ClearLiveRegion erases the active live region from the terminal.
func (c *Context) ClearLiveRegion() error {
	status := C.gibson_clear_live_region(c.ptr)
	if status != C.GIBSON_OK {
		return errors.New("failed to clear live region")
	}
	return nil
}

// Render executes differential rendering on the current root node immediately.
func (c *Context) Render() error {
	status := C.gibson_render(c.ptr)
	if status != C.GIBSON_OK {
		return errors.New("render pass failed")
	}
	return nil
}

// RenderIfDue renders only if the frame budget interval has elapsed.
func (c *Context) RenderIfDue() (bool, error) {
	var rendered C.int32_t
	status := C.gibson_render_if_due(c.ptr, &rendered)
	if status != C.GIBSON_OK {
		return false, errors.New("render_if_due pass failed")
	}
	return rendered != 0, nil
}

// RequestRender flags the frame scheduler that a repaint is needed.
func (c *Context) RequestRender() error {
	status := C.gibson_request_render(c.ptr)
	if status != C.GIBSON_OK {
		return errors.New("request_render failed")
	}
	return nil
}

// SetRoot assigns the root UI node tree to the context (takes ownership).
func (c *Context) SetRoot(n *Node) {
	if n != nil && n.ptr != nil {
		C.gibson_set_root_node(c.ptr, n.ptr)
		n.ptr = nil // Ownership transferred to context
	}
}

// GetStats returns current frame and rendering metrics.
func (c *Context) GetStats() (*Stats, error) {
	var s C.gibson_stats_t
	C.gibson_stats_init(&s)
	status := C.gibson_get_stats(c.ptr, &s)
	if status != C.GIBSON_OK {
		return nil, errors.New("failed to get stats")
	}
	return &Stats{
		StructSize:               uint32(s.struct_size),
		AbiVersion:               uint32(s.abi_version),
		Frames:                   uint64(s.frames),
		SkippedFrames:            uint64(s.skipped_frames),
		DirtyCells:               uint64(s.dirty_cells),
		TotalCells:               uint64(s.total_cells),
		FrameBytes:               uint64(s.frame_bytes),
		FullRepaints:             uint64(s.full_repaints),
		LastRenderDurationMicros: uint64(s.last_render_duration_micros),
		HistoryInsertions:        uint64(s.history_insertions),
		InsertionRepaints:        uint64(s.insertion_repaints),
		FastInsertions:           uint64(s.fast_insertions),
		InsertionBytes:           uint64(s.insertion_bytes),
		AnchorResyncs:            uint64(s.anchor_resyncs),
		CommitBytes:              uint64(s.commit_bytes),
		ControlBytes:             uint64(s.control_bytes),
	}, nil
}

// Node represents a declarative UI element.
type Node struct {
	ptr *C.gibson_node_t
}

// NewColNode creates a flex column box.
func NewColNode() *Node {
	var ptr *C.gibson_node_t
	C.gibson_node_box_col(&ptr)
	return &Node{ptr: ptr}
}

// NewRowNode creates a flex row box.
func NewRowNode() *Node {
	var ptr *C.gibson_node_t
	C.gibson_node_box_row(&ptr)
	return &Node{ptr: ptr}
}

// NewStackNode creates an overlay container (children composited in order).
func NewStackNode() *Node {
	var ptr *C.gibson_node_t
	C.gibson_node_stack(&ptr)
	return &Node{ptr: ptr}
}

// NewDimNode creates a dim-veil overlay node.
func NewDimNode() *Node {
	var ptr *C.gibson_node_t
	C.gibson_node_dim(&ptr)
	return &Node{ptr: ptr}
}

// NewTextNode creates a text element with default styling.
func NewTextNode(text string) *Node {
	cStr := C.CString(text)
	defer C.free(unsafe.Pointer(cStr))

	var ptr *C.gibson_node_t
	C.gibson_node_text(cStr, nil, C.GIBSON_WRAP_WORD, &ptr)
	return &Node{ptr: ptr}
}

// NewBorderBoxNode creates a bordered container.
func NewBorderBoxNode(border BorderType) *Node {
	var ptr *C.gibson_node_t
	C.gibson_node_border_box(C.int32_t(border), nil, &ptr)
	return &Node{ptr: ptr}
}

// NewRuleNode creates a horizontal divider with an optional title.
func NewRuleNode(title string) *Node {
	var cTitle *C.char
	if title != "" {
		cTitle = C.CString(title)
		defer C.free(unsafe.Pointer(cTitle))
	}
	var ptr *C.gibson_node_t
	C.gibson_node_rule(cTitle, nil, &ptr)
	return &Node{ptr: ptr}
}

// NewRailNode creates a left-border callout rail container.
func NewRailNode() *Node {
	var ptr *C.gibson_node_t
	C.gibson_node_rail(nil, &ptr)
	return &Node{ptr: ptr}
}

// NewSpinnerNode creates an animated spinner element.
func NewSpinnerNode(frameIndex uint32, label string) *Node {
	var cLabel *C.char
	if label != "" {
		cLabel = C.CString(label)
		defer C.free(unsafe.Pointer(cLabel))
	}
	var ptr *C.gibson_node_t
	C.gibson_node_spinner(C.uint32_t(frameIndex), nil, cLabel, &ptr)
	return &Node{ptr: ptr}
}

// NewTextInputNode creates a text input element.
func NewTextInputNode(value string, cursorGrapheme uint32, placeholder string) *Node {
	cValue := C.CString(value)
	defer C.free(unsafe.Pointer(cValue))

	var cPlaceholder *C.char
	if placeholder != "" {
		cPlaceholder = C.CString(placeholder)
		defer C.free(unsafe.Pointer(cPlaceholder))
	}
	var ptr *C.gibson_node_t
	C.gibson_node_text_input(cValue, C.uint32_t(cursorGrapheme), cPlaceholder, nil, &ptr)
	return &Node{ptr: ptr}
}

// SetWidth sets fixed node width.
func (n *Node) SetWidth(w float32) *Node {
	if n.ptr != nil {
		C.gibson_node_set_width(n.ptr, C.float(w))
	}
	return n
}

// SetHeight sets fixed node height.
func (n *Node) SetHeight(h float32) *Node {
	if n.ptr != nil {
		C.gibson_node_set_height(n.ptr, C.float(h))
	}
	return n
}

// SetPercentWidth sets width as a percentage of parent available width.
func (n *Node) SetPercentWidth(pw float32) *Node {
	if n.ptr != nil {
		C.gibson_node_set_percent_width(n.ptr, C.float(pw))
	}
	return n
}

// SetMinWidth sets minimum node width constraint.
func (n *Node) SetMinWidth(mw float32) *Node {
	if n.ptr != nil {
		C.gibson_node_set_min_width(n.ptr, C.float(mw))
	}
	return n
}

// SetMaxWidth sets maximum node width constraint.
func (n *Node) SetMaxWidth(mw float32) *Node {
	if n.ptr != nil {
		C.gibson_node_set_max_width(n.ptr, C.float(mw))
	}
	return n
}

// SetFlexGrow sets flex grow factor.
func (n *Node) SetFlexGrow(grow float32) *Node {
	if n.ptr != nil {
		C.gibson_node_set_flex_grow(n.ptr, C.float(grow))
	}
	return n
}

// SetGap sets flex gap.
func (n *Node) SetGap(g float32) *Node {
	if n.ptr != nil {
		C.gibson_node_set_gap(n.ptr, C.float(g))
	}
	return n
}

// SetPadding sets uniform padding.
func (n *Node) SetPadding(p float32) *Node {
	if n.ptr != nil {
		C.gibson_node_set_padding(n.ptr, C.float(p))
	}
	return n
}

// SetPaddingSides sets per-side padding.
func (n *Node) SetPaddingSides(left, right, top, bottom float32) *Node {
	if n.ptr != nil {
		C.gibson_node_set_padding_sides(n.ptr, C.float(left), C.float(right), C.float(top), C.float(bottom))
	}
	return n
}

// AddChild attaches a child node, transferring ownership.
func (n *Node) AddChild(child *Node) *Node {
	if n.ptr != nil && child != nil && child.ptr != nil {
		C.gibson_node_add_child(n.ptr, child.ptr)
		child.ptr = nil
	}
	return n
}

// Free releases unattached node memory.
func (n *Node) Free() {
	if n.ptr != nil {
		C.gibson_node_free(n.ptr)
		n.ptr = nil
	}
}

// CommitText writes structured plain text to scrollback (control chars neutralized).
func (c *Context) CommitText(text string) error {
	cStr := C.CString(text)
	defer C.free(unsafe.Pointer(cStr))
	if C.gibson_commit_text(c.ptr, cStr) != C.GIBSON_OK {
		return errors.New("failed to commit text")
	}
	return nil
}

// CommitRawAnsiUnchecked is the raw escape hatch; the payload is written verbatim.
func (c *Context) CommitRawAnsiUnchecked(text string) error {
	cStr := C.CString(text)
	defer C.free(unsafe.Pointer(cStr))
	if C.gibson_commit_raw_ansi_unchecked(c.ptr, cStr) != C.GIBSON_OK {
		return errors.New("failed to commit raw ansi")
	}
	return nil
}

// CommitRichText commits structured rich text (borrows rich).
func (c *Context) CommitRichText(rich *RichText) error {
	if rich == nil || rich.ptr == nil {
		return errors.New("invalid rich text")
	}
	if C.gibson_commit_rich_text(c.ptr, rich.ptr) != C.GIBSON_OK {
		return errors.New("failed to commit rich text")
	}
	return nil
}

// InsertRichTextBeforeLive inserts structured rich text above the live region.
func (c *Context) InsertRichTextBeforeLive(rich *RichText) error {
	if rich == nil || rich.ptr == nil {
		return errors.New("invalid rich text")
	}
	if C.gibson_insert_rich_text_before_live(c.ptr, rich.ptr) != C.GIBSON_OK {
		return errors.New("failed to insert rich text")
	}
	return nil
}

// Line is a language-neutral line of styled spans.
type Line struct {
	ptr *C.gibson_line_t
}

// NewLine creates an empty line.
func NewLine() *Line {
	var ptr *C.gibson_line_t
	C.gibson_line_new(&ptr)
	return &Line{ptr: ptr}
}

// AddSpan appends a styled span. style may be nil.
func (l *Line) AddSpan(text string, style *C.gibson_style_t) *Line {
	if l.ptr == nil {
		return l
	}
	cStr := C.CString(text)
	defer C.free(unsafe.Pointer(cStr))
	C.gibson_line_add_span(l.ptr, cStr, style)
	return l
}

// SetAlign sets line alignment (0 left, 1 center, 2 right).
func (l *Line) SetAlign(align int32) *Line {
	if l.ptr != nil {
		C.gibson_line_set_align(l.ptr, C.int32_t(align))
	}
	return l
}

// Free releases the line.
func (l *Line) Free() {
	if l.ptr != nil {
		C.gibson_line_free(l.ptr)
		l.ptr = nil
	}
}

// RichText is a structured multi-line rich text block.
type RichText struct {
	ptr *C.gibson_rich_text_t
}

// NewRichText creates an empty rich text block.
func NewRichText() *RichText {
	var ptr *C.gibson_rich_text_t
	C.gibson_rich_text_new(&ptr)
	return &RichText{ptr: ptr}
}

// AddLine copies line into the rich text block.
func (r *RichText) AddLine(line *Line) *RichText {
	if r.ptr != nil && line != nil && line.ptr != nil {
		C.gibson_rich_text_add_line(r.ptr, line.ptr)
	}
	return r
}

// ToNode builds a UI node from the rich text (borrows rich).
func (r *RichText) ToNode(wrap int32) *Node {
	var ptr *C.gibson_node_t
	C.gibson_node_rich_text(r.ptr, C.int32_t(wrap), &ptr)
	return &Node{ptr: ptr}
}

// Free releases the rich text.
func (r *RichText) Free() {
	if r.ptr != nil {
		C.gibson_rich_text_free(r.ptr)
		r.ptr = nil
	}
}

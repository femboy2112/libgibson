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

// Commit writes finalized text to immutable terminal scrollback.
func (c *Context) Commit(text string) error {
	cStr := C.CString(text)
	defer C.free(unsafe.Pointer(cStr))

	status := C.gibson_commit(c.ptr, cStr)
	if status != C.GIBSON_OK {
		return errors.New("failed to commit text to scrollback")
	}
	return nil
}

// Render executes differential rendering on the current root node.
func (c *Context) Render() error {
	status := C.gibson_render(c.ptr)
	if status != C.GIBSON_OK {
		return errors.New("render pass failed")
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

// NewTextNode creates a text element.
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

// SetGap sets flex gap.
func (n *Node) SetGap(g float32) *Node {
	if n.ptr != nil {
		C.gibson_node_set_gap(n.ptr, C.float(g))
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

"""
LibGibson Python Wrapper using ctypes.
Provides a clean, idiomatic interface to the LibGibson differential terminal UI engine.
"""

import ctypes
import os
import sys

# Locate libgibson.so
def _find_library():
    repo_dir = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    candidates = [
        os.path.join(repo_dir, "target", "release", "libgibson.so"),
        os.path.join(repo_dir, "target", "debug", "libgibson.so"),
        "libgibson.so",
    ]
    for path in candidates:
        if os.path.exists(path):
            return path
    return "libgibson.so"

_lib = ctypes.CDLL(_find_library())

# Enums
class RenderMode:
    INLINE = 0
    FULLSCREEN = 1

class ColorType:
    RESET = 0
    BLACK = 1
    RED = 2
    GREEN = 3
    YELLOW = 4
    BLUE = 5
    MAGENTA = 6
    CYAN = 7
    WHITE = 8
    BRIGHT_BLACK = 9
    BRIGHT_RED = 10
    BRIGHT_GREEN = 11
    BRIGHT_YELLOW = 12
    BRIGHT_BLUE = 13
    BRIGHT_MAGENTA = 14
    BRIGHT_CYAN = 15
    BRIGHT_WHITE = 16
    ANSI256 = 17
    RGB = 18

class BorderType:
    SINGLE = 0
    DOUBLE = 1
    ROUNDED = 2
    THICK = 3
    ASCII = 4

class WrapMode:
    NONE = 0
    CHAR = 1
    WORD = 2

# Structs
class GibsonColor(ctypes.Structure):
    _fields_ = [
        ("color_type", ctypes.c_int32),
        ("r", ctypes.c_uint8),
        ("g", ctypes.c_uint8),
        ("b", ctypes.c_uint8),
        ("_pad", ctypes.c_uint8),
    ]

    @classmethod
    def named(cls, color_type):
        return cls(color_type=color_type, r=0, g=0, b=0, _pad=0)

    @classmethod
    def rgb(cls, r, g, b):
        return cls(color_type=ColorType.RGB, r=r, g=g, b=b, _pad=0)

class GibsonStyle(ctypes.Structure):
    _fields_ = [
        ("fg", GibsonColor),
        ("bg", GibsonColor),
        ("bold", ctypes.c_uint8),
        ("dim", ctypes.c_uint8),
        ("italic", ctypes.c_uint8),
        ("underline", ctypes.c_uint8),
        ("reverse", ctypes.c_uint8),
        ("_reserved", ctypes.c_uint8 * 3),
    ]

    @classmethod
    def make(cls, fg=None, bg=None, bold=False, dim=False, italic=False, underline=False, reverse=False):
        st = cls()
        st.fg = fg if fg is not None else GibsonColor.named(ColorType.RESET)
        st.bg = bg if bg is not None else GibsonColor.named(ColorType.RESET)
        st.bold = 1 if bold else 0
        st.dim = 1 if dim else 0
        st.italic = 1 if italic else 0
        st.underline = 1 if underline else 0
        st.reverse = 1 if reverse else 0
        return st

class GibsonStats(ctypes.Structure):
    _fields_ = [
        ("frames", ctypes.c_uint64),
        ("skipped_frames", ctypes.c_uint64),
        ("dirty_cells", ctypes.c_uint64),
        ("total_cells", ctypes.c_uint64),
        ("bytes_emitted", ctypes.c_uint64),
        ("full_repaints", ctypes.c_uint64),
        ("last_render_duration_micros", ctypes.c_uint64),
    ]

# Setup ctypes signatures
_lib.gibson_create_context.argtypes = [ctypes.c_int32, ctypes.POINTER(ctypes.c_void_p)]
_lib.gibson_create_context.restype = ctypes.c_int32

_lib.gibson_destroy_context.argtypes = [ctypes.c_void_p]
_lib.gibson_destroy_context.restype = None

_lib.gibson_set_root_node.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
_lib.gibson_set_root_node.restype = ctypes.c_int32

_lib.gibson_render.argtypes = [ctypes.c_void_p]
_lib.gibson_render.restype = ctypes.c_int32

_lib.gibson_commit.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
_lib.gibson_commit.restype = ctypes.c_int32

_lib.gibson_get_stats.argtypes = [ctypes.c_void_p, ctypes.POINTER(GibsonStats)]
_lib.gibson_get_stats.restype = ctypes.c_int32

_lib.gibson_node_box_col.argtypes = [ctypes.POINTER(ctypes.c_void_p)]
_lib.gibson_node_box_col.restype = ctypes.c_int32

_lib.gibson_node_box_row.argtypes = [ctypes.POINTER(ctypes.c_void_p)]
_lib.gibson_node_box_row.restype = ctypes.c_int32

_lib.gibson_node_text.argtypes = [ctypes.c_char_p, ctypes.POINTER(GibsonStyle), ctypes.c_int32, ctypes.POINTER(ctypes.c_void_p)]
_lib.gibson_node_text.restype = ctypes.c_int32

_lib.gibson_node_spinner.argtypes = [ctypes.c_uint32, ctypes.POINTER(GibsonStyle), ctypes.c_char_p, ctypes.POINTER(ctypes.c_void_p)]
_lib.gibson_node_spinner.restype = ctypes.c_int32

_lib.gibson_node_border_box.argtypes = [ctypes.c_int32, ctypes.POINTER(GibsonStyle), ctypes.POINTER(ctypes.c_void_p)]
_lib.gibson_node_border_box.restype = ctypes.c_int32

_lib.gibson_node_add_child.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
_lib.gibson_node_add_child.restype = ctypes.c_int32

_lib.gibson_node_set_width.argtypes = [ctypes.c_void_p, ctypes.c_float]
_lib.gibson_node_set_width.restype = ctypes.c_int32

_lib.gibson_node_set_height.argtypes = [ctypes.c_void_p, ctypes.c_float]
_lib.gibson_node_set_height.restype = ctypes.c_int32

_lib.gibson_node_set_gap.argtypes = [ctypes.c_void_p, ctypes.c_float]
_lib.gibson_node_set_gap.restype = ctypes.c_int32

_lib.gibson_node_set_padding.argtypes = [ctypes.c_void_p, ctypes.c_float]
_lib.gibson_node_set_padding.restype = ctypes.c_int32

_lib.gibson_node_free.argtypes = [ctypes.c_void_p]
_lib.gibson_node_free.restype = None

_lib.gibson_last_error_message.argtypes = [ctypes.c_char_p, ctypes.c_size_t]
_lib.gibson_last_error_message.restype = ctypes.c_int32


class Node:
    def __init__(self, handle):
        self.handle = handle

    def __del__(self):
        if self.handle:
            _lib.gibson_node_free(self.handle)
            self.handle = None

    def release(self):
        h = self.handle
        self.handle = None
        return h

    @classmethod
    def col(cls):
        h = ctypes.c_void_p()
        _lib.gibson_node_box_col(ctypes.byref(h))
        return cls(h)

    @classmethod
    def row(cls):
        h = ctypes.c_void_p()
        _lib.gibson_node_box_row(ctypes.byref(h))
        return cls(h)

    @classmethod
    def text(cls, content, style=None, wrap=WrapMode.WORD):
        h = ctypes.c_void_p()
        style_ref = ctypes.byref(style) if style is not None else None
        _lib.gibson_node_text(content.encode("utf-8"), style_ref, wrap, ctypes.byref(h))
        return cls(h)

    @classmethod
    def border_box(cls, border_type=BorderType.ROUNDED, style=None):
        h = ctypes.c_void_p()
        style_ref = ctypes.byref(style) if style is not None else None
        _lib.gibson_node_border_box(border_type, style_ref, ctypes.byref(h))
        return cls(h)

    def width(self, w):
        if self.handle:
            _lib.gibson_node_set_width(self.handle, float(w))
        return self

    def height(self, h):
        if self.handle:
            _lib.gibson_node_set_height(self.handle, float(h))
        return self

    def gap(self, g):
        if self.handle:
            _lib.gibson_node_set_gap(self.handle, float(g))
        return self

    def padding(self, p):
        if self.handle:
            _lib.gibson_node_set_padding(self.handle, float(p))
        return self

    def add_child(self, child):
        if self.handle and child.handle:
            _lib.gibson_node_add_child(self.handle, child.release())
        return self


class Context:
    def __init__(self, mode=RenderMode.INLINE):
        self.handle = ctypes.c_void_p()
        status = _lib.gibson_create_context(mode, ctypes.byref(self.handle))
        if status != 0:
            err_buf = ctypes.create_string_buffer(256)
            _lib.gibson_last_error_message(err_buf, 256)
            raise RuntimeError(f"Failed to create Gibson context: {err_buf.value.decode('utf-8')}")

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()

    def close(self):
        if self.handle:
            _lib.gibson_destroy_context(self.handle)
            self.handle = None

    def set_root(self, root_node):
        status = _lib.gibson_set_root_node(self.handle, root_node.release())
        if status != 0:
            raise RuntimeError(f"Failed to set root node: status {status}")

    def render(self):
        status = _lib.gibson_render(self.handle)
        if status != 0:
            raise RuntimeError(f"Render failed: status {status}")

    def commit(self, text):
        status = _lib.gibson_commit(self.handle, text.encode("utf-8"))
        if status != 0:
            raise RuntimeError(f"Commit failed: status {status}")

    def stats(self):
        s = GibsonStats()
        _lib.gibson_get_stats(self.handle, ctypes.byref(s))
        return s

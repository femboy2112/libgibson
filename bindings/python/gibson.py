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
        ("struct_size", ctypes.c_uint32),
        ("abi_version", ctypes.c_uint32),
        ("frames", ctypes.c_uint64),
        ("skipped_frames", ctypes.c_uint64),
        ("dirty_cells", ctypes.c_uint64),
        ("total_cells", ctypes.c_uint64),
        ("frame_bytes", ctypes.c_uint64),
        ("full_repaints", ctypes.c_uint64),
        ("last_render_duration_micros", ctypes.c_uint64),
        ("history_insertions", ctypes.c_uint64),
        ("insertion_repaints", ctypes.c_uint64),
        ("fast_insertions", ctypes.c_uint64),
        ("insertion_bytes", ctypes.c_uint64),
        ("anchor_resyncs", ctypes.c_uint64),
        ("commit_bytes", ctypes.c_uint64),
        ("control_bytes", ctypes.c_uint64),
    ]

    def __init__(self):
        super().__init__()
        self.struct_size = ctypes.sizeof(GibsonStats)
        self.abi_version = _lib.gibson_abi_version()

# Setup ctypes signatures
_lib.gibson_create_context.argtypes = [ctypes.c_int32, ctypes.POINTER(ctypes.c_void_p)]
_lib.gibson_create_context.restype = ctypes.c_int32

_lib.gibson_destroy_context.argtypes = [ctypes.c_void_p]
_lib.gibson_destroy_context.restype = None

_lib.gibson_set_sync_updates.argtypes = [ctypes.c_void_p, ctypes.c_int32]
_lib.gibson_set_sync_updates.restype = ctypes.c_int32

_lib.gibson_set_max_fps.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
_lib.gibson_set_max_fps.restype = ctypes.c_int32

_lib.gibson_set_root_node.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
_lib.gibson_set_root_node.restype = ctypes.c_int32

_lib.gibson_render.argtypes = [ctypes.c_void_p]
_lib.gibson_render.restype = ctypes.c_int32

_lib.gibson_render_if_due.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_int32)]
_lib.gibson_render_if_due.restype = ctypes.c_int32

_lib.gibson_request_render.argtypes = [ctypes.c_void_p]
_lib.gibson_request_render.restype = ctypes.c_int32

_lib.gibson_commit.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
_lib.gibson_commit.restype = ctypes.c_int32

_lib.gibson_insert_text_before_live.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
_lib.gibson_insert_text_before_live.restype = ctypes.c_int32

_lib.gibson_insert_raw_lines_before_live_unchecked.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
_lib.gibson_insert_raw_lines_before_live_unchecked.restype = ctypes.c_int32

_lib.gibson_commit_node.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
_lib.gibson_commit_node.restype = ctypes.c_int32

_lib.gibson_insert_node_before_live.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
_lib.gibson_insert_node_before_live.restype = ctypes.c_int32

_lib.gibson_clear_live_region.argtypes = [ctypes.c_void_p]
_lib.gibson_clear_live_region.restype = ctypes.c_int32

_lib.gibson_get_stats.argtypes = [ctypes.c_void_p, ctypes.POINTER(GibsonStats)]
_lib.gibson_get_stats.restype = ctypes.c_int32

_lib.gibson_node_box_col.argtypes = [ctypes.POINTER(ctypes.c_void_p)]
_lib.gibson_node_box_col.restype = ctypes.c_int32

_lib.gibson_node_box_row.argtypes = [ctypes.POINTER(ctypes.c_void_p)]
_lib.gibson_node_box_row.restype = ctypes.c_int32
_lib.gibson_node_stack.argtypes = [ctypes.POINTER(ctypes.c_void_p)]
_lib.gibson_node_stack.restype = ctypes.c_int32
_lib.gibson_node_dim.argtypes = [ctypes.POINTER(ctypes.c_void_p)]
_lib.gibson_node_dim.restype = ctypes.c_int32

_lib.gibson_node_text.argtypes = [ctypes.c_char_p, ctypes.POINTER(GibsonStyle), ctypes.c_int32, ctypes.POINTER(ctypes.c_void_p)]
_lib.gibson_node_text.restype = ctypes.c_int32

_lib.gibson_node_spinner.argtypes = [ctypes.c_uint32, ctypes.POINTER(GibsonStyle), ctypes.c_char_p, ctypes.POINTER(ctypes.c_void_p)]
_lib.gibson_node_spinner.restype = ctypes.c_int32

_lib.gibson_node_text_input.argtypes = [ctypes.c_char_p, ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(GibsonStyle), ctypes.POINTER(ctypes.c_void_p)]
_lib.gibson_node_text_input.restype = ctypes.c_int32

_lib.gibson_node_border_box.argtypes = [ctypes.c_int32, ctypes.POINTER(GibsonStyle), ctypes.POINTER(ctypes.c_void_p)]
_lib.gibson_node_border_box.restype = ctypes.c_int32

_lib.gibson_node_rule.argtypes = [ctypes.c_char_p, ctypes.POINTER(GibsonStyle), ctypes.POINTER(ctypes.c_void_p)]
_lib.gibson_node_rule.restype = ctypes.c_int32

_lib.gibson_node_rail.argtypes = [ctypes.POINTER(GibsonStyle), ctypes.POINTER(ctypes.c_void_p)]
_lib.gibson_node_rail.restype = ctypes.c_int32

_lib.gibson_node_add_child.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
_lib.gibson_node_add_child.restype = ctypes.c_int32

_lib.gibson_node_set_width.argtypes = [ctypes.c_void_p, ctypes.c_float]
_lib.gibson_node_set_width.restype = ctypes.c_int32

_lib.gibson_node_set_height.argtypes = [ctypes.c_void_p, ctypes.c_float]
_lib.gibson_node_set_height.restype = ctypes.c_int32

_lib.gibson_node_set_percent_width.argtypes = [ctypes.c_void_p, ctypes.c_float]
_lib.gibson_node_set_percent_width.restype = ctypes.c_int32

_lib.gibson_node_set_min_width.argtypes = [ctypes.c_void_p, ctypes.c_float]
_lib.gibson_node_set_min_width.restype = ctypes.c_int32

_lib.gibson_node_set_max_width.argtypes = [ctypes.c_void_p, ctypes.c_float]
_lib.gibson_node_set_max_width.restype = ctypes.c_int32

_lib.gibson_node_set_flex_grow.argtypes = [ctypes.c_void_p, ctypes.c_float]
_lib.gibson_node_set_flex_grow.restype = ctypes.c_int32

_lib.gibson_node_set_gap.argtypes = [ctypes.c_void_p, ctypes.c_float]
_lib.gibson_node_set_gap.restype = ctypes.c_int32

_lib.gibson_node_set_padding.argtypes = [ctypes.c_void_p, ctypes.c_float]
_lib.gibson_node_set_padding.restype = ctypes.c_int32

_lib.gibson_node_set_padding_sides.argtypes = [ctypes.c_void_p, ctypes.c_float, ctypes.c_float, ctypes.c_float, ctypes.c_float]
_lib.gibson_node_set_padding_sides.restype = ctypes.c_int32

_lib.gibson_node_free.argtypes = [ctypes.c_void_p]
_lib.gibson_node_free.restype = None

_lib.gibson_last_error_message.argtypes = [ctypes.c_char_p, ctypes.c_size_t]
_lib.gibson_last_error_message.restype = ctypes.c_int32

# Versioning / stats init
_lib.gibson_abi_version.argtypes = []
_lib.gibson_abi_version.restype = ctypes.c_uint32

_lib.gibson_stats_init.argtypes = [ctypes.POINTER(GibsonStats)]
_lib.gibson_stats_init.restype = None

# Structured commits / rich text
_lib.gibson_commit_text.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
_lib.gibson_commit_text.restype = ctypes.c_int32

_lib.gibson_commit_raw_ansi_unchecked.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
_lib.gibson_commit_raw_ansi_unchecked.restype = ctypes.c_int32

_lib.gibson_commit_rich_text.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
_lib.gibson_commit_rich_text.restype = ctypes.c_int32

_lib.gibson_insert_rich_text_before_live.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
_lib.gibson_insert_rich_text_before_live.restype = ctypes.c_int32

_lib.gibson_node_rich_text.argtypes = [ctypes.c_void_p, ctypes.c_int32, ctypes.POINTER(ctypes.c_void_p)]
_lib.gibson_node_rich_text.restype = ctypes.c_int32

_lib.gibson_line_new.argtypes = [ctypes.POINTER(ctypes.c_void_p)]
_lib.gibson_line_new.restype = ctypes.c_int32

_lib.gibson_line_add_span.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.POINTER(GibsonStyle)]
_lib.gibson_line_add_span.restype = ctypes.c_int32

_lib.gibson_line_set_align.argtypes = [ctypes.c_void_p, ctypes.c_int32]
_lib.gibson_line_set_align.restype = ctypes.c_int32

_lib.gibson_line_free.argtypes = [ctypes.c_void_p]
_lib.gibson_line_free.restype = None

_lib.gibson_rich_text_new.argtypes = [ctypes.POINTER(ctypes.c_void_p)]
_lib.gibson_rich_text_new.restype = ctypes.c_int32

_lib.gibson_rich_text_add_line.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
_lib.gibson_rich_text_add_line.restype = ctypes.c_int32

_lib.gibson_rich_text_free.argtypes = [ctypes.c_void_p]
_lib.gibson_rich_text_free.restype = None


class Line:
    """A single line composed of independently styled spans."""

    def __init__(self):
        self.handle = ctypes.c_void_p()
        if _lib.gibson_line_new(ctypes.byref(self.handle)) != 0:
            raise RuntimeError("Failed to create Line")

    def add_span(self, text, style=None):
        style_ref = ctypes.byref(style) if style is not None else None
        status = _lib.gibson_line_add_span(self.handle, text.encode("utf-8"), style_ref)
        if status != 0:
            raise RuntimeError(f"add_span failed: {status}")
        return self

    def align(self, mode):
        status = _lib.gibson_line_set_align(self.handle, int(mode))
        if status != 0:
            raise RuntimeError(f"set_align failed: {status}")
        return self

    def release(self):
        h = self.handle
        self.handle = None
        return h

    def __del__(self):
        if getattr(self, "handle", None):
            _lib.gibson_line_free(self.handle)
            self.handle = None


class RichText:
    """A language-neutral structured multi-line rich text block."""

    def __init__(self):
        self.handle = ctypes.c_void_p()
        if _lib.gibson_rich_text_new(ctypes.byref(self.handle)) != 0:
            raise RuntimeError("Failed to create RichText")

    def add_line(self, line):
        status = _lib.gibson_rich_text_add_line(self.handle, line.handle)
        if status != 0:
            raise RuntimeError(f"add_line failed: {status}")
        return self

    def to_node(self, wrap=WrapMode.WORD):
        h = ctypes.c_void_p()
        status = _lib.gibson_node_rich_text(self.handle, wrap, ctypes.byref(h))
        if status != 0:
            raise RuntimeError(f"node_rich_text failed: {status}")
        return Node(h)

    def release(self):
        h = self.handle
        self.handle = None
        return h

    def __del__(self):
        if getattr(self, "handle", None):
            _lib.gibson_rich_text_free(self.handle)
            self.handle = None



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
    def stack(cls):
        h = ctypes.c_void_p()
        _lib.gibson_node_stack(ctypes.byref(h))
        return cls(h)

    @classmethod
    def dim(cls):
        h = ctypes.c_void_p()
        _lib.gibson_node_dim(ctypes.byref(h))
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

    @classmethod
    def rule(cls, title=None, style=None):
        h = ctypes.c_void_p()
        title_bytes = title.encode("utf-8") if title else None
        style_ref = ctypes.byref(style) if style is not None else None
        _lib.gibson_node_rule(title_bytes, style_ref, ctypes.byref(h))
        return cls(h)

    @classmethod
    def rail(cls, style=None):
        h = ctypes.c_void_p()
        style_ref = ctypes.byref(style) if style is not None else None
        _lib.gibson_node_rail(style_ref, ctypes.byref(h))
        return cls(h)

    @classmethod
    def spinner(cls, frame_index=0, label=None, style=None):
        h = ctypes.c_void_p()
        label_bytes = label.encode("utf-8") if label else None
        style_ref = ctypes.byref(style) if style is not None else None
        _lib.gibson_node_spinner(frame_index, style_ref, label_bytes, ctypes.byref(h))
        return cls(h)

    @classmethod
    def text_input(cls, value, cursor_grapheme=0, placeholder=None, style=None):
        h = ctypes.c_void_p()
        placeholder_bytes = placeholder.encode("utf-8") if placeholder else None
        style_ref = ctypes.byref(style) if style is not None else None
        _lib.gibson_node_text_input(value.encode("utf-8"), cursor_grapheme, placeholder_bytes, style_ref, ctypes.byref(h))
        return cls(h)

    def width(self, w):
        if self.handle:
            _lib.gibson_node_set_width(self.handle, float(w))
        return self

    def height(self, h):
        if self.handle:
            _lib.gibson_node_set_height(self.handle, float(h))
        return self

    def percent_width(self, pw):
        if self.handle:
            _lib.gibson_node_set_percent_width(self.handle, float(pw))
        return self

    def min_width(self, mw):
        if self.handle:
            _lib.gibson_node_set_min_width(self.handle, float(mw))
        return self

    def max_width(self, mw):
        if self.handle:
            _lib.gibson_node_set_max_width(self.handle, float(mw))
        return self

    def flex_grow(self, grow):
        if self.handle:
            _lib.gibson_node_set_flex_grow(self.handle, float(grow))
        return self

    def gap(self, g):
        if self.handle:
            _lib.gibson_node_set_gap(self.handle, float(g))
        return self

    def padding(self, p):
        if self.handle:
            _lib.gibson_node_set_padding(self.handle, float(p))
        return self

    def padding_sides(self, left, right, top, bottom):
        if self.handle:
            _lib.gibson_node_set_padding_sides(self.handle, float(left), float(right), float(top), float(bottom))
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

    def set_sync_updates(self, enabled: bool):
        _lib.gibson_set_sync_updates(self.handle, 1 if enabled else 0)

    def set_max_fps(self, fps: int):
        _lib.gibson_set_max_fps(self.handle, fps)

    def set_root(self, root_node):
        status = _lib.gibson_set_root_node(self.handle, root_node.release())
        if status != 0:
            raise RuntimeError(f"Failed to set root node: status {status}")

    def render(self):
        status = _lib.gibson_render(self.handle)
        if status != 0:
            raise RuntimeError(f"Render failed: status {status}")

    def render_if_due(self) -> bool:
        rendered = ctypes.c_int32(0)
        status = _lib.gibson_render_if_due(self.handle, ctypes.byref(rendered))
        if status != 0:
            raise RuntimeError(f"Render if due failed: status {status}")
        return rendered.value != 0

    def request_render(self):
        status = _lib.gibson_request_render(self.handle)
        if status != 0:
            raise RuntimeError(f"Request render failed: status {status}")

    def commit(self, text: str):
        status = _lib.gibson_commit_text(self.handle, text.encode("utf-8"))
        if status != 0:
            raise RuntimeError(f"Commit failed: status {status}")

    def commit_text(self, text: str):
        self.commit(text)

    def commit_raw_ansi_unchecked(self, text: str):
        status = _lib.gibson_commit_raw_ansi_unchecked(self.handle, text.encode("utf-8"))
        if status != 0:
            raise RuntimeError(f"Commit raw failed: status {status}")

    def commit_rich_text(self, rich: "RichText"):
        status = _lib.gibson_commit_rich_text(self.handle, rich.handle)
        if status != 0:
            raise RuntimeError(f"Commit rich text failed: status {status}")

    def insert_rich_text_before_live(self, rich: "RichText"):
        status = _lib.gibson_insert_rich_text_before_live(self.handle, rich.handle)
        if status != 0:
            raise RuntimeError(f"Insert rich text failed: status {status}")

    def insert_text_before_live(self, text: str):
        """Insert safe, width-aware plain text above the live region.

        Terminal control characters are neutralized by the engine; untrusted
        text cannot inject escape sequences.
        """
        status = _lib.gibson_insert_text_before_live(self.handle, text.encode("utf-8"))
        if status != 0:
            raise RuntimeError(f"Insert text before live failed: status {status}")

    def insert_raw_lines_before_live_unchecked(self, text: str):
        """Insert raw text above the live region WITHOUT sanitization.

        The text is a terminal byte stream; embedded escape/OSC/CSI sequences
        reach the terminal. Prefer insert_text_before_live.
        """
        status = _lib.gibson_insert_raw_lines_before_live_unchecked(self.handle, text.encode("utf-8"))
        if status != 0:
            raise RuntimeError(f"Insert raw lines before live failed: status {status}")

    def commit_node(self, node: Node):
        status = _lib.gibson_commit_node(self.handle, node.release())
        if status != 0:
            raise RuntimeError(f"Commit node failed: status {status}")

    def insert_node_before_live(self, node: Node):
        status = _lib.gibson_insert_node_before_live(self.handle, node.release())
        if status != 0:
            raise RuntimeError(f"Insert node before live failed: status {status}")

    def clear_live_region(self):
        status = _lib.gibson_clear_live_region(self.handle)
        if status != 0:
            raise RuntimeError(f"Clear live region failed: status {status}")

    def stats(self):
        s = GibsonStats()
        _lib.gibson_stats_init(ctypes.byref(s))
        status = _lib.gibson_get_stats(self.handle, ctypes.byref(s))
        if status != 0:
            raise RuntimeError(f"get_stats failed: status {status}")
        return s

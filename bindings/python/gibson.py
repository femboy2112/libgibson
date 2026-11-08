"""
LibGibson Python Wrapper using ctypes.
Provides a clean, idiomatic interface to the LibGibson differential terminal UI engine.
"""

import ctypes
import dataclasses
import os
import sys
from typing import Optional, Union

import ctypes.util

# The C ABI version this wrapper is built against (GIBSON_ABI_VERSION).
EXPECTED_ABI_VERSION = 1


def _load_library():
    """Locate and load the native libgibson shared library.

    Search order (an installed wrapper never silently walks a source checkout):
      1. the ``LIBGIBSON_LIBRARY`` environment variable, an explicit path;
      2. the system loader via ``ctypes.util.find_library("gibson")``;
      3. a bare ``libgibson.so`` resolved by the dynamic loader (honours
         ``LD_LIBRARY_PATH``), which is how a staged SDK is typically found.

    For in-repository development, point ``LIBGIBSON_LIBRARY`` at
    ``target/release/libgibson.so`` (or add that directory to ``LD_LIBRARY_PATH``).
    """
    explicit = os.environ.get("LIBGIBSON_LIBRARY")
    if explicit:
        if not os.path.exists(explicit):
            raise RuntimeError(
                f"LIBGIBSON_LIBRARY is set to {explicit!r}, which does not exist."
            )
        return ctypes.CDLL(explicit)

    found = ctypes.util.find_library("gibson")
    if found:
        return ctypes.CDLL(found)

    try:
        return ctypes.CDLL("libgibson.so")
    except OSError as exc:
        raise RuntimeError(
            "Could not locate the native libgibson shared library. Install the "
            "LibGibson native SDK and either set LIBGIBSON_LIBRARY to the path of "
            "libgibson.so or add its directory to LD_LIBRARY_PATH."
        ) from exc


_lib = _load_library()

# Verify the native ABI matches what this wrapper was built against, before any
# other call touches the library — fail clearly instead of segfaulting later.
_lib.gibson_abi_version.argtypes = []
_lib.gibson_abi_version.restype = ctypes.c_uint32
_loaded_abi_version = _lib.gibson_abi_version()
if _loaded_abi_version != EXPECTED_ABI_VERSION:
    raise RuntimeError(
        f"LibGibson ABI mismatch: this wrapper expects ABI {EXPECTED_ABI_VERSION}, "
        f"but the loaded native library reports ABI {_loaded_abi_version}. Install a "
        f"compatible libgibson (see docs/RELEASE_CONTRACT.md)."
    )

# Enums
class RenderMode:
    INLINE = 0
    FULLSCREEN = 1

class ColorDepth:
    # Downgrade ladder: TRUECOLOR -> ANSI256 -> ANSI16 -> MONO.
    MONO = 0
    ANSI16 = 1
    ANSI256 = 2
    TRUECOLOR = 3

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

class Status:
    """gibson_status_t. Non-OK statuses surface as GibsonError (a RuntimeError)."""
    OK = 0
    ERR_INVALID_PARAM = -1
    ERR_IO = -2
    ERR_PANIC = -3
    ERR_NOT_INITIALIZED = -4
    ERR_QUEUE_FULL = -5  # push_event only

class EventType:
    """gibson_event_type_t."""
    NONE = 0
    KEY = 1
    PASTE = 2
    RESIZE = 3

class KeyKind:
    """GIBSON_KEY_KIND_*: how to read a KEY event's key_code. GIBSON_KEY_* occupy U+1001-U+100D,
    which are also real Unicode scalars (Myanmar letters); only the kind tells them apart."""
    NONE = 0     # not a key event
    CHAR = 1     # key_code is a typed Unicode scalar value
    SPECIAL = 2  # key_code is a Key.* constant

class Key:
    """GIBSON_KEY_* special keys (the value of a SPECIAL key event's key_code)."""
    ENTER = 0x1001
    LEFT = 0x1002
    RIGHT = 0x1003
    UP = 0x1004
    DOWN = 0x1005
    HOME = 0x1006
    END = 0x1007
    BACKSPACE = 0x1008
    DELETE = 0x1009
    ESC = 0x100A
    TAB = 0x100B
    PAGE_UP = 0x100C
    PAGE_DOWN = 0x100D

class Mod:
    """GIBSON_MOD_* key-modifier bits."""
    SHIFT = 0x01
    CONTROL = 0x02
    ALT = 0x04

# Most undelivered events one headless context holds (GIBSON_EVENT_QUEUE_CAPACITY).
GIBSON_EVENT_QUEUE_CAPACITY = 1024


class GibsonError(RuntimeError):
    """A non-OK gibson_status_t from an event call. `status` is the Status code; `last_error` is
    the C ABI's last-error text, read on the raising thread right after the call ("" if none)."""

    def __init__(self, what, status, last_error=""):
        self.status = status
        self.last_error = last_error
        text = f"{what} failed: status {status}"
        super().__init__(f"{text}: {last_error}" if last_error else text)


class QueueFullError(GibsonError):
    """push_event on a headless context already holding GIBSON_EVENT_QUEUE_CAPACITY events."""


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

GIBSON_CELL_GLYPH_CAP = 16

class GibsonCell(ctypes.Structure):
    _fields_ = [
        ("glyph", ctypes.c_uint8 * GIBSON_CELL_GLYPH_CAP),
        ("glyph_len", ctypes.c_uint8),
        ("width", ctypes.c_uint8),
        ("is_continuation", ctypes.c_uint8),
        ("transparent", ctypes.c_uint8),
        ("_reserved", ctypes.c_uint8 * 4),
        ("style", GibsonStyle),
    ]

    @property
    def glyph_str(self) -> str:
        """The grapheme as a Python str (decoded from the written UTF-8 bytes)."""
        return bytes(self.glyph[: self.glyph_len]).decode("utf-8", "replace")

    @property
    def glyph_truncated(self) -> bool:
        """True iff the full grapheme cluster exceeded GIBSON_CELL_GLYPH_CAP and `glyph` holds only
        a codepoint-boundary prefix; fetch the whole cluster with Context.frame_cell_glyph()."""
        return bool(self._reserved[0])

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


class GibsonEvent(ctypes.Structure):
    """gibson_event_t, field for field. Most callers want the Event value type instead."""
    _fields_ = [
        ("event_type", ctypes.c_int32),
        ("key_code", ctypes.c_uint32),
        ("modifiers", ctypes.c_uint8),
        ("_pad", ctypes.c_uint8 * 3),
        ("cols", ctypes.c_uint16),
        ("rows", ctypes.c_uint16),
        ("paste_str", ctypes.c_char_p),
    ]


@dataclasses.dataclass(frozen=True)
class Event:
    """One input event, as pushed to or polled from a headless context.

    Fields the event type does not use are 0 / None (poll writes them that way). `paste` is a str
    when polled; push also accepts bytes (e.g. to hand the engine invalid UTF-8 on purpose).
    `key_kind` is a KeyKind for a KEY event polled with kind (CHAR vs SPECIAL, so a typed U+1001
    stays distinct from Key.ENTER) and KeyKind.NONE for other event types; it is None only when
    polled with ``kind=False``. Pushing key_kind=None uses the kind-less call, where U+1001-U+100D
    always mean the special keys. A polled KEY, PASTE or RESIZE Event can be pushed back as is;
    a NONE (empty queue) cannot, because the C ABI refuses to queue NONE."""
    event_type: int = EventType.NONE
    key_code: int = 0
    modifiers: int = 0
    cols: int = 0
    rows: int = 0
    paste: Union[str, bytes, None] = None
    key_kind: Optional[int] = None

    @classmethod
    def char(cls, ch: Union[str, int], modifiers: int = 0) -> "Event":
        """A typed character (KeyKind.CHAR); `ch` is a one-character str or a scalar value."""
        code = ord(ch) if isinstance(ch, str) else ch
        return cls(EventType.KEY, code, modifiers, key_kind=KeyKind.CHAR)

    @classmethod
    def special(cls, key: int, modifiers: int = 0) -> "Event":
        """A special key (KeyKind.SPECIAL); `key` is a Key.* constant."""
        return cls(EventType.KEY, key, modifiers, key_kind=KeyKind.SPECIAL)

    @classmethod
    def paste_text(cls, text: Union[str, bytes]) -> "Event":
        return cls(EventType.PASTE, paste=text)

    @classmethod
    def resize(cls, cols: int, rows: int) -> "Event":
        return cls(EventType.RESIZE, cols=cols, rows=rows)


def _event_to_struct(ev: Event):
    """Event -> (GibsonEvent, keepalive). Range-checks what a C field would silently wrap."""
    for name, val, hi in (("event_type", ev.event_type, 2**31 - 1), ("key_code", ev.key_code, 2**32 - 1),
                          ("modifiers", ev.modifiers, 0xFF), ("cols", ev.cols, 0xFFFF),
                          ("rows", ev.rows, 0xFFFF)):
        lo = -(2**31) if name == "event_type" else 0
        if not isinstance(val, int) or not lo <= val <= hi:
            raise ValueError(f"Event.{name}={val!r} does not fit the C field")
    c = GibsonEvent()
    c.event_type, c.key_code, c.modifiers, c.cols, c.rows = (
        ev.event_type, ev.key_code, ev.modifiers, ev.cols, ev.rows)
    keep = None
    if ev.paste is not None:
        keep = ev.paste.encode("utf-8") if isinstance(ev.paste, str) else bytes(ev.paste)
        if b"\0" in keep:
            raise ValueError("Event.paste contains a NUL byte (the C string would be cut short)")
        c.paste_str = keep
    return c, keep


def _event_from_struct(c: GibsonEvent, key_kind) -> Event:
    # paste_str is only valid until the next poll: copy it out now.
    paste = c.paste_str.decode("utf-8") if c.event_type == EventType.PASTE and c.paste_str is not None else None
    return Event(c.event_type, c.key_code, c.modifiers, c.cols, c.rows, paste, key_kind)

# Setup ctypes signatures
_lib.gibson_create_context.argtypes = [ctypes.c_int32, ctypes.POINTER(ctypes.c_void_p)]
_lib.gibson_create_context.restype = ctypes.c_int32

_lib.gibson_create_headless_context.argtypes = [ctypes.c_int32, ctypes.c_uint16, ctypes.c_uint16, ctypes.POINTER(ctypes.c_void_p)]
_lib.gibson_create_headless_context.restype = ctypes.c_int32

_lib.gibson_get_rendered_output.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_uint8), ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)]
_lib.gibson_get_rendered_output.restype = ctypes.c_int32

_lib.gibson_take_rendered_output.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_uint8), ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)]
_lib.gibson_take_rendered_output.restype = ctypes.c_int32

_lib.gibson_clear_rendered_output.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_size_t)]
_lib.gibson_clear_rendered_output.restype = ctypes.c_int32

_lib.gibson_get_frame_size.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_uint16), ctypes.POINTER(ctypes.c_uint16)]
_lib.gibson_get_frame_size.restype = ctypes.c_int32

_lib.gibson_get_frame_cell.argtypes = [ctypes.c_void_p, ctypes.c_uint16, ctypes.c_uint16, ctypes.POINTER(GibsonCell)]
_lib.gibson_get_frame_cell.restype = ctypes.c_int32

_lib.gibson_get_frame_cells.argtypes = [ctypes.c_void_p, ctypes.c_uint16, ctypes.c_uint16, ctypes.POINTER(GibsonCell), ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)]
_lib.gibson_get_frame_cells.restype = ctypes.c_int32

_lib.gibson_get_frame_cell_glyph.argtypes = [ctypes.c_void_p, ctypes.c_uint16, ctypes.c_uint16, ctypes.POINTER(ctypes.c_uint8), ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)]
_lib.gibson_get_frame_cell_glyph.restype = ctypes.c_int32

_lib.gibson_destroy_context.argtypes = [ctypes.c_void_p]
_lib.gibson_destroy_context.restype = None

_lib.gibson_set_sync_updates.argtypes = [ctypes.c_void_p, ctypes.c_int32]
_lib.gibson_set_sync_updates.restype = ctypes.c_int32

_lib.gibson_set_max_fps.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
_lib.gibson_set_max_fps.restype = ctypes.c_int32

_lib.gibson_set_color_depth.argtypes = [ctypes.c_void_p, ctypes.c_int32]
_lib.gibson_set_color_depth.restype = ctypes.c_int32

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

_lib.gibson_push_event.argtypes = [ctypes.c_void_p, ctypes.POINTER(GibsonEvent)]
_lib.gibson_push_event.restype = ctypes.c_int32
_lib.gibson_push_event_kind.argtypes = [ctypes.c_void_p, ctypes.POINTER(GibsonEvent), ctypes.c_uint32]
_lib.gibson_push_event_kind.restype = ctypes.c_int32
_lib.gibson_poll_event.argtypes = [ctypes.c_void_p, ctypes.c_uint32, ctypes.POINTER(GibsonEvent)]
_lib.gibson_poll_event.restype = ctypes.c_int32
_lib.gibson_poll_event_kind.argtypes = [ctypes.c_void_p, ctypes.c_uint32, ctypes.POINTER(GibsonEvent), ctypes.POINTER(ctypes.c_uint32)]
_lib.gibson_poll_event_kind.restype = ctypes.c_int32

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

    @classmethod
    def headless(cls, mode=RenderMode.INLINE, cols=80, rows=24):
        """Create a headless context that renders into an in-memory buffer with deterministic,
        TTY-independent semantics (fixed cols x rows, truecolor, no terminal lease). Read the
        painted ANSI frame back with rendered_output(). The capture keeps every frame until
        take_rendered_output() or clear_rendered_output() empties it. (ABI v1, additive.)"""
        self = cls.__new__(cls)
        self.handle = ctypes.c_void_p()
        status = _lib.gibson_create_headless_context(mode, cols, rows, ctypes.byref(self.handle))
        if status != 0:
            err_buf = ctypes.create_string_buffer(256)
            _lib.gibson_last_error_message(err_buf, 256)
            raise RuntimeError(
                f"Failed to create headless Gibson context: {err_buf.value.decode('utf-8')}"
            )
        return self

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

    def set_color_depth(self, depth: int):
        """Overrides the color depth (a ColorDepth constant). Call before the first render; see
        the note on gibson_set_color_depth. Raises on an unknown depth."""
        status = _lib.gibson_set_color_depth(self.handle, depth)
        if status != 0:
            raise RuntimeError(f"Failed to set color depth: status {status}")

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

    def rendered_output(self) -> bytes:
        """The raw ANSI frame bytes captured by a headless context since creation or the last
        take_rendered_output() / clear_rendered_output() (binary-safe; mirrors
        Context::rendered_bytes). Copies only, never drains. Empty for an interactive context."""
        need = ctypes.c_size_t(0)
        status = _lib.gibson_get_rendered_output(self.handle, None, 0, ctypes.byref(need))
        if status != 0:
            raise RuntimeError(f"rendered_output size query failed: status {status}")
        if need.value == 0:
            return b""
        buf = (ctypes.c_uint8 * need.value)()
        got = ctypes.c_size_t(0)
        status = _lib.gibson_get_rendered_output(self.handle, buf, need.value, ctypes.byref(got))
        if status != 0:
            raise RuntimeError(f"rendered_output copy failed: status {status}")
        return bytes(buf[: got.value])

    def take_rendered_output(self) -> bytes:
        """Drains the headless capture: returns its bytes and empties it (binary-safe; mirrors
        Context::take_rendered_bytes). Empty for an interactive context. (ABI v1, additive, #297.)

        The capture is copied and the Python ``bytes`` built BEFORE anything is removed, so a
        failure here (e.g. MemoryError for a large capture) leaves the capture intact. A context
        is not thread-safe: if another thread renders between the copy and the clear, the clear
        removes more than was returned and this raises RuntimeError."""
        while True:
            need = ctypes.c_size_t(0)
            status = _lib.gibson_get_rendered_output(self.handle, None, 0, ctypes.byref(need))
            if status != 0:
                raise RuntimeError(f"take_rendered_output size query failed: status {status}")
            if need.value == 0:
                return b""
            buf = (ctypes.c_uint8 * need.value)()
            got = ctypes.c_size_t(0)
            status = _lib.gibson_get_rendered_output(self.handle, buf, need.value, ctypes.byref(got))
            if status != 0:
                raise RuntimeError(f"take_rendered_output copy failed: status {status}")
            if got.value > need.value:
                continue  # the capture grew between the two calls; nothing was copied
            data = ctypes.string_at(buf, got.value)
            removed = self.clear_rendered_output()
            if removed != len(data):
                raise RuntimeError(
                    f"take_rendered_output: the capture changed between copy and clear "
                    f"({len(data)} copied, {removed} removed); contexts are not thread-safe")
            return data

    def clear_rendered_output(self) -> int:
        """Discards the headless capture without copying it; returns the number of bytes removed
        (0 for an interactive context). (ABI v1, additive, #297.)"""
        n = ctypes.c_size_t(0)
        status = _lib.gibson_clear_rendered_output(self.handle, ctypes.byref(n))
        if status != 0:
            raise RuntimeError(f"clear_rendered_output failed: status {status}")
        return n.value

    def frame_size(self):
        """(cols, rows) of the most recently rendered frame, or (0, 0) if there is no frame yet."""
        cols = ctypes.c_uint16(0)
        rows = ctypes.c_uint16(0)
        status = _lib.gibson_get_frame_size(self.handle, ctypes.byref(cols), ctypes.byref(rows))
        if status != 0:
            raise RuntimeError(f"frame_size failed: status {status}")
        return (cols.value, rows.value)

    def frame_cell(self, x: int, y: int) -> GibsonCell:
        """The resolved styled cell at (x, y) in the most recent frame — glyph, width, flags, and
        the style quantized to the context's color depth. Raises if there is no frame yet or
        (x, y) is out of bounds. Use `.glyph_str` for the grapheme as a str."""
        cell = GibsonCell()
        status = _lib.gibson_get_frame_cell(self.handle, x, y, ctypes.byref(cell))
        if status != 0:
            raise RuntimeError(f"frame_cell({x}, {y}) failed: status {status}")
        return cell

    def frame_cells(self, x: int = 0, y: int = 0, max_cells: int = None) -> list:
        """Bulk readback of resolved cells row-major from (x, y) to the end of the frame (or at most
        `max_cells`). One ABI call instead of N; identical per-cell encoding to frame_cell().
        Returns a list of GibsonCell."""
        cols, rows = self.frame_size()
        total = cols * rows
        start = y * cols + x
        avail = max(0, total - start)
        want = avail if max_cells is None else min(max_cells, avail)
        if want <= 0:
            return []
        buf = (GibsonCell * want)()
        written = ctypes.c_size_t(0)
        status = _lib.gibson_get_frame_cells(self.handle, x, y, buf, want, ctypes.byref(written))
        if status != 0:
            raise RuntimeError(f"frame_cells({x}, {y}) failed: status {status}")
        return [buf[i] for i in range(written.value)]

    def frame_cell_glyph(self, x: int, y: int) -> bytes:
        """The full (untruncated) grapheme-cluster UTF-8 bytes of the cell at (x, y) — the escape
        hatch for cells whose GibsonCell.glyph_truncated is True. Size-query first (buf=NULL), then
        the exact copy. Raises if there is no frame or (x, y) is out of bounds."""
        out_len = ctypes.c_size_t(0)
        status = _lib.gibson_get_frame_cell_glyph(self.handle, x, y, None, 0, ctypes.byref(out_len))
        if status != 0:
            raise RuntimeError(f"frame_cell_glyph({x}, {y}) size query failed: status {status}")
        n = out_len.value
        if n == 0:
            return b""
        buf = (ctypes.c_uint8 * n)()
        status = _lib.gibson_get_frame_cell_glyph(self.handle, x, y, buf, n, ctypes.byref(out_len))
        if status != 0:
            raise RuntimeError(f"frame_cell_glyph({x}, {y}) failed: status {status}")
        return bytes(buf[: out_len.value])

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

    def _event_error(self, what, status):
        # The last error is thread-local: read it now, on this thread, before any other call.
        buf = ctypes.create_string_buffer(512)
        _lib.gibson_last_error_message(buf, 512)
        cls = QueueFullError if status == Status.ERR_QUEUE_FULL else GibsonError
        return cls(what, status, buf.value.decode("utf-8", "replace"))

    def push_event(self, event: Event):
        """Queue `event` on a HEADLESS context for poll_event (GIBSON_EVENT_QUEUE_CAPACITY at most).

        With ``event.key_kind`` set, calls gibson_push_event_kind (KeyKind.CHAR pushes any scalar,
        U+1001-U+100D included; KeyKind.SPECIAL needs a Key.* value; any other kind is refused);
        with key_kind None, calls gibson_push_event, where U+1001-U+100D are always special keys.
        The kind is ignored for non-KEY events. PASTE needs `paste`; RESIZE needs non-zero cols and
        rows. Raises QueueFullError (full queue, nothing displaced) or GibsonError (terminal
        context, unrepresentable event: status Status.ERR_INVALID_PARAM) with the C last-error
        text. Raises ValueError (UnicodeEncodeError is one) when Python cannot build the C struct:
        a value that does not fit its field, or a paste str that is not encodable as UTF-8 or
        contains a NUL byte. Event validity itself is the C ABI's decision. (ABI v1, additive.)"""
        c, keep = _event_to_struct(event)
        if event.key_kind is None:
            status = _lib.gibson_push_event(self.handle, ctypes.byref(c))
        else:
            if not isinstance(event.key_kind, int) or not 0 <= event.key_kind <= 0xFFFFFFFF:
                raise ValueError(f"Event.key_kind={event.key_kind!r} does not fit the C field")
            status = _lib.gibson_push_event_kind(self.handle, ctypes.byref(c), event.key_kind)
        del keep
        if status != 0:
            raise self._event_error("push_event", status)

    def poll_event(self, timeout_ms: int = 0, kind: bool = True) -> Event:
        """Next event, oldest first. On a headless context this never waits and returns an Event
        with event_type EventType.NONE when the queue is empty; a polled RESIZE has already set the
        headless size (see gibson.h). On a terminal context it waits up to `timeout_ms`.

        ``kind=True`` (default) uses gibson_poll_event_kind, so a KEY event carries
        KeyKind.CHAR or KeyKind.SPECIAL; ``kind=False`` uses gibson_poll_event, where a typed
        U+1001 and Key.ENTER look the same and key_kind is None. Every field is written, so a
        polled KEY, PASTE or RESIZE can go straight back into push_event (a NONE from an empty
        queue cannot: the C ABI refuses to queue NONE). Raises GibsonError on a non-OK status."""
        c = GibsonEvent()
        if kind:
            k = ctypes.c_uint32(0)
            status = _lib.gibson_poll_event_kind(self.handle, timeout_ms, ctypes.byref(c), ctypes.byref(k))
        else:
            status = _lib.gibson_poll_event(self.handle, timeout_ms, ctypes.byref(c))
        if status != 0:
            raise self._event_error("poll_event", status)
        return _event_from_struct(c, k.value if kind else None)

    def stats(self):
        s = GibsonStats()
        _lib.gibson_stats_init(ctypes.byref(s))
        status = _lib.gibson_get_stats(self.handle, ctypes.byref(s))
        if status != 0:
            raise RuntimeError(f"get_stats failed: status {status}")
        return s

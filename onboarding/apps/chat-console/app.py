#!/usr/bin/env python3
"""An offline echo console. It does not contact a model or a network service."""
import argparse
import os
import shutil
import sys

from gibson import Context, Event, EventType, Key, KeyKind, Mod, Node, RenderMode, WrapMode

MAX_INPUT = 200
MAX_MESSAGES = 50


class Console:
    def __init__(self, rows):
        self.rows = rows
        self.draft = ""
        self.messages = []
        self.done = False

    def add_text(self, text):
        # Only printable characters reach the terminal UI and C strings. This simple
        # editor backspaces one code point, not a full Unicode grapheme cluster.
        text = "".join(ch for ch in text if ch.isprintable())
        self.draft = (self.draft + text)[:MAX_INPUT]

    def handle(self, ev):
        if ev.event_type == EventType.RESIZE:
            self.rows = ev.rows
        elif ev.event_type == EventType.PASTE:
            self.add_text(ev.paste or "")
        elif ev.event_type == EventType.KEY:
            if ev.key_kind == KeyKind.SPECIAL:
                if ev.key_code == Key.ESC:
                    self.done = True
                elif ev.key_code == Key.BACKSPACE:
                    self.draft = self.draft[:-1]
                elif ev.key_code == Key.ENTER and self.draft.strip():
                    text = self.draft.strip()
                    self.messages.extend((f"You: {text}", f"Echo (local): {text}"))
                    self.messages = self.messages[-MAX_MESSAGES:]
                    self.draft = ""
            elif ev.key_kind == KeyKind.CHAR:
                if ev.modifiers & Mod.CONTROL:
                    if chr(ev.key_code).lower() == "c":
                        self.done = True
                elif not ev.modifiers & Mod.ALT:
                    self.add_text(chr(ev.key_code))

    def view(self):
        root = Node.col().percent_width(100.0)
        root.add_child(Node.text("Offline echo console (no model connected)", wrap=WrapMode.NONE))
        root.add_child(Node.text("Enter: send  |  Esc: quit", wrap=WrapMode.NONE))
        for line in self.messages[-max(self.rows - 4, 0):] if self.rows > 4 else []:
            root.add_child(Node.text(line, wrap=WrapMode.NONE))
        root.add_child(Node.text("> " + self.draft, wrap=WrapMode.NONE))
        return root


def paint(ctx, app):
    ctx.set_root(app.view())
    ctx.render()


def line(ctx, row):
    cols, _ = ctx.frame_size()
    return "".join(ctx.frame_cell_glyph(x, row).decode("utf-8") or " " for x in range(cols))


def self_test():
    app = Console(15)
    with Context.headless(RenderMode.FULLSCREEN, 40, 15) as ctx:
        paint(ctx, app)
        assert "Offline echo console" in line(ctx, 0)
        for ev in (Event.char("h"), Event.char("i"), Event.special(Key.ENTER), Event.resize(40, 8)):
            ctx.push_event(ev)
            app.handle(ctx.poll_event())
        paint(ctx, app)
        assert ctx.frame_size() == (40, 8)
        assert "You: hi" in line(ctx, 2)
        assert "Echo (local): hi" in line(ctx, 3)
        ctx.push_event(Event.special(Key.ESC))
        app.handle(ctx.poll_event())
        assert app.done
    print("chat console: headless input, echo, resize and quit OK")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true", help="exercise a headless context")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return 0
    if not (sys.stdin.isatty() and sys.stdout.isatty()):
        print("chat console needs a TTY on stdin and stdout; use --self-test for headless mode", file=sys.stderr)
        return 2
    if os.environ.get("TERM") == "dumb":
        print("chat console needs an interactive terminal; TERM=dumb is unsupported", file=sys.stderr)
        return 2
    app = Console(shutil.get_terminal_size((80, 24)).lines)
    with Context(RenderMode.FULLSCREEN) as ctx:
        initial = ctx.poll_event(0)
        if initial.event_type != EventType.NONE:
            app.handle(initial)
        if not app.done:
            paint(ctx, app)
        while not app.done:
            ev = ctx.poll_event(100)
            if ev.event_type != EventType.NONE:
                app.handle(ev)
                paint(ctx, app)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, RuntimeError) as exc:
        print(f"chat console: {exc}", file=sys.stderr)
        sys.exit(1)

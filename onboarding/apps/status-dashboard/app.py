#!/usr/bin/env python3
"""A responsive status dashboard with an explicitly simulated producer."""
import argparse
import os
import sys
import time

from gibson import Context, Event, EventType, Key, KeyKind, Mod, Node, RenderMode, WrapMode


class Dashboard:
    def __init__(self):
        self.samples = 0
        self.paused = False
        self.done = False

    def sample(self):
        if not self.paused:
            self.samples += 1

    def handle(self, ev):
        if ev.event_type == EventType.KEY and ev.key_kind == KeyKind.SPECIAL:
            if ev.key_code == Key.ESC:
                self.done = True
        elif ev.event_type == EventType.KEY and ev.key_kind == KeyKind.CHAR:
            ch = chr(ev.key_code).lower()
            if ev.modifiers & Mod.CONTROL:
                if ch == "c":
                    self.done = True
            elif not ev.modifiers & Mod.ALT:
                if ch == "q":
                    self.done = True
                elif ch == "p":
                    self.paused = not self.paused
                elif ch == "r":
                    self.samples = 0

    def view(self):
        root = Node.col().percent_width(100.0)
        for text in (
            "Status dashboard | simulated producer",
            "State: " + ("paused" if self.paused else "running"),
            f"Samples: {self.samples}",
            f"Load: {(self.samples * 13) % 101}% (simulated)",
            "p: pause/resume  r: reset  q: quit",
        ):
            root.add_child(Node.text(text, wrap=WrapMode.NONE))
        return root


def paint(ctx, app):
    ctx.set_root(app.view())
    ctx.render()


def line(ctx, row):
    cols, _ = ctx.frame_size()
    return "".join(ctx.frame_cell_glyph(x, row).decode("utf-8") or " " for x in range(cols))


def self_test():
    app = Dashboard()
    with Context.headless(RenderMode.FULLSCREEN, 40, 15) as ctx:
        app.sample()
        paint(ctx, app)
        assert "simulated producer" in line(ctx, 0)
        assert "Samples: 1" in line(ctx, 2)
        for ev in (Event.char("p"), Event.resize(30, 8)):
            ctx.push_event(ev)
            app.handle(ctx.poll_event())
        app.sample()
        paint(ctx, app)
        assert ctx.frame_size() == (30, 8)
        assert "paused" in line(ctx, 1) and "Samples: 1" in line(ctx, 2)
        ctx.push_event(Event.char("r"))
        app.handle(ctx.poll_event())
        paint(ctx, app)
        assert "Samples: 0" in line(ctx, 2)
        ctx.push_event(Event.char("q"))
        app.handle(ctx.poll_event())
        assert app.done
    print("status dashboard: headless sampling, pause, reset, resize and quit OK")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true", help="exercise a headless context")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return 0
    if not (sys.stdin.isatty() and sys.stdout.isatty()):
        print("status dashboard needs a TTY on stdin and stdout; use --self-test for headless mode", file=sys.stderr)
        return 2
    if os.environ.get("TERM") == "dumb":
        print("status dashboard needs an interactive terminal; TERM=dumb is unsupported", file=sys.stderr)
        return 2
    app = Dashboard()
    with Context(RenderMode.FULLSCREEN) as ctx:
        initial = ctx.poll_event(0)
        if initial.event_type != EventType.NONE:
            app.handle(initial)
        if not app.done:
            paint(ctx, app)
        due = time.monotonic() + 1.0
        while not app.done:
            ev = ctx.poll_event(100)
            changed = ev.event_type != EventType.NONE
            if changed:
                app.handle(ev)
            now = time.monotonic()
            if now >= due:
                app.sample()
                due = now + 1.0
                changed = True
            if changed:
                paint(ctx, app)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, RuntimeError) as exc:
        print(f"status dashboard: {exc}", file=sys.stderr)
        sys.exit(1)

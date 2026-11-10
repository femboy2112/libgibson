#!/usr/bin/env python3
"""A simulated temperature monitor with a separately controlled setpoint."""
import argparse
import os
import sys
import time

from gibson import Context, Event, EventType, Key, KeyKind, Mod, Node, RenderMode, WrapMode


def degrees(tenths):
    """Format a non-negative integer number of tenths of a degree Celsius."""
    return f"{tenths // 10}.{tenths % 10}"


class Instrument:
    def __init__(self):
        # These are independent authorities: the simulated source owns the sample;
        # the user owns a requested setpoint. Neither writes the other.
        self.sample_number = 0
        self.sample_tenths = 200
        self.setpoint_tenths = 210
        self.paused = False
        self.done = False
        self.feedback = "ready"

    def sample(self):
        if not self.paused:
            self.sample_number += 1
            self.sample_tenths = 200 + (self.sample_number % 11) * 5

    def edit_setpoint(self, delta):
        candidate = self.setpoint_tenths + delta
        if not 100 <= candidate <= 400:
            self.feedback = "refused: range 10-40 C"
            return
        self.setpoint_tenths = candidate
        self.feedback = "setpoint " + degrees(candidate)

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
                elif ch in ("+", "="):
                    self.edit_setpoint(5)
                elif ch == "-":
                    self.edit_setpoint(-5)

    def view(self):
        status = "above" if self.sample_tenths > self.setpoint_tenths else "at/below"
        root = Node.col().percent_width(100.0)
        for text in (
            "Temperature instrument",
            "Source: simulated (not sensor)",
            f"Sample: {degrees(self.sample_tenths)} deg C",
            f"Setpoint: {degrees(self.setpoint_tenths)} deg C [user]",
            f"Status: {status} setpoint",
            "Producer: " + ("paused" if self.paused else "running"),
            "Action: " + self.feedback,
            "+/-: edit p: pause q: quit",
        ):
            root.add_child(Node.text(text, wrap=WrapMode.NONE))
        return root


def paint(ctx, app):
    ctx.set_root(app.view())
    ctx.render()


def line(ctx, row):
    cols, _ = ctx.frame_size()
    return "".join(ctx.frame_cell_glyph(x, row).decode("utf-8") or " " for x in range(cols))


def deliver(ctx, app, event):
    ctx.push_event(event)
    app.handle(ctx.poll_event())


def self_test():
    app = Instrument()
    with Context.headless(RenderMode.FULLSCREEN, 40, 15) as ctx:
        paint(ctx, app)
        assert "simulated (not sensor)" in line(ctx, 1)
        assert "Sample: 20.0 deg C" in line(ctx, 2)
        assert "Setpoint: 21.0 deg C [user]" in line(ctx, 3)
        deliver(ctx, app, Event.char("+"))
        paint(ctx, app)
        assert "Sample: 20.0 deg C" in line(ctx, 2)  # user edit did not fabricate a sample
        assert "Setpoint: 21.5 deg C [user]" in line(ctx, 3)
        app.sample()
        paint(ctx, app)
        assert "Sample: 20.5 deg C" in line(ctx, 2)
        assert "Setpoint: 21.5 deg C [user]" in line(ctx, 3)  # sample did not overwrite user edit
        for _ in range(3):
            app.sample()
        paint(ctx, app)
        assert "Sample: 22.0 deg C" in line(ctx, 2) and "above setpoint" in line(ctx, 4)
        assert "Setpoint: 21.5 deg C [user]" in line(ctx, 3)
        deliver(ctx, app, Event.char("p"))
        app.sample()
        deliver(ctx, app, Event.resize(30, 8))
        paint(ctx, app)
        assert ctx.frame_size() == (30, 8)
        assert "Sample: 22.0 deg C" in line(ctx, 2) and "paused" in line(ctx, 5)
        app.setpoint_tenths = 400
        deliver(ctx, app, Event.char("+"))
        paint(ctx, app)
        assert app.setpoint_tenths == 400 and "refused" in line(ctx, 6)
        assert app.sample_tenths == 220
        app.setpoint_tenths = 100
        deliver(ctx, app, Event.char("-"))
        paint(ctx, app)
        assert app.setpoint_tenths == 100 and "refused" in line(ctx, 6)
        assert app.sample_tenths == 220
        deliver(ctx, app, Event.char("q"))
        assert app.done
    print("temperature instrument: sample, user edit, refusal, pause, resize and quit OK")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true", help="exercise a headless context")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return 0
    if not (sys.stdin.isatty() and sys.stdout.isatty()):
        print("temperature instrument needs a TTY on stdin and stdout; use --self-test for headless mode", file=sys.stderr)
        return 2
    if os.environ.get("TERM") == "dumb":
        print("temperature instrument needs an interactive terminal; TERM=dumb is unsupported", file=sys.stderr)
        return 2
    app = Instrument()
    with Context(RenderMode.FULLSCREEN) as ctx:
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
        print(f"temperature instrument: {exc}", file=sys.stderr)
        sys.exit(1)

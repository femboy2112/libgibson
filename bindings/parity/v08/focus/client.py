import os
import sys
import gibson

mode = sys.argv[1]
ctx = gibson.Context()
if mode == "events":
    if ctx.poll_event(0).event_type != gibson.EventType.NONE:
        raise AssertionError("unexpected initial event")
    ctx.set_focus_reporting(True)  # The PTY peer sends two input sequences after this.
    first = ctx.poll_event(2000)
    second = ctx.poll_event(2000)
    extra = ctx.poll_event(0)
    if (first.event_type, second.event_type, extra.event_type) != (
            gibson.EventType.FOCUS_GAINED, gibson.EventType.FOCUS_LOST, gibson.EventType.NONE):
        print("FOCUS_ORDER_MISMATCH", file=sys.stderr)
        ctx.close()
        sys.exit(5)
    ctx.close()
    print("FOCUS_INPUT_OK", file=sys.stderr)
elif mode == "io_error":
    write_fd = os.open("/dev/full", os.O_WRONLY)
    os.dup2(write_fd, 1)
    os.close(write_fd)
    try:
        ctx.set_focus_reporting(True)
    except gibson.GibsonError as exc:
        if exc.status == gibson.Status.ERR_IO and "os error 28" in exc.last_error:
            print("EXPECTED_IO_ERROR", file=sys.stderr)
        else:
            raise
    else:
        raise AssertionError("failed write did not produce an I/O error")
    finally:
        ctx.close()
else:
    def enable():
        if mode != "noop":
            ctx.set_focus_reporting(True)
    enable(); enable()
    ctx.set_focus_reporting(False); ctx.set_focus_reporting(False)
    enable()
    ctx.close()
    print("COMPLETE", file=sys.stderr)

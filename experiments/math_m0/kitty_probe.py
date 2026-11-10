#!/usr/bin/env python3
"""Manual graphics-protocol probe, separate from the LibGibson renderer.

Run only inside a disposable Kitty window; do not use as a rendering backend.
"""
import base64
import os
import select
import sys
import termios
import time
import tty


def query(fd, command):
    os.write(fd, command)
    end = time.monotonic() + 2
    result = b''
    while time.monotonic() < end:
        ready, _, _ = select.select([fd], [], [], end - time.monotonic())
        if not ready:
            break
        result += os.read(fd, 4096)
        if b'\x1b\\' in result:
            break
    return result


def main():
    if len(sys.argv) != 2:
        raise SystemExit('usage: kitty_probe.py NEW_RESULT_FILE')
    # Refuse to overwrite prior evidence. Result path must be trusted.
    with open(sys.argv[1], 'xb') as result:
        with open('/dev/tty', 'r+b', buffering=0) as terminal:
            fd = terminal.fileno()
            original = termios.tcgetattr(fd)
            tty.setraw(fd)
            try:
                good = query(fd, b'\x1b_Gi=31,a=q,t=d,f=24,s=1,v=1;AAAA\x1b\\')
                bad = query(fd, b'\x1b_Gi=32,a=q,t=d,f=99,s=1,v=1;AAAA\x1b\\')
                result.write(b'GOOD=' + good + b'\nBAD=' + bad + b'\n')
                result.flush()
                os.write(fd, b'\x1b[2J\x1b[H')
                payload = base64.b64encode(bytes([255, 0, 0]) * 144)
                os.write(fd, b'\x1b_Ga=T,f=24,s=12,v=12,c=20,r=8,i=33;' + payload + b'\x1b\\')
                # Allow a separately controlled screenshot of this window.
                time.sleep(15)
            finally:
                os.write(fd, b'\x1b_Ga=d,d=i,i=33\x1b\\')
                termios.tcsetattr(fd, termios.TCSANOW, original)


if __name__ == '__main__':
    main()

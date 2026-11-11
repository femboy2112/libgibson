#!/usr/bin/env python3
"""Fault control: terminal restoration is attempted even if image deletion fails."""
import io
from pathlib import Path
import sys
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import kitty_probe

class Terminal:
    def __enter__(self): return self
    def __exit__(self, *args): return False
    def fileno(self): return 42


def write(_fd, data):
    if data.startswith(b'\x1b_Ga=d'):
        raise BrokenPipeError('injected delete failure')
    return len(data)


def main():
    original = ['saved-termios']
    with mock.patch.object(sys, 'argv', ['kitty_probe.py', 'unused']), \
         mock.patch.object(kitty_probe, 'open', side_effect=[io.BytesIO(), Terminal()], create=True), \
         mock.patch.object(kitty_probe.termios, 'tcgetattr', return_value=original), \
         mock.patch.object(kitty_probe.termios, 'tcsetattr') as restore, \
         mock.patch.object(kitty_probe.tty, 'setraw'), \
         mock.patch.object(kitty_probe, 'query', side_effect=[b'OK', b'EINVAL']), \
         mock.patch.object(kitty_probe.os, 'write', side_effect=write), \
         mock.patch.object(kitty_probe.time, 'sleep'):
        try:
            kitty_probe.main()
        except BrokenPipeError:
            pass
        else:
            raise AssertionError('injected write failure was not surfaced')
        restore.assert_called_once_with(42, kitty_probe.termios.TCSANOW, original)
    print('PASS: delete failure still attempts termios restoration')

if __name__ == '__main__': main()

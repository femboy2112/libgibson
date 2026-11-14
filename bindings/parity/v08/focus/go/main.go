package main

import (
	"errors"
	"fmt"
	"os"
	"strconv"
	"strings"
	"syscall"

	"github.com/femboy2112/libgibson/bindings/go/gibson"
)

func must(err error) {
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(2)
	}
}

// Since #371 a session writes its own duplicate of the terminal it acquired, not fd 1 as it is
// later. Point every open descriptor for fd 1's terminal device (fd 0 excepted) at /dev/full and
// report how many were moved.
func redirectTerminalWritesToFull() int {
	var out syscall.Stat_t
	must(syscall.Fstat(1, &out))
	full, err := syscall.Open("/dev/full", syscall.O_WRONLY|syscall.O_CLOEXEC, 0)
	must(err)
	entries, err := os.ReadDir("/proc/self/fd")
	must(err)
	moved := 0
	for _, e := range entries {
		fd, convErr := strconv.Atoi(e.Name())
		if convErr != nil || fd == 0 || fd == full {
			continue
		}
		var st syscall.Stat_t
		if syscall.Fstat(fd, &st) != nil || st.Mode&syscall.S_IFMT != syscall.S_IFCHR || st.Rdev != out.Rdev {
			continue
		}
		flags, _, _ := syscall.Syscall(syscall.SYS_FCNTL, uintptr(fd), syscall.F_GETFD, 0)
		dupFlags := 0
		if flags&syscall.FD_CLOEXEC != 0 {
			dupFlags = syscall.O_CLOEXEC
		}
		if syscall.Dup3(full, fd, dupFlags) == nil {
			moved++
		}
	}
	must(syscall.Close(full))
	return moved
}

func main() {
	mode := os.Args[1]
	ctx, err := gibson.NewContext(gibson.ModeInline)
	must(err)
	if mode == "events" {
		initial, initialErr := ctx.PollEventKind(0)
		must(initialErr)
		if initial.Type != gibson.EventNone {
			os.Exit(6)
		}
		must(ctx.SetFocusReporting(true)) // The PTY peer sends input sequences after this.
		first, e1 := ctx.PollEventKind(2000)
		must(e1)
		second, e2 := ctx.PollEventKind(2000)
		must(e2)
		extra, e3 := ctx.PollEventKind(0)
		must(e3)
		if first.Type != gibson.EventFocusGained || second.Type != gibson.EventFocusLost || extra.Type != gibson.EventNone {
			ctx.Close()
			fmt.Fprintln(os.Stderr, "FOCUS_ORDER_MISMATCH")
			os.Exit(5)
		}
		ctx.Close()
		fmt.Fprintln(os.Stderr, "FOCUS_INPUT_OK")
		return
	}
	if mode == "io_error" {
		if redirectTerminalWritesToFull() < 2 {
			os.Exit(2)
		}
		err = ctx.SetFocusReporting(true)
		var ge *gibson.Error
		if !errors.As(err, &ge) || ge.Status != gibson.StatusIO || !strings.Contains(ge.LastError, "os error 28") {
			fmt.Fprintln(os.Stderr, "missing expected I/O detail:", err)
			os.Exit(3)
		}
		ctx.Close()
		fmt.Fprintln(os.Stderr, "EXPECTED_IO_ERROR")
		return
	}
	enable := func() {
		if mode != "noop" {
			must(ctx.SetFocusReporting(true))
		}
	}
	enable()
	enable()
	must(ctx.SetFocusReporting(false))
	must(ctx.SetFocusReporting(false))
	enable()
	ctx.Close()
	fmt.Fprintln(os.Stderr, "COMPLETE")
}

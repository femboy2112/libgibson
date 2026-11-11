package main

import (
	"errors"
	"fmt"
	"os"
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
		fd, openErr := syscall.Open("/dev/full", syscall.O_WRONLY, 0)
		must(openErr)
		must(syscall.Dup2(fd, 1))
		must(syscall.Close(fd))
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

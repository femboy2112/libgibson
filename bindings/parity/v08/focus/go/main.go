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

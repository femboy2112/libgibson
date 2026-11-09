package main

import (
	"fmt"
	gibson "github.com/femboy2112/libgibson/bindings/go/gibson"
	"os"
)

func run() error {
	ctx, err := gibson.NewHeadlessContext(gibson.ModeFullscreen, 40, 8)
	if err != nil {
		return err
	}
	defer ctx.Close()
	root := gibson.NewColNode()
	text, err := gibson.NewStyledTextNode("Hello, LibGibson", nil, gibson.WrapNone)
	if err != nil {
		return err
	}
	root.AddChild(text)
	if err = ctx.SetRootNode(root); err != nil {
		return err
	}
	if err = ctx.Render(); err != nil {
		return err
	}
	cols, rows, err := ctx.FrameSize()
	if err != nil {
		return err
	}
	if cols != 40 || rows != 8 {
		return fmt.Errorf("unexpected frame size %dx%d", cols, rows)
	}
	glyph, err := ctx.FrameCellGlyph(0, 0)
	if err != nil {
		return err
	}
	if string(glyph) != "H" {
		return fmt.Errorf("unexpected frame content %q", glyph)
	}
	fmt.Printf("Go headless frame %dx%d OK\n", cols, rows)
	return nil
}
func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

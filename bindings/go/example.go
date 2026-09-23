// UNVERIFIED: Go compiler not available in current test environment
//go:build ignore
package main

import (
	"fmt"
	"os"

	"github.com/libgibson/libgibson/bindings/go/gibson"
)

func main() {
	fmt.Println("--- Running Go cgo Example with LibGibson ---")

	ctx, err := gibson.NewContext(gibson.ModeInline)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Failed to create context: %v\n", err)
		os.Exit(1)
	}
	defer ctx.Close()

	ctx.Commit("[Go cgo] Context initialized.")

	root := gibson.NewColNode().SetPercentWidth(100.0).SetMaxWidth(72.0).SetGap(1.0)
	root.AddChild(gibson.NewRuleNode("LibGibson Go cgo Demo"))

	rail := gibson.NewRailNode()
	rail.AddChild(gibson.NewTextNode("Clean Go cgo bindings over stable extern C ABI."))
	root.AddChild(rail)

	box := gibson.NewBorderBoxNode(gibson.BorderRounded).SetWidth(60.0).SetHeight(3.0)
	box.AddChild(gibson.NewTextNode("Type-safe Go wrapper around high-performance Rust core"))
	root.AddChild(box)

	ctx.SetRoot(root)
	if err := ctx.Render(); err != nil {
		fmt.Fprintf(os.Stderr, "Render failed: %v\n", err)
		os.Exit(1)
	}

	// Safe insertion: controls in the text are neutralized by the engine.
	if err := ctx.InsertTextBeforeLive("[Go cgo] Live asynchronous notice inserted before active region."); err != nil {
		fmt.Fprintf(os.Stderr, "insert text failed: %v\n", err)
		os.Exit(1)
	}

	// Raw escape hatch: explicitly unchecked and unsanitized.
	if err := ctx.InsertRawLinesBeforeLiveUnchecked("[Go cgo] Raw notice (unchecked path)."); err != nil {
		fmt.Fprintf(os.Stderr, "insert raw lines failed: %v\n", err)
		os.Exit(1)
	}

	ctx.Commit("[Go cgo] Output rendered and committed.")
	fmt.Println("[Go cgo] Example finished successfully.")
}

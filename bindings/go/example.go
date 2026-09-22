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

	root := gibson.NewColNode().SetWidth(70.0).SetGap(1.0)
	root.AddChild(gibson.NewTextNode("Go cgo Native Interface"))

	box := gibson.NewBorderBoxNode(gibson.BorderRounded).SetWidth(60.0).SetHeight(3.0)
	box.AddChild(gibson.NewTextNode("Clean Go bindings over stable extern C ABI"))
	root.AddChild(box)

	ctx.SetRoot(root)
	if err := ctx.Render(); err != nil {
		fmt.Fprintf(os.Stderr, "Render failed: %v\n", err)
		os.Exit(1)
	}

	ctx.Commit("[Go cgo] Output rendered and committed.")
	fmt.Println("[Go cgo] Example finished successfully.")
}

package main

import (
	"context"
	"os"

	"github.com/sachahjkl/dw/internal/bootstrap"
	"github.com/sachahjkl/dw/internal/platform"
)

func main() {
	code, cleanup := platform.CleanupExitCode()
	if !cleanup {
		code = bootstrap.Run(context.Background(), os.Args[1:], os.Stdin, os.Stdout, os.Stderr)
	}
	os.Exit(code)
}

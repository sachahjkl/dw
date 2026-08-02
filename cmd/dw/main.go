package main

import (
	"context"
	"os"
	"os/signal"

	"github.com/sachahjkl/dw/internal/bootstrap"
	"github.com/sachahjkl/dw/internal/platform"
)

func main() {
	code, cleanup := platform.CleanupExitCode()
	if !cleanup {
		ctx, stop := signal.NotifyContext(context.Background(), terminationSignals()...)
		defer stop()
		code = bootstrap.Run(ctx, os.Args[1:], os.Stdin, os.Stdout, os.Stderr)
	}
	os.Exit(code)
}

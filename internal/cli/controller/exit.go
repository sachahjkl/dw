package controller

import "github.com/sachahjkl/dw/internal/console"

// ExitCode applies the process contract without terminating the process. The
// executable entry point is the sole os.Exit boundary.
func ExitCode(err error) console.ExitCode {
	if err == nil || console.IsBrokenPipe(err) {
		return console.ExitSuccess
	}
	return console.ExitCodeFor(err)
}

//go:build windows

package main

import (
	"os"
	"syscall"

	"golang.org/x/sys/windows"
)

const utf8CodePage = 65001

func terminationSignals() []os.Signal {
	return []os.Signal{os.Interrupt, syscall.SIGTERM}
}

func configureConsole() func() {
	output, outputErr := windows.GetConsoleOutputCP()
	input, inputErr := windows.GetConsoleCP()
	if outputErr == nil && output != utf8CodePage {
		_ = windows.SetConsoleOutputCP(utf8CodePage)
	}
	if inputErr == nil && input != utf8CodePage {
		_ = windows.SetConsoleCP(utf8CodePage)
	}
	return func() {
		if outputErr == nil && output != utf8CodePage {
			_ = windows.SetConsoleOutputCP(output)
		}
		if inputErr == nil && input != utf8CodePage {
			_ = windows.SetConsoleCP(input)
		}
	}
}

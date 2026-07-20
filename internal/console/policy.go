package console

import (
	"errors"
	"io"
	"os"
	"strings"
	"syscall"
)

// ColorMode controls ANSI styling for human output. Machine output is always plain.
type ColorMode uint8

const (
	ColorAuto ColorMode = iota
	ColorAlways
	ColorNever
)

func ParseColorMode(value string) (ColorMode, bool) {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "auto", "":
		return ColorAuto, true
	case "always":
		return ColorAlways, true
	case "never":
		return ColorNever, true
	default:
		return ColorAuto, false
	}
}

// Streams makes stream routing and terminal detection explicit and testable.
type Streams struct {
	Stdin  io.Reader
	Stdout io.Writer
	Stderr io.Writer

	StdinTTY  bool
	StdoutTTY bool
	StderrTTY bool
}

func DetectStreams(stdin *os.File, stdout *os.File, stderr *os.File) Streams {
	return Streams{
		Stdin: stdin, Stdout: stdout, Stderr: stderr,
		StdinTTY: isTerminal(stdin), StdoutTTY: isTerminal(stdout), StderrTTY: isTerminal(stderr),
	}
}

func StandardStreams() Streams { return DetectStreams(os.Stdin, os.Stdout, os.Stderr) }

func isTerminal(file *os.File) bool {
	if file == nil {
		return false
	}
	info, err := file.Stat()
	return err == nil && info.Mode()&os.ModeCharDevice != 0
}

// Policy is the single decision point for terminal interaction, color and event routing.
type Policy struct {
	Streams    Streams
	Color      ColorMode
	NoColor    bool
	Machine    bool
	ShowEvents bool
}

func NewPolicy(streams Streams, mode ColorMode, lookupEnv func(string) (string, bool)) Policy {
	noColor := false
	if lookupEnv != nil {
		_, noColor = lookupEnv("NO_COLOR")
	}
	return Policy{Streams: streams, Color: mode, NoColor: noColor, ShowEvents: true}
}

func EnvironmentPolicy(mode ColorMode) Policy {
	return NewPolicy(StandardStreams(), mode, os.LookupEnv)
}

func (p Policy) WithMachine(machine bool) Policy {
	p.Machine = machine
	return p
}

func (p Policy) StdoutColor() bool     { return p.colorFor(p.Streams.StdoutTTY) }
func (p Policy) StderrColor() bool     { return p.colorFor(p.Streams.StderrTTY) }
func (p Policy) Interactive() bool     { return p.Streams.StdinTTY }
func (p Policy) ProgressEnabled() bool { return !p.Machine && p.Streams.StderrTTY }
func (p Policy) EventsEnabled() bool   { return !p.Machine && p.ShowEvents }

func (p Policy) colorFor(terminal bool) bool {
	if p.Machine {
		return false
	}
	switch p.Color {
	case ColorAlways:
		return true
	case ColorNever:
		return false
	default:
		return terminal && !p.NoColor
	}
}

// IsBrokenPipe identifies a closed output consumer. Entry points should treat it as success.
func IsBrokenPipe(err error) bool {
	return errors.Is(err, io.ErrClosedPipe) || errors.Is(err, syscall.EPIPE)
}

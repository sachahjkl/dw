package console

import (
	"errors"
	"io"

	"github.com/sachahjkl/dw/internal/l10n"
)

type OutputFormat uint8

const (
	FormatHuman OutputFormat = iota
	FormatJSON
	FormatTSV
	FormatMarkdown
	FormatHTML
	FormatRaw
)

func (f OutputFormat) Machine() bool {
	return f == FormatJSON || f == FormatTSV || f == FormatMarkdown || f == FormatHTML || f == FormatRaw
}

// Output is complete, already-rendered output. Body never includes ANSI for machine formats.
type Output struct {
	Format OutputFormat
	Body   []byte
}

func TextOutput(format OutputFormat, body string) Output {
	return Output{Format: format, Body: []byte(body)}
}

func (o Output) Empty() bool { return len(o.Body) == 0 }

// WriteOutput writes final action data to stdout and adds exactly one trailing newline.
func WriteOutput(writer io.Writer, output Output) error {
	if len(output.Body) == 0 {
		return nil
	}
	if _, err := writer.Write(output.Body); err != nil {
		return err
	}
	if output.Body[len(output.Body)-1] != '\n' {
		_, err := io.WriteString(writer, "\n")
		return err
	}
	return nil
}

func WriteDiagnostic(writer io.Writer, line string) error {
	if line == "" {
		return nil
	}
	_, err := io.WriteString(writer, line+"\n")
	return err
}

type ExitCode int

const (
	ExitSuccess ExitCode = 0
	ExitFailure ExitCode = 1
	ExitUsage   ExitCode = 2
)

type ExitCoder interface{ ExitCode() ExitCode }

// StatusError adds CLI status semantics without changing the underlying error text.
type StatusError struct {
	Code ExitCode
	Err  error
}

func (e StatusError) Error() string      { return e.Err.Error() }
func (e StatusError) Unwrap() error      { return e.Err }
func (e StatusError) ExitCode() ExitCode { return e.Code }

func WithExitCode(err error, code ExitCode) error {
	if err == nil {
		return nil
	}
	return StatusError{Code: code, Err: err}
}

func ExitCodeFor(err error) ExitCode {
	if err == nil || IsBrokenPipe(err) {
		return ExitSuccess
	}
	var coded ExitCoder
	if errors.As(err, &coded) {
		return coded.ExitCode()
	}
	return ExitFailure
}

type localizedError interface{ Localized() l10n.Message }

// LocalizedErrorText resolves the shared typed-error contract for any human
// diagnostic. Untyped/internal errors retain their original Error text.
func LocalizedErrorText(localizer Localizer, err error) string {
	if err == nil {
		return ""
	}
	localizer = WithConsoleMessages(localizer)
	var localized localizedError
	if errors.As(err, &localized) {
		return localizer.Render(localized.Localized())
	}
	return err.Error()
}

func ErrorLine(localizer Localizer, theme Theme, err error) string {
	if err == nil {
		return ""
	}
	localizer = WithConsoleMessages(localizer)
	detail := LocalizedErrorText(localizer, err)
	message := localize(localizer, "console.error.detail", l10n.A("label", localize(localizer, "console.error")), l10n.A("detail", detail))
	return theme.Failure(message)
}

func WriteErrorDiagnostic(writer io.Writer, localizer Localizer, theme Theme, err error) error {
	return WriteDiagnostic(writer, ErrorLine(localizer, theme, err))
}

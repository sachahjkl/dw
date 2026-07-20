// Package l10n is the only gateway for human-facing text. Machine tokens such
// as command names, JSON keys, and error codes must not be passed through it.
package l10n

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/sachahjkl/dw/locales"
)

// ID is a stable message identifier. It is a machine token and is never
// localized itself.
type ID string

// Arg is a named message substitution. A slice is used instead of a map so
// callers retain deterministic behavior when the same name is supplied twice.
type Arg struct {
	Name  string
	Value any
}

// A constructs a named substitution.
func A(name string, value any) Arg { return Arg{Name: name, Value: value} }

// Message is localized only at a presentation boundary.
type Message struct {
	ID   ID
	Args []Arg
}

// M constructs a message while preserving argument order.
func M(id ID, args ...Arg) Message { return Message{ID: id, Args: args} }

// Entry is one English catalog entry.
type Entry struct {
	ID   ID
	Text string
}

// Localizer is the presentation dependency used by CLI, TUI, and console
// packages. Text handles fixed labels; Render handles named substitutions.
type Localizer interface {
	Text(ID) string
	Render(Message) string
}

// Catalog is an immutable English message catalog and is safe for concurrent
// use. Extend returns a new catalog rather than mutating shared state.
type Catalog struct {
	messages map[ID]string
}

// NewCatalog validates and copies entries into an immutable catalog.
func NewCatalog(entries []Entry) (*Catalog, error) {
	messages := make(map[ID]string, len(entries))
	for _, entry := range entries {
		if entry.ID == "" {
			return nil, fmt.Errorf("l10n.empty-id")
		}
		if _, exists := messages[entry.ID]; exists {
			return nil, fmt.Errorf("l10n.duplicate-id:%s", entry.ID)
		}
		messages[entry.ID] = entry.Text
	}
	return &Catalog{messages: messages}, nil
}

// NewEnglish returns an independent catalog backed by the English messages
// embedded in the binary.
func NewEnglish() *Catalog {
	entries, err := parseEnglishTOML(locales.ActiveEnglishTOML)
	if err != nil {
		panic("l10n.invalid-embedded-catalog")
	}
	catalog, err := NewCatalog(entries)
	if err != nil {
		panic("l10n.invalid-embedded-catalog")
	}
	return catalog
}

// Extend returns a catalog containing the receiver followed by additional
// entries. Existing IDs cannot be replaced accidentally.
func (c *Catalog) Extend(entries ...Entry) (*Catalog, error) {
	messages := make(map[ID]string, len(c.messages)+len(entries))
	for id, text := range c.messages {
		messages[id] = text
	}
	for _, entry := range entries {
		if entry.ID == "" {
			return nil, fmt.Errorf("l10n.empty-id")
		}
		if _, exists := messages[entry.ID]; exists {
			return nil, fmt.Errorf("l10n.duplicate-id:%s", entry.ID)
		}
		messages[entry.ID] = entry.Text
	}
	return &Catalog{messages: messages}, nil
}

// Text resolves a fixed message. Missing coverage is a programming error;
// silently showing a message ID would leak a machine token into human output.
func (c *Catalog) Text(id ID) string {
	if text, ok := c.messages[id]; ok {
		return text
	}
	panic("l10n.missing-id:" + string(id))
}

// Has reports whether id is present without allocating a rendered string.
func (c *Catalog) Has(id ID) bool {
	_, ok := c.messages[id]
	return ok
}

// Render resolves a message and substitutes named placeholders. Message-valued
// arguments are localized recursively; ordinary values are inserted as data,
// so braces inside strings are never interpreted as more placeholders.
func (c *Catalog) Render(message Message) string {
	return c.render(message, 0)
}

func (c *Catalog) render(message Message, depth int) string {
	if depth >= 64 {
		panic("l10n.maximum-message-depth")
	}
	template := c.Text(message.ID)
	var rendered strings.Builder
	rendered.Grow(len(template))
	for len(template) > 0 {
		open := strings.IndexByte(template, '{')
		if open < 0 {
			rendered.WriteString(template)
			break
		}
		rendered.WriteString(template[:open])
		closeOffset := strings.IndexByte(template[open+1:], '}')
		if closeOffset < 0 {
			rendered.WriteString(template[open:])
			break
		}
		close := open + 1 + closeOffset
		name := template[open+1 : close]
		value, found := firstArg(message.Args, name)
		if !found {
			rendered.WriteString(template[open : close+1])
		} else if nested, ok := value.(Message); ok {
			rendered.WriteString(c.render(nested, depth+1))
		} else {
			fmt.Fprint(&rendered, value)
		}
		template = template[close+1:]
	}
	return rendered.String()
}

func firstArg(args []Arg, name string) (any, bool) {
	for _, arg := range args {
		if arg.Name == name {
			return arg.Value, true
		}
	}
	return nil, false
}

var english = NewEnglish()

// Text resolves a fixed message through the process-wide English gateway.
func Text(id ID) string { return english.Text(id) }

// Render resolves a parameterized message through the English gateway.
func Render(message Message) string { return english.Render(message) }

func parseEnglishTOML(source string) ([]Entry, error) {
	var entries []Entry
	inMessages := false
	for lineNumber, raw := range strings.Split(source, "\n") {
		line := strings.TrimSpace(raw)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		if strings.HasPrefix(line, "[") {
			inMessages = line == "[messages]"
			continue
		}
		if !inMessages {
			continue
		}
		key, value, ok := strings.Cut(line, "=")
		if !ok {
			return nil, fmt.Errorf("line %d: expected key = value", lineNumber+1)
		}
		decodedKey, err := strconv.Unquote(strings.TrimSpace(key))
		if err != nil {
			return nil, fmt.Errorf("line %d: invalid key: %w", lineNumber+1, err)
		}
		decodedValue, err := strconv.Unquote(strings.TrimSpace(value))
		if err != nil {
			return nil, fmt.Errorf("line %d: invalid value: %w", lineNumber+1, err)
		}
		entries = append(entries, Entry{ID: ID(decodedKey), Text: decodedValue})
	}
	return entries, nil
}

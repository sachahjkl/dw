package sqlserver

import (
	"strings"

	"github.com/sachahjkl/dw/internal/l10n"
)

var forbiddenTokens = [...]string{
	"insert", "update", "delete", "merge", "drop", "alter", "truncate", "exec", "execute",
	"create", "grant", "revoke", "into", "openquery", "openrowset", "opendatasource",
}

// GuardResult is the stable, machine-readable result of the compatibility SQL guard.
type GuardResult struct {
	IsAllowed bool    `json:"is_allowed"`
	Reason    *string `json:"reason"`
}

// ValidateReadOnlySQL preserves dw's intentionally conservative lexical guard. It is not a SQL
// parser; connection-level read-only intent remains mandatory as a second line of defence.
func ValidateReadOnlySQL(statement string) GuardResult {
	if strings.TrimSpace(statement) == "" {
		return blocked(l10n.Text("db.guard.empty"))
	}

	cleaned := strings.TrimSpace(sanitizeSQL(statement))
	if !startsWithReadOnlyVerb(cleaned) {
		return blocked(l10n.Text("db.guard.readonly_only"))
	}
	if hasMultipleTopLevelStatements(cleaned) {
		return blocked(l10n.Text("db.guard.multiple_statements"))
	}
	if firstSQLWord(cleaned) == "with" && !withResolvesToSelect(cleaned) {
		return blocked(l10n.Text("db.guard.readonly_only"))
	}

	lowered := strings.ToLower(cleaned)
	for _, token := range forbiddenTokens {
		if containsWord(lowered, token) {
			return blocked(l10n.Render(l10n.M("db.guard.forbidden", l10n.A("keyword", strings.ToUpper(token)))))
		}
	}
	if containsWordSequence(lowered, "next", "value", "for") {
		return blocked(l10n.Render(l10n.M("db.guard.forbidden", l10n.A("keyword", "NEXT VALUE FOR"))))
	}
	return GuardResult{IsAllowed: true}
}

func blocked(reason string) GuardResult {
	return GuardResult{Reason: &reason}
}

func startsWithReadOnlyVerb(statement string) bool {
	verb := firstSQLWord(statement)
	return verb == "select" || verb == "with"
}

func firstSQLWord(statement string) string {
	statement = strings.TrimSpace(statement)
	end := 0
	for end < len(statement) && isWordByte(statement[end]) {
		end++
	}
	return strings.ToLower(statement[:end])
}

func hasMultipleTopLevelStatements(statement string) bool {
	depth := 0
	lineStart := true
	seenMainSelect := false
	lastTopWord, previousTopWord := "", ""
	for index := 0; index < len(statement); {
		switch statement[index] {
		case '\n':
			lineStart = true
			index++
			continue
		case ' ', '\t', '\r':
			index++
			continue
		case '(':
			depth++
			lineStart = false
			index++
			continue
		case ')':
			if depth > 0 {
				depth--
			}
			lineStart = false
			index++
			continue
		case ';':
			if depth == 0 && strings.TrimSpace(statement[index+1:]) != "" {
				return true
			}
			lineStart = false
			index++
			continue
		}
		if !isWordByte(statement[index]) {
			lineStart = false
			index++
			continue
		}
		start := index
		for index < len(statement) && isWordByte(statement[index]) {
			index++
		}
		if depth != 0 {
			lineStart = false
			continue
		}
		word := strings.ToLower(statement[start:index])
		if word == "select" {
			setContinuation := lastTopWord == "union" || lastTopWord == "except" || lastTopWord == "intersect" ||
				(lastTopWord == "all" && (previousTopWord == "union" || previousTopWord == "except" || previousTopWord == "intersect"))
			if lineStart && seenMainSelect && !setContinuation {
				return true
			}
			seenMainSelect = true
		} else if lineStart && seenMainSelect && isStatementStartWord(word) {
			return true
		}
		previousTopWord, lastTopWord = lastTopWord, word
		lineStart = false
	}
	return false
}

func isStatementStartWord(word string) bool {
	switch word {
	case "add", "alter", "backup", "begin", "bind", "break", "bulk", "checkpoint", "close", "commit", "create",
		"dbcc", "deallocate", "declare", "delete", "deny", "disable", "drop", "dump", "enable",
		"exec", "execute", "goto", "grant", "if", "insert", "kill", "load", "merge", "move", "open", "print",
		"raiserror", "readtext", "receive", "reconfigure", "restore", "return", "revert", "revoke", "rollback",
		"save", "send", "set", "setuser", "shutdown", "throw", "truncate", "unbind", "update", "updatetext", "use", "waitfor",
		"while", "with", "writetext":
		return true
	default:
		return false
	}
}

func withResolvesToSelect(statement string) bool {
	depth := 0
	for index := 0; index < len(statement); {
		switch statement[index] {
		case '(':
			depth++
			index++
			continue
		case ')':
			if depth > 0 {
				depth--
			}
			index++
			continue
		}
		if !isWordByte(statement[index]) {
			index++
			continue
		}
		start := index
		for index < len(statement) && isWordByte(statement[index]) {
			index++
		}
		if depth != 0 {
			continue
		}
		word := strings.ToLower(statement[start:index])
		switch word {
		case "select":
			return true
		case "insert", "update", "delete", "merge":
			return false
		}
	}
	return false
}

func containsWord(value, token string) bool {
	for offset := 0; ; {
		index := strings.Index(value[offset:], token)
		if index < 0 {
			return false
		}
		index += offset
		if isBoundary(value, index) && isBoundary(value, index+len(token)) {
			return true
		}
		offset = index + len(token)
	}
}

func containsWordSequence(value string, sequence ...string) bool {
	matched := 0
	for index := 0; index < len(value); {
		for index < len(value) && !isWordByte(value[index]) {
			index++
		}
		start := index
		for index < len(value) && isWordByte(value[index]) {
			index++
		}
		if start == index {
			break
		}
		word := value[start:index]
		if word == sequence[matched] {
			matched++
			if matched == len(sequence) {
				return true
			}
		} else if word == sequence[0] {
			matched = 1
		} else {
			matched = 0
		}
	}
	return false
}

// isBoundary intentionally mirrors the Rust byte-oriented compatibility implementation, including
// its permissive "either side" check around the requested byte index.
func isBoundary(value string, index int) bool {
	if index == 0 || index >= len(value) {
		return true
	}
	current, previous := value[index], value[index-1]
	return (!isWordByte(current)) || (!isWordByte(previous))
}

func isWordByte(value byte) bool {
	return value == '_' || value >= 'a' && value <= 'z' || value >= 'A' && value <= 'Z' || value >= '0' && value <= '9'
}

func sanitizeSQL(statement string) string {
	const (
		lexSQL byte = iota
		lexSingleQuote
		lexDoubleQuote
		lexBracketIdentifier
		lexLineComment
		lexBlockComment
	)

	var output strings.Builder
	output.Grow(len(statement))
	state := lexSQL
	blockDepth := 0
	for index := 0; index < len(statement); {
		current := statement[index]
		next := byte(0)
		if index+1 < len(statement) {
			next = statement[index+1]
		}

		switch state {
		case lexSQL:
			switch {
			case current == '\'':
				state = lexSingleQuote
				output.WriteByte(' ')
				index++
			case current == '"':
				state = lexDoubleQuote
				output.WriteByte(' ')
				index++
			case current == '[':
				state = lexBracketIdentifier
				output.WriteByte(' ')
				index++
			case current == '-' && next == '-':
				state = lexLineComment
				output.WriteByte(' ')
				index += 2
			case current == '/' && next == '*':
				state = lexBlockComment
				blockDepth = 1
				output.WriteByte(' ')
				index += 2
			default:
				output.WriteByte(current)
				index++
			}
		case lexSingleQuote:
			if current == '\n' {
				output.WriteByte('\n')
			}
			index++
			if current == '\'' {
				if index < len(statement) && statement[index] == '\'' {
					index++
				} else {
					state = lexSQL
				}
			}
		case lexDoubleQuote:
			if current == '\n' {
				output.WriteByte('\n')
			}
			index++
			if current == '"' {
				if index < len(statement) && statement[index] == '"' {
					index++
				} else {
					state = lexSQL
				}
			}
		case lexBracketIdentifier:
			if current == '\n' {
				output.WriteByte('\n')
			}
			index++
			if current == ']' {
				if index < len(statement) && statement[index] == ']' {
					index++
				} else {
					state = lexSQL
				}
			}
		case lexLineComment:
			index++
			if current == '\n' {
				output.WriteByte('\n')
				state = lexSQL
			}
		case lexBlockComment:
			switch {
			case current == '/' && next == '*':
				blockDepth++
				index += 2
			case current == '*' && next == '/':
				blockDepth--
				index += 2
				if blockDepth == 0 {
					state = lexSQL
				}
			default:
				if current == '\n' {
					output.WriteByte('\n')
				}
				index++
			}
		}
	}
	return output.String()
}

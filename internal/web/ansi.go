package web

import (
	"strconv"
	"strings"
)

func ansiToSpans(value string) []ansiSpan {
	spans := make([]ansiSpan, 0)
	class := ""
	for len(value) != 0 {
		start := strings.Index(value, "\x1b[")
		if start < 0 {
			appendANSISpan(&spans, value, class)
			break
		}
		appendANSISpan(&spans, value[:start], class)
		value = value[start+2:]
		end := strings.IndexByte(value, 'm')
		if end < 0 {
			appendANSISpan(&spans, "\x1b["+value, class)
			break
		}
		class = ansiClass(value[:end], class)
		value = value[end+1:]
	}
	return spans
}

func appendANSISpan(spans *[]ansiSpan, text, class string) {
	if text == "" {
		return
	}
	if len(*spans) != 0 && (*spans)[len(*spans)-1].Class == class {
		(*spans)[len(*spans)-1].Text += text
		return
	}
	*spans = append(*spans, ansiSpan{Text: text, Class: class})
}

func ansiClass(sequence, current string) string {
	bold, color := strings.Contains(current, "ansi-bold"), ansiColorClass(current)
	if sequence == "" {
		return ""
	}
	for _, field := range strings.Split(sequence, ";") {
		code, err := strconv.Atoi(field)
		if err != nil {
			continue
		}
		switch {
		case code == 0:
			bold, color = false, ""
		case code == 1:
			bold = true
		case code == 22:
			bold = false
		case code == 39:
			color = ""
		case code >= 30 && code <= 37:
			color = "ansi-fg-" + strconv.Itoa(code-30)
		case code >= 90 && code <= 97:
			color = "ansi-fg-bright-" + strconv.Itoa(code-90)
		}
	}
	classes := make([]string, 0, 2)
	if bold {
		classes = append(classes, "ansi-bold")
	}
	if color != "" {
		classes = append(classes, color)
	}
	return strings.Join(classes, " ")
}

func ansiColorClass(class string) string {
	for _, value := range strings.Fields(class) {
		if strings.HasPrefix(value, "ansi-fg-") {
			return value
		}
	}
	return ""
}

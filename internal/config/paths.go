package config

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"
)

func DefaultRoot() string {
	return ResolvePlatformBaseDirs().DefaultRoot()
}

func UserConfigDirectory() string {
	return ResolvePlatformBaseDirs().UserConfigDirectory()
}

func UserSettingsPath() string {
	return filepath.Join(UserConfigDirectory(), "settings.json")
}

// ResolveRoot applies the Rust CLI's precedence: a nonblank command value,
// then the persisted setting, then the platform default. Resolution is lexical;
// the target need not exist and symlinks are not evaluated.
func ResolveRoot(explicitRoot string) string {
	if strings.TrimSpace(explicitRoot) != "" {
		return NormalizePathLossy(explicitRoot)
	}
	if settings := LoadUserSettings(); settings.Root != nil && strings.TrimSpace(*settings.Root) != "" {
		return NormalizePathLossy(*settings.Root)
	}
	return NormalizePathLossy(DefaultRoot())
}

// NormalizePath expands the leading Unix-style home marker and both Windows
// and shell environment-variable syntaxes, makes the path absolute, and
// lexically removes dot segments. It deliberately does not stat the path.
func NormalizePath(value string) (string, error) {
	expanded := expandEnvironmentVariables(expandHome(value))
	if !filepath.IsAbs(expanded) {
		current, err := os.Getwd()
		if err != nil {
			return "", err
		}
		expanded = appendPath(current, expanded)
	}
	return normalizePathComponents(expanded), nil
}

func NormalizePathLossy(value string) string {
	normalized, err := NormalizePath(value)
	if err == nil {
		return normalized
	}
	return expandHome(value)
}

func expandHome(value string) string {
	home := ResolvePlatformBaseDirs().HomeDir
	if value == "~" {
		return home
	}
	if strings.HasPrefix(value, "~/") {
		return appendPath(home, value[2:])
	}
	return value
}

func expandEnvironmentVariables(value string) string {
	return expandDollarEnvironmentVariables(expandPercentEnvironmentVariables(value))
}

func expandPercentEnvironmentVariables(value string) string {
	var output strings.Builder
	output.Grow(len(value))
	rest := value
	for {
		start := strings.IndexByte(rest, '%')
		if start < 0 {
			output.WriteString(rest)
			return output.String()
		}
		output.WriteString(rest[:start])
		afterStart := rest[start+1:]
		end := strings.IndexByte(afterStart, '%')
		if end < 0 {
			output.WriteByte('%')
			output.WriteString(afterStart)
			return output.String()
		}
		key := afterStart[:end]
		if replacement, ok := os.LookupEnv(key); ok {
			output.WriteString(replacement)
		} else {
			output.WriteByte('%')
			output.WriteString(key)
			output.WriteByte('%')
		}
		rest = afterStart[end+1:]
	}
}

func expandDollarEnvironmentVariables(value string) string {
	var output strings.Builder
	output.Grow(len(value))
	for index := 0; index < len(value); {
		if value[index] != '$' {
			output.WriteByte(value[index])
			index++
			continue
		}
		index++
		if index < len(value) && value[index] == '{' {
			index++
			start := index
			for index < len(value) && value[index] != '}' {
				index++
			}
			key := value[start:index]
			if index < len(value) {
				index++
			}
			if replacement, ok := os.LookupEnv(key); ok {
				output.WriteString(replacement)
			} else {
				output.WriteString("${")
				output.WriteString(key)
				output.WriteByte('}')
			}
			continue
		}
		start := index
		for index < len(value) {
			char := value[index]
			if char != '_' && (char < '0' || char > '9') && (char < 'a' || char > 'z') && (char < 'A' || char > 'Z') {
				break
			}
			index++
		}
		if start == index {
			output.WriteByte('$')
			continue
		}
		key := value[start:index]
		if replacement, ok := os.LookupEnv(key); ok {
			output.WriteString(replacement)
		} else {
			output.WriteByte('$')
			output.WriteString(key)
		}
	}
	return output.String()
}

func appendPath(base, child string) string {
	if base == "" {
		return child
	}
	if child == "" {
		return base
	}
	if isPathSeparator(base[len(base)-1]) {
		return base + child
	}
	return base + string(filepath.Separator) + child
}

func normalizePathComponents(path string) string {
	volume := filepath.VolumeName(path)
	rest := path[len(volume):]
	rooted := len(rest) != 0 && isPathSeparator(rest[0])
	parts := strings.FieldsFunc(rest, func(r rune) bool {
		return r == rune(filepath.Separator) || runtime.GOOS == "windows" && r == '/'
	})
	normalized := make([]string, 0, len(parts))
	for _, part := range parts {
		switch part {
		case ".":
			continue
		case "..":
			if len(normalized) != 0 && normalized[len(normalized)-1] != ".." {
				normalized = normalized[:len(normalized)-1]
			} else {
				normalized = append(normalized, part)
			}
		default:
			normalized = append(normalized, part)
		}
	}
	separator := string(filepath.Separator)
	prefix := volume
	if rooted {
		prefix += separator
	}
	joined := strings.Join(normalized, separator)
	if joined == "" {
		return prefix
	}
	if prefix == "" || isPathSeparator(prefix[len(prefix)-1]) {
		return prefix + joined
	}
	return prefix + separator + joined
}

func isPathSeparator(value byte) bool {
	return value == byte(filepath.Separator) || runtime.GOOS == "windows" && value == '/'
}

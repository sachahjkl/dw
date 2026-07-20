//go:build !windows

package secret

func validatePlatformValue(string) error { return nil }

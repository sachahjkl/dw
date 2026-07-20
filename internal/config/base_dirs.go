package config

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"
)

// PlatformBaseDirs are the platform user directories used by dw. Empty optional
// fields mean the operating system does not define a corresponding directory.
type PlatformBaseDirs struct {
	HomeDir       string `json:"homeDir"`
	CacheDir      string `json:"cacheDir,omitempty"`
	ConfigDir     string `json:"configDir,omitempty"`
	DataDir       string `json:"dataDir,omitempty"`
	DataLocalDir  string `json:"dataLocalDir,omitempty"`
	ExecutableDir string `json:"executableDir,omitempty"`
	PreferenceDir string `json:"preferenceDir,omitempty"`
	RuntimeDir    string `json:"runtimeDir,omitempty"`
	StateDir      string `json:"stateDir,omitempty"`
}

// ResolvePlatformBaseDirs resolves base directories without touching the file
// system. Environment values containing only whitespace are treated as unset.
func ResolvePlatformBaseDirs() PlatformBaseDirs {
	home := resolveHomeDir()
	if runtime.GOOS == "windows" {
		local := firstEnvironment("LOCALAPPDATA")
		roaming := firstEnvironment("APPDATA", "LOCALAPPDATA")
		return PlatformBaseDirs{
			HomeDir:       home,
			CacheDir:      local,
			ConfigDir:     roaming,
			DataDir:       roaming,
			DataLocalDir:  local,
			PreferenceDir: roaming,
		}
	}

	config := firstEnvironment("XDG_CONFIG_HOME")
	if config == "" {
		config = filepath.Join(home, ".config")
	}
	cache := firstEnvironment("XDG_CACHE_HOME")
	if cache == "" {
		cache = filepath.Join(home, ".cache")
	}
	data := firstEnvironment("XDG_DATA_HOME")
	if data == "" {
		data = filepath.Join(home, ".local", "share")
	}
	state := firstEnvironment("XDG_STATE_HOME")
	if state == "" {
		state = filepath.Join(home, ".local", "state")
	}
	executable := firstEnvironment("XDG_BIN_HOME")
	if executable == "" {
		executable = filepath.Join(filepath.Dir(data), "bin")
	}
	runtimeDir := firstEnvironment("XDG_RUNTIME_DIR")
	return PlatformBaseDirs{
		HomeDir:       home,
		CacheDir:      cache,
		ConfigDir:     config,
		DataDir:       data,
		DataLocalDir:  data,
		ExecutableDir: executable,
		PreferenceDir: config,
		RuntimeDir:    runtimeDir,
		StateDir:      state,
	}
}

func (dirs PlatformBaseDirs) DefaultRoot() string {
	return filepath.Join(dirs.HomeDir, "dev", "dw")
}

func (dirs PlatformBaseDirs) UserConfigDirectory() string {
	if runtime.GOOS == "windows" {
		base := dirs.DataLocalDir
		if base == "" {
			base = dirs.ConfigDir
		}
		if base == "" {
			base = dirs.HomeDir
		}
		return filepath.Join(base, "DevWorkflow")
	}
	base := dirs.ConfigDir
	if base == "" {
		base = filepath.Join(dirs.HomeDir, ".config")
	}
	return filepath.Join(base, "DevWorkflow")
}

func resolveHomeDir() string {
	if runtime.GOOS == "windows" {
		if home := firstEnvironment("USERPROFILE"); home != "" {
			return home
		}
		drive, path := environment("HOMEDRIVE"), environment("HOMEPATH")
		if drive != "" && path != "" {
			return drive + path
		}
		if home := firstEnvironment("HOME"); home != "" {
			return home
		}
		return "~"
	}
	if home := firstEnvironment("HOME", "USERPROFILE"); home != "" {
		return home
	}
	drive, path := environment("HOMEDRIVE"), environment("HOMEPATH")
	if drive != "" && path != "" {
		return drive + path
	}
	return "~"
}

func firstEnvironment(keys ...string) string {
	for _, key := range keys {
		if value := environment(key); value != "" {
			return value
		}
	}
	return ""
}

func environment(key string) string {
	return strings.TrimSpace(os.Getenv(key))
}

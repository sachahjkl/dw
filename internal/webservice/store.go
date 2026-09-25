package webservice

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"runtime"

	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/fsutil"
)

type Paths struct {
	ConfigFile       string
	LegacyConfigFile string
	StateFile        string
}

type Store struct{ paths Paths }

func ResolvePaths(dirs config.PlatformBaseDirs) Paths {
	configFile := filepath.Join(dirs.UserConfigDirectory(), "web.json")
	var runtimeDirectory, legacyConfigFile string
	if runtime.GOOS == "windows" {
		if dirs.ConfigDir != "" {
			if legacy := filepath.Join(dirs.ConfigDir, "DevWorkflow", "web.json"); legacy != configFile {
				legacyConfigFile = legacy
			}
		}
		base := dirs.DataLocalDir
		if base == "" {
			base = dirs.HomeDir
		}
		runtimeDirectory = filepath.Join(base, "DevWorkflow", "web")
	} else if dirs.RuntimeDir != "" {
		runtimeDirectory = filepath.Join(dirs.RuntimeDir, "devworkflow", "web")
	} else {
		base := dirs.StateDir
		if base == "" {
			base = dirs.HomeDir
		}
		runtimeDirectory = filepath.Join(base, "DevWorkflow", "web")
	}
	return Paths{ConfigFile: configFile, LegacyConfigFile: legacyConfigFile, StateFile: filepath.Join(runtimeDirectory, "state.json")}
}

func (store *Store) migrateLegacyConfig() error {
	legacy := store.paths.LegacyConfigFile
	if legacy == "" {
		return nil
	}
	if _, err := os.Stat(store.paths.ConfigFile); !errors.Is(err, os.ErrNotExist) {
		return nil
	}
	content, err := os.ReadFile(legacy)
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	if err != nil {
		return err
	}
	if err = writeAtomic(store.paths.ConfigFile, content); err != nil {
		return err
	}
	return os.Remove(legacy)
}

func NewStore(dirs config.PlatformBaseDirs) *Store { return &Store{paths: ResolvePaths(dirs)} }
func (store *Store) Paths() Paths                  { return store.paths }

func (store *Store) LoadConfig() (WebConfigV1, error) {
	if err := store.migrateLegacyConfig(); err != nil {
		return WebConfigV1{}, err
	}
	content, err := os.ReadFile(store.paths.ConfigFile)
	if err != nil {
		return WebConfigV1{}, err
	}
	var value WebConfigV1
	if err = decodeStrict(content, &value); err != nil {
		return WebConfigV1{}, fmt.Errorf("web.invalid-config:%w", err)
	}
	if err = value.Validate(); err != nil {
		return WebConfigV1{}, err
	}
	return value, nil
}

func (store *Store) SaveConfig(value WebConfigV1) error {
	if err := value.Validate(); err != nil {
		return err
	}
	return writeAtomicJSON(store.paths.ConfigFile, value)
}

func (store *Store) LoadState() (WebStateV1, error) {
	content, err := os.ReadFile(store.paths.StateFile)
	if err != nil {
		return WebStateV1{}, err
	}
	var value WebStateV1
	if err = decodeStrict(content, &value); err != nil {
		return WebStateV1{}, fmt.Errorf("web.invalid-state:%w", err)
	}
	if err = value.Validate(); err != nil {
		return WebStateV1{}, err
	}
	return value, nil
}

func (store *Store) SaveState(value WebStateV1) error {
	if err := value.Validate(); err != nil {
		return err
	}
	return writeAtomicJSON(store.paths.StateFile, value)
}

func (store *Store) RemoveState() error {
	err := os.Remove(store.paths.StateFile)
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	return err
}

func (store *Store) EnsureConfig(root string, port uint16, executable string) (WebConfigV1, error) {
	current, err := store.LoadConfig()
	if err == nil {
		current.Root = config.ResolveRoot(root)
		current.Port = port
		current.Executable = executable
		return current, store.SaveConfig(current)
	}
	if !errors.Is(err, os.ErrNotExist) {
		return WebConfigV1{}, err
	}
	secret, err := NewServiceSecret()
	if err != nil {
		return WebConfigV1{}, err
	}
	value := WebConfigV1{Schema: SchemaV1, Root: config.ResolveRoot(root), Port: port, Executable: executable, Registration: RegistrationNone, ServiceSecret: secret}
	return value, store.SaveConfig(value)
}

type persistedJSON interface {
	WebConfigV1 | WebStateV1
}

func writeAtomicJSON[T persistedJSON](path string, value T) error {
	content, err := json.Marshal(value)
	if err != nil {
		return err
	}
	return writeAtomic(path, content)
}

func writeAtomic(path string, content []byte) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		return err
	}
	return fsutil.WriteFileAtomic(path, content, 0o600)
}

func decodeStrict[T persistedJSON](content []byte, target *T) error {
	decoder := json.NewDecoder(bytes.NewReader(content))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(target); err != nil {
		return err
	}
	if err := decoder.Decode(&struct{}{}); !errors.Is(err, io.EOF) {
		return fmt.Errorf("trailing JSON")
	}
	return nil
}

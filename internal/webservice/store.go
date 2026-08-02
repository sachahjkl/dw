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
)

type Paths struct {
	ConfigFile string
	StateFile  string
}

type Store struct{ paths Paths }

func ResolvePaths(dirs config.PlatformBaseDirs) Paths {
	configFile := filepath.Join(dirs.UserConfigDirectory(), "web.json")
	var runtimeDirectory string
	if runtime.GOOS == "windows" {
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
	return Paths{ConfigFile: configFile, StateFile: filepath.Join(runtimeDirectory, "state.json")}
}

func NewStore(dirs config.PlatformBaseDirs) *Store { return &Store{paths: ResolvePaths(dirs)} }
func (store *Store) Paths() Paths                  { return store.paths }

func (store *Store) LoadConfig() (WebConfigV1, error) {
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
	file, err := os.CreateTemp(filepath.Dir(path), ".dw-web-*")
	if err != nil {
		return err
	}
	name := file.Name()
	keep := false
	defer func() {
		_ = file.Close()
		if !keep {
			_ = os.Remove(name)
		}
	}()
	if err = file.Chmod(0o600); err != nil {
		return err
	}
	if _, err = file.Write(content); err != nil {
		return err
	}
	if err = file.Sync(); err != nil {
		return err
	}
	if err = file.Close(); err != nil {
		return err
	}
	if err = os.Rename(name, path); err != nil {
		return err
	}
	keep = true
	return nil
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

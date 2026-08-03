package runtimeconfig

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/sachahjkl/dw/internal/config"
)

func TestLoadCreatesEditableDefaultsAndReloadsChanges(t *testing.T) {
	directory := t.TempDir()
	dirs := config.PlatformBaseDirs{HomeDir: directory, ConfigDir: filepath.Join(directory, "config")}

	created, err := Load(dirs)
	if err != nil {
		t.Fatal(err)
	}
	if created != Default() {
		t.Fatalf("created config = %#v", created)
	}
	path := Path(dirs)
	content, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(content), `"leaseRenewMilliseconds": 5000`) {
		t.Fatalf("runtime config is not readable JSON:\n%s", content)
	}

	created.Execution.MaxEvents = 321
	updated, err := json.MarshalIndent(created, "", "  ")
	if err != nil {
		t.Fatal(err)
	}
	if err = os.WriteFile(path, updated, 0o644); err != nil {
		t.Fatal(err)
	}
	loaded, err := Load(dirs)
	if err != nil {
		t.Fatal(err)
	}
	if loaded.Execution.MaxEvents != 321 {
		t.Fatalf("max events = %d", loaded.Execution.MaxEvents)
	}
}

func TestLoadRejectsUnknownAndInvalidValues(t *testing.T) {
	tests := []struct {
		name   string
		mutate func(map[string]any)
	}{
		{"unknown field", func(value map[string]any) { value["unknown"] = true }},
		{"short lease", func(value map[string]any) {
			execution := value["execution"].(map[string]any)
			execution["leaseDurationMilliseconds"] = execution["leaseRenewMilliseconds"]
		}},
		{"zero port", func(value map[string]any) { value["webService"].(map[string]any)["defaultPort"] = float64(0) }},
		{"zero action response timeout", func(value map[string]any) {
			value["web"].(map[string]any)["actionResponseTimeoutMilliseconds"] = float64(0)
		}},
		{"browser timer overflow", func(value map[string]any) {
			value["web"].(map[string]any)["notificationTTLMilliseconds"] = float64(MaxBrowserTimerMilliseconds + 1)
		}},
		{"missing existing field", func(value map[string]any) {
			delete(value["web"].(map[string]any), "maxHeaderBytes")
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			directory := t.TempDir()
			dirs := config.PlatformBaseDirs{HomeDir: directory, ConfigDir: filepath.Join(directory, "config")}
			encoded, err := json.Marshal(Default())
			if err != nil {
				t.Fatal(err)
			}
			var value map[string]any
			if err = json.Unmarshal(encoded, &value); err != nil {
				t.Fatal(err)
			}
			test.mutate(value)
			encoded, err = json.Marshal(value)
			if err != nil {
				t.Fatal(err)
			}
			if err = os.MkdirAll(filepath.Dir(Path(dirs)), 0o755); err != nil {
				t.Fatal(err)
			}
			if err = os.WriteFile(Path(dirs), encoded, 0o644); err != nil {
				t.Fatal(err)
			}
			if _, err = Load(dirs); err == nil {
				t.Fatal("invalid runtime config was accepted")
			}
		})
	}
}

func TestLoadAddsNewWebLimitsToPersistedConfiguration(t *testing.T) {
	directory := t.TempDir()
	dirs := config.PlatformBaseDirs{HomeDir: directory, ConfigDir: filepath.Join(directory, "config")}
	encoded, err := json.Marshal(Default())
	if err != nil {
		t.Fatal(err)
	}
	var value map[string]any
	if err = json.Unmarshal(encoded, &value); err != nil {
		t.Fatal(err)
	}
	web := value["web"].(map[string]any)
	delete(web, "actionResponseTimeoutMilliseconds")
	delete(web, "notificationTTLMilliseconds")
	encoded, err = json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	if err = os.MkdirAll(filepath.Dir(Path(dirs)), 0o755); err != nil {
		t.Fatal(err)
	}
	if err = os.WriteFile(Path(dirs), encoded, 0o644); err != nil {
		t.Fatal(err)
	}
	loaded, err := Load(dirs)
	if err != nil {
		t.Fatal(err)
	}
	defaults := Default().Web
	if loaded.Web.ActionResponseTimeoutMilliseconds != defaults.ActionResponseTimeoutMilliseconds || loaded.Web.NotificationTTLMilliseconds != defaults.NotificationTTLMilliseconds {
		t.Fatalf("loaded web limits = %#v", loaded.Web)
	}
	updated, err := os.ReadFile(Path(dirs))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(updated), `"actionResponseTimeoutMilliseconds": 15000`) || !strings.Contains(string(updated), `"notificationTTLMilliseconds": 8000`) {
		t.Fatalf("persisted web limits were not normalized:\n%s", updated)
	}
}

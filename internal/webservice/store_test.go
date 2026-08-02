package webservice

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
	"time"

	"github.com/sachahjkl/dw/internal/config"
)

func TestIdentifiersRoundTripBase64URL(t *testing.T) {
	secret, err := NewServiceSecret()
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(secret.String(), "=") || len(secret.String()) != 43 {
		t.Fatalf("service secret encoding = %q", secret.String())
	}
	parsedSecret, err := ParseServiceSecret(secret.String())
	if err != nil || parsedSecret != secret {
		t.Fatalf("service secret round trip = %#v, %v", parsedSecret, err)
	}
	serverID, err := NewServerID()
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(serverID.String(), "=") || len(serverID.String()) != 22 {
		t.Fatalf("server ID encoding = %q", serverID.String())
	}
	parsedID, err := ParseServerID(serverID.String())
	if err != nil || parsedID != serverID {
		t.Fatalf("server ID round trip = %#v, %v", parsedID, err)
	}
}

func TestStorePreservesSecretAndExplicitPortZero(t *testing.T) {
	base := t.TempDir()
	dirs := config.PlatformBaseDirs{HomeDir: base, ConfigDir: filepath.Join(base, "config"), StateDir: filepath.Join(base, "state"), RuntimeDir: filepath.Join(base, "runtime"), DataLocalDir: filepath.Join(base, "local")}
	store := NewStore(dirs)
	first, err := store.EnsureConfig(filepath.Join(base, "root-a"), DefaultPort, filepath.Join(base, "dw"))
	if err != nil {
		t.Fatal(err)
	}
	second, err := store.EnsureConfig(filepath.Join(base, "root-b"), 0, filepath.Join(base, "dw"))
	if err != nil {
		t.Fatal(err)
	}
	if first.ServiceSecret != second.ServiceSecret || second.Port != 0 || second.Root != filepath.Join(base, "root-b") {
		t.Fatalf("updated config = %#v", second)
	}
	loaded, err := store.LoadConfig()
	if err != nil || loaded != second {
		t.Fatalf("loaded config = %#v, %v", loaded, err)
	}
	if runtime.GOOS != "windows" {
		info, statErr := os.Stat(store.Paths().ConfigFile)
		if statErr != nil {
			t.Fatal(statErr)
		}
		if info.Mode().Perm() != 0o600 {
			t.Fatalf("config mode = %v", info.Mode().Perm())
		}
	}
}

func TestStateRejectsNonLoopbackAddress(t *testing.T) {
	serverID, err := NewServerID()
	if err != nil {
		t.Fatal(err)
	}
	state := WebStateV1{Schema: SchemaV1, ServerID: serverID, PID: 1, Address: "192.0.2.1:7331", StartedAt: time.Now(), Executable: "/tmp/dw"}
	if err = state.Validate(); err == nil {
		t.Fatal("non-loopback state was accepted")
	}
}

func TestStoreRejectsUnknownConfigurationFields(t *testing.T) {
	base := t.TempDir()
	store := NewStore(config.PlatformBaseDirs{HomeDir: base, ConfigDir: base, StateDir: base})
	if err := os.MkdirAll(filepath.Dir(store.Paths().ConfigFile), 0o700); err != nil {
		t.Fatal(err)
	}
	content := `{"schema":1,"root":"/tmp/root","port":7331,"executable":"/tmp/dw","registration":"none","serviceSecret":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","unknown":true}`
	if err := os.WriteFile(store.Paths().ConfigFile, []byte(content), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := store.LoadConfig(); err == nil {
		t.Fatal("unknown config field was accepted")
	}
}

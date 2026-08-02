package webservice

import (
	"context"
	"fmt"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"strings"
	"sync/atomic"
	"testing"
	"time"

	"github.com/sachahjkl/dw/internal/config"
)

func TestManagerOpenAcceptsCompleteTicketResponse(t *testing.T) {
	directory := t.TempDir()
	dirs := config.PlatformBaseDirs{
		HomeDir:    directory,
		ConfigDir:  filepath.Join(directory, "config"),
		StateDir:   filepath.Join(directory, "state"),
		RuntimeDir: filepath.Join(directory, "run"),
	}
	executable := filepath.Join(directory, "dw")
	manager, err := NewManager(dirs, executable)
	if err != nil {
		t.Fatal(err)
	}
	configValue, err := manager.store.EnsureConfig(directory, DefaultPort, executable)
	if err != nil {
		t.Fatal(err)
	}
	serverID, err := NewServerID()
	if err != nil {
		t.Fatal(err)
	}
	server := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		if request.Header.Get("Authorization") != "Bearer "+configValue.ServiceSecret.String() {
			http.Error(writer, "unauthorized", http.StatusUnauthorized)
			return
		}
		switch request.URL.Path {
		case "/healthz":
			writer.WriteHeader(http.StatusOK)
		case "/admin/tickets":
			writer.Header().Set("Content-Type", "application/json")
			writer.WriteHeader(http.StatusCreated)
			_, _ = fmt.Fprintf(writer, `{"schema":1,"ticket":"ticket-value","expiresAt":%q}`, time.Now().Add(time.Minute).UTC().Format(time.RFC3339Nano))
		default:
			http.NotFound(writer, request)
		}
	}))
	defer server.Close()
	address := strings.TrimPrefix(server.URL, "http://")
	if err = manager.store.SaveState(WebStateV1{
		Schema: SchemaV1, ServerID: serverID, PID: 1, Address: address,
		StartedAt: time.Now().UTC(), Executable: executable,
	}); err != nil {
		t.Fatal(err)
	}
	var opened string
	manager.launch = func(location string) error {
		opened = location
		return nil
	}
	manager.client = server.Client()

	result, err := manager.Open(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if result.Location != "http://"+address+"/?ticket=ticket-value" {
		t.Fatalf("ticket URL = %q", result.Location)
	}
	if !result.Opened {
		t.Fatal("browser was not marked as opened")
	}
	if opened != result.Location {
		t.Fatalf("opened URL = %q", opened)
	}

	manager.launch = func(string) error { return fmt.Errorf("browser unavailable") }
	result, err = manager.Open(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if result.Location != "http://"+address+"/?ticket=ticket-value" || result.Opened {
		t.Fatalf("headless result = %#v", result)
	}
}

func TestManagerStatusReportsConfiguredAddressWhileStopped(t *testing.T) {
	directory := t.TempDir()
	dirs := config.PlatformBaseDirs{
		HomeDir:    directory,
		ConfigDir:  filepath.Join(directory, "config"),
		StateDir:   filepath.Join(directory, "state"),
		RuntimeDir: filepath.Join(directory, "run"),
	}
	executable := filepath.Join(directory, "dw")
	manager, err := NewManager(dirs, executable)
	if err != nil {
		t.Fatal(err)
	}
	if _, err = manager.store.EnsureConfig(directory, 3050, executable); err != nil {
		t.Fatal(err)
	}

	status, err := manager.Status(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if status.Running || status.Address != "127.0.0.1:3050" {
		t.Fatalf("stopped status = %#v", status)
	}
}

func TestManagerStartStopsPreviousExecutableBeforeReplacement(t *testing.T) {
	directory := t.TempDir()
	dirs := config.PlatformBaseDirs{
		HomeDir:    directory,
		ConfigDir:  filepath.Join(directory, "config"),
		StateDir:   filepath.Join(directory, "state"),
		RuntimeDir: filepath.Join(directory, "run"),
	}
	oldExecutable := filepath.Join(directory, "dw-old")
	newExecutable := filepath.Join(directory, "dw-new")
	manager, err := NewManager(dirs, newExecutable)
	if err != nil {
		t.Fatal(err)
	}
	configValue, err := manager.store.EnsureConfig(directory, 7331, oldExecutable)
	if err != nil {
		t.Fatal(err)
	}
	serverID, err := NewServerID()
	if err != nil {
		t.Fatal(err)
	}
	var shutdownCalled atomic.Bool
	oldServer := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		if request.Header.Get("Authorization") != "Bearer "+configValue.ServiceSecret.String() {
			http.Error(writer, "unauthorized", http.StatusUnauthorized)
			return
		}
		switch request.URL.Path {
		case "/healthz":
			writer.WriteHeader(http.StatusOK)
		case "/admin/shutdown":
			shutdownCalled.Store(true)
			if removeErr := manager.store.RemoveState(); removeErr != nil {
				http.Error(writer, removeErr.Error(), http.StatusInternalServerError)
				return
			}
			writer.WriteHeader(http.StatusNoContent)
		default:
			http.NotFound(writer, request)
		}
	}))
	defer oldServer.Close()
	if err = manager.store.SaveState(WebStateV1{
		Schema: SchemaV1, ServerID: serverID, PID: 1,
		Address:   strings.TrimPrefix(oldServer.URL, "http://"),
		StartedAt: time.Now().UTC(), Executable: oldExecutable,
	}); err != nil {
		t.Fatal(err)
	}
	manager.client = oldServer.Client()

	var newServer *httptest.Server
	t.Cleanup(func() {
		if newServer != nil {
			newServer.Close()
		}
	})
	manager.start = func(current WebConfigV1) error {
		newServer = httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
			if request.Header.Get("Authorization") != "Bearer "+current.ServiceSecret.String() {
				http.Error(writer, "unauthorized", http.StatusUnauthorized)
				return
			}
			if request.URL.Path != "/healthz" {
				http.NotFound(writer, request)
				return
			}
			writer.WriteHeader(http.StatusOK)
		}))
		newServerID, createErr := NewServerID()
		if createErr != nil {
			return createErr
		}
		return manager.store.SaveState(WebStateV1{
			Schema: SchemaV1, ServerID: newServerID, PID: 2,
			Address:   strings.TrimPrefix(newServer.URL, "http://"),
			StartedAt: time.Now().UTC(), Executable: current.Executable,
		})
	}

	result, err := manager.Start(context.Background(), StartOptions{NoOpen: true})
	if err != nil {
		t.Fatal(err)
	}
	if !shutdownCalled.Load() {
		t.Fatal("previous executable was not stopped")
	}
	if !result.Status.Running || result.Status.Executable != newExecutable || result.Status.PID != 2 {
		t.Fatalf("replacement status = %#v", result.Status)
	}
}

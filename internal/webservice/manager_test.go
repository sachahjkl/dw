package webservice

import (
	"context"
	"fmt"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"strings"
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

	if err = manager.Open(context.Background()); err != nil {
		t.Fatal(err)
	}
	if opened != "http://"+address+"/?ticket=ticket-value" {
		t.Fatalf("opened URL = %q", opened)
	}
}

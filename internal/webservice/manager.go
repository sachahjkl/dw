package webservice

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"net"
	"net/http"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"time"

	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/runtimeconfig"
)

type StartOptions struct {
	Root            *string
	Port            *uint16
	Open            bool
	NoExpiry        bool
	Unauthenticated bool
	Token           *string
	preserveAuth    bool
}

type OpenOptions struct {
	NoExpiry bool
	Token    *string
}

type RegisterOptions struct {
	Root *string
	Port *uint16
}

type NativeManager interface {
	Registration() Registration
	Register(context.Context, string) error
	Unregister(context.Context) error
	Start(context.Context) error
	Restart(context.Context) error
	Stop(context.Context) error
	Running(context.Context) (bool, error)
}

type Manager struct {
	store      *Store
	native     NativeManager
	executable string
	client     *http.Client
	now        func() time.Time
	settings   runtimeconfig.WebService
	start      func(WebConfigV1) error
	launch     func(string) error
}

func NewManager(dirs config.PlatformBaseDirs, executable string) (*Manager, error) {
	return NewManagerWithSettings(dirs, executable, runtimeconfig.Default().WebService)
}

func NewManagerWithSettings(dirs config.PlatformBaseDirs, executable string, settings runtimeconfig.WebService) (*Manager, error) {
	if err := runtimeconfig.ValidateWebService(settings); err != nil {
		return nil, err
	}
	if strings.TrimSpace(executable) == "" {
		return nil, fmt.Errorf("web.executable-required")
	}
	absolute, err := filepathAbs(executable)
	if err != nil {
		return nil, err
	}
	native, err := newNativeManager(dirs)
	if err != nil {
		return nil, err
	}
	manager := &Manager{
		store: NewStore(dirs), native: native, executable: absolute, settings: settings,
		client: &http.Client{Timeout: runtimeconfig.Milliseconds(settings.HTTPClientTimeoutMilliseconds)}, now: time.Now, launch: openBrowser,
	}
	manager.start = manager.launchLocal
	return manager, nil
}

func (manager *Manager) Store() *Store { return manager.store }

func (manager *Manager) Start(ctx context.Context, options StartOptions) (StartResult, error) {
	if options.NoExpiry && !options.Open {
		return StartResult{}, localizedError("web.error.no-expiry-requires-open")
	}
	previous, previousErr := manager.store.LoadConfig()
	if previousErr != nil && !errors.Is(previousErr, os.ErrNotExist) {
		return StartResult{}, previousErr
	}
	authMode, accessTokenDigest, err := resolveStartAuth(previous, previousErr == nil, options)
	if err != nil {
		return StartResult{}, err
	}
	if options.NoExpiry && authMode != AuthTicket {
		return StartResult{}, localizedError("web.error.authentication-option-unavailable")
	}
	if previousErr == nil && previous.Registration == RegistrationNone {
		resolvedRoot := previous.Root
		resolvedPort := previous.Port
		if options.Root != nil {
			resolvedRoot = config.ResolveRoot(*options.Root)
		}
		if options.Port != nil {
			resolvedPort = *options.Port
		}
		if previous.Root != resolvedRoot || previous.Port != resolvedPort || previous.Executable != manager.executable || previous.EffectiveAuthMode() != authMode || previous.AccessTokenDigest != accessTokenDigest {
			if err := manager.Stop(ctx); err != nil {
				return StartResult{}, err
			}
		}
	}
	current, err := manager.loadOrCreateConfig(options.Root, options.Port)
	if err != nil {
		return StartResult{}, err
	}
	current.AuthMode = authMode
	current.AccessTokenDigest = accessTokenDigest
	if err = manager.store.SaveConfig(current); err != nil {
		return StartResult{}, err
	}
	changed := previousErr != nil ||
		previous.Root != current.Root ||
		previous.Port != current.Port ||
		previous.Executable != current.Executable ||
		previous.EffectiveAuthMode() != current.EffectiveAuthMode() ||
		previous.AccessTokenDigest != current.AccessTokenDigest
	status, err := manager.Status(ctx)
	if err != nil {
		return StartResult{}, err
	}
	if status.Running && !changed {
		return manager.completeStart(ctx, status, options)
	}
	if current.Registration != RegistrationNone {
		if current.Registration != manager.native.Registration() {
			return StartResult{}, fmt.Errorf("web.registration-platform-mismatch:%s", current.Registration)
		}
		if changed || status.Stale {
			err = manager.native.Restart(ctx)
		} else {
			err = manager.native.Start(ctx)
		}
		if err != nil {
			return StartResult{}, err
		}
	} else {
		if status.Running {
			if err = manager.Stop(ctx); err != nil {
				return StartResult{}, err
			}
		} else if status.Stale {
			if err = manager.store.RemoveState(); err != nil {
				return StartResult{}, err
			}
		}
		if err = manager.start(current); err != nil {
			return StartResult{}, err
		}
	}
	status, err = manager.waitForStatus(ctx, true)
	if err != nil {
		return StartResult{}, err
	}
	return manager.completeStart(ctx, status, options)
}

func (manager *Manager) launchLocal(current WebConfigV1) error {
	address := net.JoinHostPort("127.0.0.1", strconv.Itoa(int(current.Port)))
	listener, err := net.Listen("tcp", address)
	if err != nil {
		return l10n.WrapError(err, "web.error.port-unavailable", l10n.A("address", address))
	}
	_ = listener.Close()
	command := exec.Command(manager.executable, "web", "serve")
	command.Stdin, command.Stdout, command.Stderr = nil, nil, nil
	if runtime.GOOS == "windows" {
		command.SysProcAttr = detachedProcessAttributes()
	}
	if err = command.Start(); err != nil {
		return err
	}
	return command.Process.Release()
}

func (manager *Manager) completeStart(ctx context.Context, status StatusV1, options StartOptions) (StartResult, error) {
	result := StartResult{Status: status}
	if !options.Open {
		return result, nil
	}
	opened, err := manager.Open(ctx, OpenOptions{NoExpiry: options.NoExpiry, Token: options.Token})
	result.Open = &opened
	return result, err
}

func (manager *Manager) Stop(ctx context.Context) error {
	configValue, err := manager.store.LoadConfig()
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return nil
		}
		return err
	}
	if configValue.Registration != RegistrationNone {
		if configValue.Registration != manager.native.Registration() {
			return fmt.Errorf("web.registration-platform-mismatch:%s", configValue.Registration)
		}
		if err = manager.native.Stop(ctx); err != nil {
			return err
		}
		return manager.store.RemoveState()
	}
	state, err := manager.store.LoadState()
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	if err != nil {
		return err
	}
	body, err := json.Marshal(struct {
		Schema   uint16   `json:"schema"`
		ServerID ServerID `json:"serverId"`
	}{Schema: SchemaV1, ServerID: state.ServerID})
	if err != nil {
		return err
	}
	request, err := manager.adminRequest(ctx, http.MethodPost, state.Address, "/admin/shutdown", configValue.ServiceSecret, body)
	if err != nil {
		return err
	}
	response, err := manager.client.Do(request)
	if err != nil {
		status, statusErr := manager.Status(ctx)
		if statusErr == nil && status.Stale {
			return manager.store.RemoveState()
		}
		return err
	}
	_ = response.Body.Close()
	if response.StatusCode != http.StatusNoContent {
		return localizedError("web.error.shutdown-failed", l10n.A("status", response.StatusCode))
	}
	if _, err = manager.waitForStatus(ctx, false); err != nil {
		return err
	}
	return manager.waitForStateRemoval(ctx)
}

func (manager *Manager) Status(ctx context.Context) (StatusV1, error) {
	configValue, err := manager.store.LoadConfig()
	if errors.Is(err, os.ErrNotExist) {
		return StatusV1{Schema: SchemaV1}, nil
	}
	if err != nil {
		return StatusV1{}, err
	}
	status := StatusV1{
		Schema:     SchemaV1,
		Registered: configValue.Registration != RegistrationNone,
		Address:    net.JoinHostPort("127.0.0.1", strconv.Itoa(int(configValue.Port))),
		Executable: configValue.Executable,
		AuthMode:   configValue.EffectiveAuthMode(),
	}
	if status.Registered {
		if configValue.Registration != manager.native.Registration() {
			return StatusV1{}, fmt.Errorf("web.registration-platform-mismatch:%s", configValue.Registration)
		}
		nativeRunning, runningErr := manager.native.Running(ctx)
		if runningErr != nil {
			return StatusV1{}, runningErr
		}
		if !nativeRunning {
			return status, nil
		}
	}
	state, err := manager.store.LoadState()
	if errors.Is(err, os.ErrNotExist) {
		return status, nil
	}
	if err != nil {
		status.Stale = true
		return status, nil
	}
	status.Address, status.PID = state.Address, state.PID
	if state.Executable != configValue.Executable || configValue.Executable != manager.executable {
		status.Stale = true
		return status, nil
	}
	request, err := manager.adminRequest(ctx, http.MethodGet, state.Address, "/healthz", configValue.ServiceSecret, nil)
	if err != nil {
		return StatusV1{}, err
	}
	response, err := manager.client.Do(request)
	if err != nil {
		status.Stale = true
		return status, nil
	}
	_ = response.Body.Close()
	status.Running = response.StatusCode == http.StatusOK
	status.Stale = !status.Running
	return status, nil
}

func (manager *Manager) Register(ctx context.Context, options RegisterOptions) (StatusV1, error) {
	value, err := manager.loadOrCreateConfig(options.Root, options.Port)
	if err != nil {
		return StatusV1{}, err
	}
	value.Registration = manager.native.Registration()
	if err = manager.store.SaveConfig(value); err != nil {
		return StatusV1{}, err
	}
	if err = manager.native.Register(ctx, manager.executable); err != nil {
		value.Registration = RegistrationNone
		return StatusV1{}, errors.Join(err, manager.store.SaveConfig(value))
	}
	return manager.waitForStatus(ctx, true)
}

func (manager *Manager) Unregister(ctx context.Context) error {
	value, err := manager.store.LoadConfig()
	if err != nil {
		return err
	}
	if value.Registration != RegistrationNone {
		if value.Registration != manager.native.Registration() {
			return fmt.Errorf("web.registration-platform-mismatch:%s", value.Registration)
		}
		if err = manager.native.Unregister(ctx); err != nil {
			return err
		}
	}
	value.Registration = RegistrationNone
	if err = manager.store.SaveConfig(value); err != nil {
		return err
	}
	return manager.store.RemoveState()
}

func (manager *Manager) Open(ctx context.Context, options OpenOptions) (OpenResult, error) {
	status, statusErr := manager.Status(ctx)
	if statusErr != nil {
		return OpenResult{}, statusErr
	}
	if !status.Running {
		if _, startErr := manager.Start(ctx, StartOptions{preserveAuth: true}); startErr != nil {
			return OpenResult{}, startErr
		}
	}
	configValue, err := manager.store.LoadConfig()
	if err != nil {
		return OpenResult{}, err
	}
	state, err := manager.store.LoadState()
	if err != nil {
		return OpenResult{}, err
	}
	location := url.URL{Scheme: "http", Host: state.Address, Path: "/"}
	switch configValue.EffectiveAuthMode() {
	case AuthNone:
		if options.NoExpiry || options.Token != nil {
			return OpenResult{}, localizedError("web.error.authentication-option-unavailable")
		}
	case AuthToken:
		if options.NoExpiry {
			return OpenResult{}, localizedError("web.error.authentication-option-unavailable")
		}
		if options.Token == nil || !AccessTokenMatches(configValue.AccessTokenDigest, *options.Token) {
			return OpenResult{}, localizedError("web.error.access-token-invalid")
		}
		location.RawQuery = url.Values{"token": []string{*options.Token}}.Encode()
	case AuthTicket:
		if options.Token != nil {
			return OpenResult{}, localizedError("web.error.authentication-option-unavailable")
		}
		body, marshalErr := json.Marshal(struct {
			Schema   uint16 `json:"schema"`
			NoExpiry bool   `json:"noExpiry"`
		}{Schema: SchemaV1, NoExpiry: options.NoExpiry})
		if marshalErr != nil {
			return OpenResult{}, marshalErr
		}
		request, requestErr := manager.adminRequest(ctx, http.MethodPost, state.Address, "/admin/tickets", configValue.ServiceSecret, body)
		if requestErr != nil {
			return OpenResult{}, requestErr
		}
		response, requestErr := manager.client.Do(request)
		if requestErr != nil {
			return OpenResult{}, requestErr
		}
		defer response.Body.Close()
		if response.StatusCode != http.StatusCreated {
			return OpenResult{}, localizedError("web.error.ticket-failed", l10n.A("status", response.StatusCode))
		}
		var ticket struct {
			Schema    uint16     `json:"schema"`
			Ticket    string     `json:"ticket"`
			ExpiresAt *time.Time `json:"expiresAt,omitempty"`
		}
		decoder := json.NewDecoder(response.Body)
		decoder.DisallowUnknownFields()
		if requestErr = decoder.Decode(&ticket); requestErr != nil || ticket.Schema != SchemaV1 || ticket.Ticket == "" {
			return OpenResult{}, localizedError("web.error.invalid-ticket")
		}
		location.RawQuery = url.Values{"ticket": []string{ticket.Ticket}}.Encode()
	default:
		return OpenResult{}, fmt.Errorf("web.invalid-auth-mode:%s", configValue.AuthMode)
	}
	result := OpenResult{Location: location.String()}
	if err = manager.launch(result.Location); err == nil {
		result.Opened = true
	}
	return result, nil
}

func (manager *Manager) loadOrCreateConfig(root *string, port *uint16) (WebConfigV1, error) {
	value, err := manager.store.LoadConfig()
	if err != nil && !errors.Is(err, os.ErrNotExist) {
		return WebConfigV1{}, err
	}
	resolvedRoot := config.ResolveRoot("")
	resolvedPort := manager.settings.DefaultPort
	if err == nil {
		resolvedRoot, resolvedPort = value.Root, value.Port
	}
	if root != nil {
		resolvedRoot = config.ResolveRoot(*root)
	}
	if port != nil {
		resolvedPort = *port
	}
	return manager.store.EnsureConfig(resolvedRoot, resolvedPort, manager.executable)
}

func resolveStartAuth(previous WebConfigV1, hasPrevious bool, options StartOptions) (AuthMode, string, error) {
	if options.preserveAuth && hasPrevious {
		return previous.EffectiveAuthMode(), previous.AccessTokenDigest, nil
	}
	if options.Unauthenticated && options.Token != nil {
		return "", "", localizedError("web.error.authentication-options-conflict")
	}
	if options.Unauthenticated {
		return AuthNone, "", nil
	}
	if options.Token != nil {
		digest, err := HashAccessToken(*options.Token)
		return AuthToken, digest, err
	}
	return AuthTicket, "", nil
}

func (manager *Manager) waitForStatus(ctx context.Context, running bool) (StatusV1, error) {
	ticker := time.NewTicker(runtimeconfig.Milliseconds(manager.settings.StatusPollMilliseconds))
	defer ticker.Stop()
	timeout := time.NewTimer(runtimeconfig.Milliseconds(manager.settings.StartTimeoutMilliseconds))
	defer timeout.Stop()
	for {
		status, err := manager.Status(ctx)
		if err == nil && status.Running == running {
			return status, nil
		}
		select {
		case <-ctx.Done():
			return StatusV1{}, ctx.Err()
		case <-timeout.C:
			return StatusV1{}, localizedError("web.error.service-timeout")
		case <-ticker.C:
		}
	}
}

func (manager *Manager) waitForStateRemoval(ctx context.Context) error {
	ticker := time.NewTicker(runtimeconfig.Milliseconds(manager.settings.StatePollMilliseconds))
	defer ticker.Stop()
	for {
		_, err := os.Stat(manager.store.Paths().StateFile)
		if errors.Is(err, os.ErrNotExist) {
			return nil
		}
		if err != nil {
			return err
		}
		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-ticker.C:
		}
	}
}

func (manager *Manager) adminRequest(ctx context.Context, method, address, path string, secret ServiceSecret, body []byte) (*http.Request, error) {
	request, err := http.NewRequestWithContext(ctx, method, "http://"+address+path, bytes.NewReader(body))
	if err != nil {
		return nil, err
	}
	request.Header.Set("Authorization", "Bearer "+secret.String())
	if len(body) != 0 {
		request.Header.Set("Content-Type", "application/json")
	}
	return request, nil
}

func filepathAbs(path string) (string, error) {
	if strings.TrimSpace(path) == "" {
		return "", fmt.Errorf("web.executable-required")
	}
	return filepath.Abs(path)
}

func openBrowser(location string) error {
	var command *exec.Cmd
	switch runtime.GOOS {
	case "windows":
		command = exec.Command("cmd", "/c", "start", "", location)
	case "darwin":
		command = exec.Command("open", location)
	default:
		command = exec.Command("xdg-open", location)
	}
	return command.Start()
}

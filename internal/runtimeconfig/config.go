package runtimeconfig

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"time"

	"github.com/sachahjkl/dw/internal/config"
)

const SchemaV1 uint16 = 1

// Config contains user-editable process limits and polling intervals.
type Config struct {
	Schema     uint16     `json:"schema"`
	Execution  Execution  `json:"execution"`
	Web        Web        `json:"web"`
	WebService WebService `json:"webService"`
}

type Execution struct {
	LeaseRenewMilliseconds      int64  `json:"leaseRenewMilliseconds"`
	LeaseDurationMilliseconds   int64  `json:"leaseDurationMilliseconds"`
	CloseTimeoutMilliseconds    int64  `json:"closeTimeoutMilliseconds"`
	LockRetryMilliseconds       int64  `json:"lockRetryMilliseconds"`
	PersistencePollMilliseconds int64  `json:"persistencePollMilliseconds"`
	MaxEvents                   uint64 `json:"maxEvents"`
	MaxPayloadBytes             int    `json:"maxPayloadBytes"`
	MaxTerminalRecordsPerRoot   uint16 `json:"maxTerminalRecordsPerRoot"`
	SubscriberCapacity          int    `json:"subscriberCapacity"`
	EventFetchLimit             uint16 `json:"eventFetchLimit"`
}

type Web struct {
	TicketTTLSeconds         int64  `json:"ticketTTLSeconds"`
	SessionTTLSeconds        int64  `json:"sessionTTLSeconds"`
	MaxRequestBodyBytes      int64  `json:"maxRequestBodyBytes"`
	MaxHeaderBytes           int    `json:"maxHeaderBytes"`
	ReadHeaderTimeoutSeconds int64  `json:"readHeaderTimeoutSeconds"`
	IdleTimeoutSeconds       int64  `json:"idleTimeoutSeconds"`
	ShutdownTimeoutSeconds   int64  `json:"shutdownTimeoutSeconds"`
	SSEHeartbeatSeconds      int64  `json:"sseHeartbeatSeconds"`
	PagePollMilliseconds     int64  `json:"pagePollMilliseconds"`
	EventSettleMilliseconds  int64  `json:"eventSettleMilliseconds"`
	RecentExecutionLimit     uint16 `json:"recentExecutionLimit"`
}

type WebService struct {
	DefaultPort                   uint16 `json:"defaultPort"`
	HTTPClientTimeoutMilliseconds int64  `json:"httpClientTimeoutMilliseconds"`
	StartTimeoutMilliseconds      int64  `json:"startTimeoutMilliseconds"`
	StatusPollMilliseconds        int64  `json:"statusPollMilliseconds"`
	StatePollMilliseconds         int64  `json:"statePollMilliseconds"`
}

func Default() Config {
	return Config{
		Schema: SchemaV1,
		Execution: Execution{
			LeaseRenewMilliseconds:      5_000,
			LeaseDurationMilliseconds:   15_000,
			CloseTimeoutMilliseconds:    10_000,
			LockRetryMilliseconds:       50,
			PersistencePollMilliseconds: 50,
			MaxEvents:                   10_000,
			MaxPayloadBytes:             256 * 1024,
			MaxTerminalRecordsPerRoot:   500,
			SubscriberCapacity:          64,
			EventFetchLimit:             1_000,
		},
		Web: Web{
			TicketTTLSeconds:         60,
			SessionTTLSeconds:        12 * 60 * 60,
			MaxRequestBodyBytes:      1 << 20,
			MaxHeaderBytes:           64 << 10,
			ReadHeaderTimeoutSeconds: 5,
			IdleTimeoutSeconds:       60,
			ShutdownTimeoutSeconds:   10,
			SSEHeartbeatSeconds:      15,
			PagePollMilliseconds:     2_000,
			EventSettleMilliseconds:  25,
			RecentExecutionLimit:     20,
		},
		WebService: WebService{
			DefaultPort:                   7331,
			HTTPClientTimeoutMilliseconds: 2_000,
			StartTimeoutMilliseconds:      10_000,
			StatusPollMilliseconds:        50,
			StatePollMilliseconds:         25,
		},
	}
}

func Path(dirs config.PlatformBaseDirs) string {
	return filepath.Join(dirs.UserConfigDirectory(), "runtime.json")
}

// Load creates runtime.json with defaults when the file does not exist.
func Load(dirs config.PlatformBaseDirs) (Config, error) {
	path := Path(dirs)
	content, err := os.ReadFile(path)
	if errors.Is(err, os.ErrNotExist) {
		value := Default()
		if err = save(path, value); err != nil {
			return Config{}, err
		}
		return value, nil
	}
	if err != nil {
		return Config{}, err
	}
	decoder := json.NewDecoder(bytes.NewReader(content))
	decoder.DisallowUnknownFields()
	var value Config
	if err = decoder.Decode(&value); err != nil {
		return Config{}, fmt.Errorf("runtime-config.invalid-json:%w", err)
	}
	if err = decoder.Decode(&struct{}{}); !errors.Is(err, io.EOF) {
		return Config{}, fmt.Errorf("runtime-config.trailing-json")
	}
	if err = value.Validate(); err != nil {
		return Config{}, err
	}
	return value, nil
}

func (value Config) Validate() error {
	if value.Schema != SchemaV1 {
		return fmt.Errorf("runtime-config.invalid-schema:%d", value.Schema)
	}
	positive := map[string]int64{
		"execution.leaseRenewMilliseconds":         value.Execution.LeaseRenewMilliseconds,
		"execution.leaseDurationMilliseconds":      value.Execution.LeaseDurationMilliseconds,
		"execution.closeTimeoutMilliseconds":       value.Execution.CloseTimeoutMilliseconds,
		"execution.lockRetryMilliseconds":          value.Execution.LockRetryMilliseconds,
		"execution.persistencePollMilliseconds":    value.Execution.PersistencePollMilliseconds,
		"web.ticketTTLSeconds":                     value.Web.TicketTTLSeconds,
		"web.sessionTTLSeconds":                    value.Web.SessionTTLSeconds,
		"web.maxRequestBodyBytes":                  value.Web.MaxRequestBodyBytes,
		"web.readHeaderTimeoutSeconds":             value.Web.ReadHeaderTimeoutSeconds,
		"web.idleTimeoutSeconds":                   value.Web.IdleTimeoutSeconds,
		"web.shutdownTimeoutSeconds":               value.Web.ShutdownTimeoutSeconds,
		"web.sseHeartbeatSeconds":                  value.Web.SSEHeartbeatSeconds,
		"web.pagePollMilliseconds":                 value.Web.PagePollMilliseconds,
		"web.eventSettleMilliseconds":              value.Web.EventSettleMilliseconds,
		"webService.httpClientTimeoutMilliseconds": value.WebService.HTTPClientTimeoutMilliseconds,
		"webService.startTimeoutMilliseconds":      value.WebService.StartTimeoutMilliseconds,
		"webService.statusPollMilliseconds":        value.WebService.StatusPollMilliseconds,
		"webService.statePollMilliseconds":         value.WebService.StatePollMilliseconds,
	}
	for name, number := range positive {
		if number <= 0 {
			return fmt.Errorf("runtime-config.non-positive:%s", name)
		}
	}
	if value.Execution.LeaseDurationMilliseconds <= value.Execution.LeaseRenewMilliseconds {
		return fmt.Errorf("runtime-config.invalid-lease-duration")
	}
	if value.Execution.MaxEvents < 3 || value.Execution.MaxPayloadBytes <= 0 || value.Execution.MaxTerminalRecordsPerRoot == 0 || value.Execution.SubscriberCapacity <= 0 || value.Execution.EventFetchLimit == 0 {
		return fmt.Errorf("runtime-config.invalid-execution-limit")
	}
	if value.Web.MaxHeaderBytes <= 0 || value.Web.RecentExecutionLimit == 0 {
		return fmt.Errorf("runtime-config.invalid-web-limit")
	}
	if value.WebService.DefaultPort == 0 {
		return fmt.Errorf("runtime-config.invalid-default-port")
	}
	return nil
}

func ValidateExecution(value Execution) error {
	config := Default()
	config.Execution = value
	return config.Validate()
}

func ValidateWeb(value Web) error {
	config := Default()
	config.Web = value
	return config.Validate()
}

func ValidateWebService(value WebService) error {
	config := Default()
	config.WebService = value
	return config.Validate()
}

func save(path string, value Config) error {
	content, err := json.MarshalIndent(value, "", "  ")
	if err != nil {
		return err
	}
	content = append(content, '\n')
	if err = os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	file, err := os.CreateTemp(filepath.Dir(path), ".dw-runtime-*")
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
	if err = file.Chmod(0o644); err != nil {
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

func Milliseconds(value int64) time.Duration { return time.Duration(value) * time.Millisecond }
func Seconds(value int64) time.Duration      { return time.Duration(value) * time.Second }

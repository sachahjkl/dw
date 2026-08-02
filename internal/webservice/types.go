package webservice

import (
	"crypto/rand"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"github.com/sachahjkl/dw/internal/runtimeconfig"
	"net"
	"strings"
	"time"
)

const SchemaV1 uint16 = 1

var DefaultPort = runtimeconfig.Default().WebService.DefaultPort

const (
	RegistrationNone          Registration = "none"
	RegistrationSystemdUser   Registration = "systemd-user"
	RegistrationTaskScheduler Registration = "task-scheduler"
)

type Registration string

type ServiceSecret struct{ value [32]byte }
type ServerID struct{ value [16]byte }

func NewServiceSecret() (ServiceSecret, error) {
	var value [32]byte
	_, err := rand.Read(value[:])
	return ServiceSecret{value: value}, err
}

func ParseServiceSecret(text string) (ServiceSecret, error) {
	decoded, err := base64.RawURLEncoding.DecodeString(text)
	if err != nil || len(decoded) != 32 {
		return ServiceSecret{}, fmt.Errorf("web.invalid-service-secret")
	}
	var value [32]byte
	copy(value[:], decoded)
	return ServiceSecret{value: value}, nil
}

func (secret ServiceSecret) String() string {
	return base64.RawURLEncoding.EncodeToString(secret.value[:])
}

func (secret ServiceSecret) IsZero() bool    { return secret.value == [32]byte{} }
func (secret ServiceSecret) Bytes() [32]byte { return secret.value }

func (secret ServiceSecret) MarshalJSON() ([]byte, error) { return json.Marshal(secret.String()) }
func (secret *ServiceSecret) UnmarshalJSON(data []byte) error {
	var text string
	if err := json.Unmarshal(data, &text); err != nil {
		return fmt.Errorf("web.invalid-service-secret")
	}
	value, err := ParseServiceSecret(text)
	if err == nil {
		*secret = value
	}
	return err
}

func NewServerID() (ServerID, error) {
	var value [16]byte
	_, err := rand.Read(value[:])
	return ServerID{value: value}, err
}

func ParseServerID(text string) (ServerID, error) {
	decoded, err := base64.RawURLEncoding.DecodeString(text)
	if err != nil || len(decoded) != 16 {
		return ServerID{}, fmt.Errorf("web.invalid-server-id")
	}
	var value [16]byte
	copy(value[:], decoded)
	return ServerID{value: value}, nil
}

func (id ServerID) String() string               { return base64.RawURLEncoding.EncodeToString(id.value[:]) }
func (id ServerID) IsZero() bool                 { return id.value == [16]byte{} }
func (id ServerID) MarshalJSON() ([]byte, error) { return json.Marshal(id.String()) }
func (id *ServerID) UnmarshalJSON(data []byte) error {
	var text string
	if err := json.Unmarshal(data, &text); err != nil {
		return fmt.Errorf("web.invalid-server-id")
	}
	value, err := ParseServerID(text)
	if err == nil {
		*id = value
	}
	return err
}

type WebConfigV1 struct {
	Schema        uint16        `json:"schema"`
	Root          string        `json:"root"`
	Port          uint16        `json:"port"`
	Executable    string        `json:"executable"`
	Registration  Registration  `json:"registration"`
	ServiceSecret ServiceSecret `json:"serviceSecret"`
}

type WebStateV1 struct {
	Schema     uint16    `json:"schema"`
	ServerID   ServerID  `json:"serverId"`
	PID        int       `json:"pid"`
	Address    string    `json:"address"`
	StartedAt  time.Time `json:"startedAt"`
	Executable string    `json:"executable"`
}

type StatusV1 struct {
	Schema     uint16 `json:"schema"`
	Running    bool   `json:"running"`
	Registered bool   `json:"registered"`
	Address    string `json:"address,omitempty"`
	PID        int    `json:"pid,omitempty"`
	Executable string `json:"executable,omitempty"`
	Stale      bool   `json:"stale"`
}

func (config WebConfigV1) Validate() error {
	if config.Schema != SchemaV1 || strings.TrimSpace(config.Root) == "" || strings.TrimSpace(config.Executable) == "" || config.ServiceSecret.IsZero() {
		return fmt.Errorf("web.invalid-config")
	}
	switch config.Registration {
	case RegistrationNone, RegistrationSystemdUser, RegistrationTaskScheduler:
		return nil
	default:
		return fmt.Errorf("web.invalid-registration:%s", config.Registration)
	}
}

func (state WebStateV1) Validate() error {
	if state.Schema != SchemaV1 || state.ServerID.IsZero() || state.PID <= 0 || state.StartedAt.IsZero() || strings.TrimSpace(state.Executable) == "" {
		return fmt.Errorf("web.invalid-state")
	}
	host, port, err := net.SplitHostPort(state.Address)
	if err != nil || port == "" || net.ParseIP(host) == nil || !net.ParseIP(host).IsLoopback() {
		return fmt.Errorf("web.invalid-state-address:%s", state.Address)
	}
	return nil
}

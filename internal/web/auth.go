package web

import (
	"crypto/rand"
	"crypto/subtle"
	"encoding/base64"
	"fmt"
	"net"
	"net/http"
	"strings"
	"sync"
	"time"

	"github.com/sachahjkl/dw/internal/runtimeconfig"
	"github.com/sachahjkl/dw/internal/webservice"
)

const sessionCookieName = "dw_session"

type ticket struct {
	value     [32]byte
	expiresAt time.Time
}

type session struct {
	csrf      [32]byte
	expiresAt time.Time
}

type authState struct {
	mu       sync.Mutex
	secret   webservice.ServiceSecret
	settings runtimeconfig.Web
	tickets  []ticket
	sessions map[string]session
	now      func() time.Time
}

func newAuthState(secret webservice.ServiceSecret, settings runtimeconfig.Web) *authState {
	if settings.TicketTTLSeconds == 0 {
		settings = runtimeconfig.Default().Web
	}
	return &authState{secret: secret, settings: settings, sessions: make(map[string]session), now: time.Now}
}

func randomToken() ([32]byte, error) {
	var token [32]byte
	_, err := rand.Read(token[:])
	return token, err
}

func encodeToken(token [32]byte) string { return base64.RawURLEncoding.EncodeToString(token[:]) }

func decodeToken(text string) ([32]byte, error) {
	decoded, err := base64.RawURLEncoding.DecodeString(text)
	if err != nil || len(decoded) != 32 {
		return [32]byte{}, fmt.Errorf("web.invalid-token")
	}
	var token [32]byte
	copy(token[:], decoded)
	return token, nil
}

func (state *authState) createTicket() (ticket, error) {
	value, err := randomToken()
	if err != nil {
		return ticket{}, err
	}
	state.mu.Lock()
	defer state.mu.Unlock()
	now := state.now()
	state.pruneLocked(now)
	created := ticket{value: value, expiresAt: now.Add(runtimeconfig.Seconds(state.settings.TicketTTLSeconds))}
	state.tickets = append(state.tickets, created)
	return created, nil
}

func (state *authState) consumeTicket(encoded string) (string, string, bool) {
	candidate, err := decodeToken(encoded)
	if err != nil {
		return "", "", false
	}
	state.mu.Lock()
	defer state.mu.Unlock()
	now := state.now()
	state.pruneLocked(now)
	for index := range state.tickets {
		if subtle.ConstantTimeCompare(candidate[:], state.tickets[index].value[:]) != 1 {
			continue
		}
		state.tickets = append(state.tickets[:index], state.tickets[index+1:]...)
		sessionToken, tokenErr := randomToken()
		if tokenErr != nil {
			return "", "", false
		}
		csrf, csrfErr := randomToken()
		if csrfErr != nil {
			return "", "", false
		}
		key := encodeToken(sessionToken)
		state.sessions[key] = session{csrf: csrf, expiresAt: now.Add(runtimeconfig.Seconds(state.settings.SessionTTLSeconds))}
		return key, encodeToken(csrf), true
	}
	return "", "", false
}

func (state *authState) authenticate(request *http.Request) (session, bool) {
	cookie, err := request.Cookie(sessionCookieName)
	if err != nil {
		return session{}, false
	}
	state.mu.Lock()
	defer state.mu.Unlock()
	now := state.now()
	state.pruneLocked(now)
	value, ok := state.sessions[cookie.Value]
	return value, ok
}

func (state *authState) authorizeMutation(request *http.Request, origin string) bool {
	value, ok := state.authenticate(request)
	if !ok || request.Header.Get("Origin") != origin {
		return false
	}
	candidate, err := decodeToken(request.Header.Get("X-DW-CSRF"))
	if err != nil {
		return false
	}
	return subtle.ConstantTimeCompare(candidate[:], value.csrf[:]) == 1
}

func (state *authState) authorizeAdmin(request *http.Request) bool {
	if !remoteLoopback(request.RemoteAddr) {
		return false
	}
	const prefix = "Bearer "
	header := request.Header.Get("Authorization")
	if !strings.HasPrefix(header, prefix) {
		return false
	}
	candidate, err := decodeToken(strings.TrimPrefix(header, prefix))
	if err != nil {
		return false
	}
	expected := state.secret.Bytes()
	return subtle.ConstantTimeCompare(candidate[:], expected[:]) == 1
}

func (state *authState) pruneLocked(now time.Time) {
	kept := state.tickets[:0]
	for _, value := range state.tickets {
		if now.Before(value.expiresAt) {
			kept = append(kept, value)
		}
	}
	state.tickets = kept
	for key, value := range state.sessions {
		if !now.Before(value.expiresAt) {
			delete(state.sessions, key)
		}
	}
}

func remoteLoopback(address string) bool {
	host, _, err := net.SplitHostPort(address)
	if err != nil {
		return false
	}
	ip := net.ParseIP(host)
	return ip != nil && ip.IsLoopback()
}

package web

import (
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/sachahjkl/dw/internal/runtimeconfig"
	"github.com/sachahjkl/dw/internal/webservice"
)

func TestTicketIsShortLivedAndSingleUse(t *testing.T) {
	secret, err := webservice.NewServiceSecret()
	if err != nil {
		t.Fatal(err)
	}
	state := newAuthState(secret, runtimeconfig.Default().Web)
	now := time.Date(2026, 8, 2, 10, 0, 0, 0, time.UTC)
	state.now = func() time.Time { return now }

	valid, err := state.createTicket()
	if err != nil {
		t.Fatal(err)
	}
	sessionToken, csrf, ok := state.consumeTicket(encodeToken(valid.value))
	if !ok || sessionToken == "" || csrf == "" {
		t.Fatal("valid ticket was rejected")
	}
	if _, _, reused := state.consumeTicket(encodeToken(valid.value)); reused {
		t.Fatal("ticket was accepted twice")
	}
	if _, _, unknown := state.consumeTicket(encodeToken([32]byte{1})); unknown {
		t.Fatal("unknown ticket was accepted")
	}

	expired, err := state.createTicket()
	if err != nil {
		t.Fatal(err)
	}
	now = now.Add(time.Minute)
	if _, _, accepted := state.consumeTicket(encodeToken(expired.value)); accepted {
		t.Fatal("expired ticket was accepted")
	}
}

func TestMutationRequiresSessionOriginAndCSRF(t *testing.T) {
	secret, err := webservice.NewServiceSecret()
	if err != nil {
		t.Fatal(err)
	}
	state := newAuthState(secret, runtimeconfig.Default().Web)
	ticket, err := state.createTicket()
	if err != nil {
		t.Fatal(err)
	}
	sessionToken, csrf, ok := state.consumeTicket(encodeToken(ticket.value))
	if !ok {
		t.Fatal("ticket exchange failed")
	}
	request := httptest.NewRequest(http.MethodPost, "http://127.0.0.1:7331/executions", nil)
	request.AddCookie(&http.Cookie{Name: sessionCookieName, Value: sessionToken})
	request.Header.Set("Origin", "http://127.0.0.1:7331")
	request.Header.Set("X-DW-CSRF", csrf)
	if !state.authorizeMutation(request, "http://127.0.0.1:7331") {
		t.Fatal("valid mutation was rejected")
	}
	request.Header.Del("X-DW-CSRF")
	if state.authorizeMutation(request, "http://127.0.0.1:7331") {
		t.Fatal("mutation without CSRF was accepted")
	}
	request.Header.Set("X-DW-CSRF", csrf)
	request.Header.Set("Origin", "http://localhost:7331")
	if state.authorizeMutation(request, "http://127.0.0.1:7331") {
		t.Fatal("mutation with the wrong origin was accepted")
	}
}

func TestSecurityHeadersAndRequestLimit(t *testing.T) {
	server := &Server{}
	handler := server.securityHeaders(http.HandlerFunc(func(writer http.ResponseWriter, _ *http.Request) { writer.WriteHeader(http.StatusNoContent) }))
	recorder := httptest.NewRecorder()
	handler.ServeHTTP(recorder, httptest.NewRequest(http.MethodGet, "http://127.0.0.1/", nil))
	if !strings.Contains(recorder.Header().Get("Content-Security-Policy"), "script-src 'self'") {
		t.Fatalf("missing CSP: %q", recorder.Header().Get("Content-Security-Policy"))
	}
	if recorder.Header().Get("X-Content-Type-Options") != "nosniff" {
		t.Fatal("missing nosniff header")
	}

	body := strings.NewReader(`{"schema":1,"value":"` + strings.Repeat("x", int(runtimeconfig.Default().Web.MaxRequestBodyBytes)) + `"}`)
	request := httptest.NewRequest(http.MethodPost, "http://127.0.0.1/", body)
	var value TextResponseV1
	if err := decodeRequest(httptest.NewRecorder(), request, &value, runtimeconfig.Default().Web.MaxRequestBodyBytes); err == nil {
		t.Fatal("oversized request was accepted")
	}
}

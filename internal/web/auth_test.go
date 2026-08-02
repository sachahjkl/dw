package web

import (
	"bytes"
	"encoding/json"
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
	state := newAuthState(secret, runtimeconfig.Default().Web, webservice.AuthTicket, "")
	now := time.Date(2026, 8, 2, 10, 0, 0, 0, time.UTC)
	state.now = func() time.Time { return now }

	valid, err := state.createTicket(false)
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

	expired, err := state.createTicket(false)
	if err != nil {
		t.Fatal(err)
	}
	now = now.Add(time.Minute)
	if _, _, accepted := state.consumeTicket(encodeToken(expired.value)); accepted {
		t.Fatal("expired ticket was accepted")
	}

	persistent, err := state.createTicket(true)
	if err != nil {
		t.Fatal(err)
	}
	now = now.Add(365 * 24 * time.Hour)
	if _, _, accepted := state.consumeTicket(encodeToken(persistent.value)); !accepted {
		t.Fatal("non-expiring ticket was rejected")
	}
}

func TestCreateTicketCanOmitExpiration(t *testing.T) {
	secret, err := webservice.NewServiceSecret()
	if err != nil {
		t.Fatal(err)
	}
	server := &Server{
		auth: newAuthState(secret, runtimeconfig.Default().Web, webservice.AuthTicket, ""),
		deps: Dependencies{Settings: runtimeconfig.Default().Web},
	}
	request := httptest.NewRequest(http.MethodPost, "http://127.0.0.1/admin/tickets", bytes.NewBufferString(`{"schema":1,"noExpiry":true}`))
	request.RemoteAddr = "127.0.0.1:12345"
	request.Header.Set("Authorization", "Bearer "+secret.String())
	recorder := httptest.NewRecorder()

	server.handleCreateTicket(recorder, request)

	if recorder.Code != http.StatusCreated {
		t.Fatalf("status = %d, body = %q", recorder.Code, recorder.Body.String())
	}
	var ticket TicketV1
	if err = json.NewDecoder(recorder.Body).Decode(&ticket); err != nil {
		t.Fatal(err)
	}
	if ticket.Ticket == "" || ticket.ExpiresAt != nil {
		t.Fatalf("ticket = %#v", ticket)
	}
	if _, _, accepted := server.auth.consumeTicket(ticket.Ticket); !accepted {
		t.Fatal("non-expiring ticket response was not usable")
	}
}

func TestMutationRequiresSessionOriginAndCSRF(t *testing.T) {
	secret, err := webservice.NewServiceSecret()
	if err != nil {
		t.Fatal(err)
	}
	state := newAuthState(secret, runtimeconfig.Default().Web, webservice.AuthTicket, "")
	ticket, err := state.createTicket(false)
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

func TestTokenAndUnauthenticatedModesCreateSessions(t *testing.T) {
	secret, err := webservice.NewServiceSecret()
	if err != nil {
		t.Fatal(err)
	}
	digest, err := webservice.HashAccessToken("chosen-token")
	if err != nil {
		t.Fatal(err)
	}
	tokenState := newAuthState(secret, runtimeconfig.Default().Web, webservice.AuthToken, digest)
	tokenServer := &Server{auth: tokenState}
	for _, test := range []struct {
		token string
		code  int
	}{
		{token: "wrong", code: http.StatusUnauthorized},
		{token: "chosen-token", code: http.StatusSeeOther},
	} {
		recorder := httptest.NewRecorder()
		request := httptest.NewRequest(http.MethodGet, "http://127.0.0.1/?token="+test.token, nil)
		tokenServer.handleIndex(recorder, request)
		if recorder.Code != test.code {
			t.Fatalf("token %q status = %d, want %d", test.token, recorder.Code, test.code)
		}
	}
	reused := httptest.NewRecorder()
	tokenServer.handleIndex(reused, httptest.NewRequest(http.MethodGet, "http://127.0.0.1/?token=chosen-token", nil))
	if reused.Code != http.StatusSeeOther || reused.Header().Get("Location") != "/" || len(reused.Result().Cookies()) != 1 {
		t.Fatalf("reused token response = %d, location = %q, cookies = %d", reused.Code, reused.Header().Get("Location"), len(reused.Result().Cookies()))
	}
	noneServer := &Server{auth: newAuthState(secret, runtimeconfig.Default().Web, webservice.AuthNone, "")}
	recorder := httptest.NewRecorder()
	noneServer.handleIndex(recorder, httptest.NewRequest(http.MethodGet, "http://127.0.0.1/", nil))
	if recorder.Code != http.StatusSeeOther || len(recorder.Result().Cookies()) != 1 {
		t.Fatalf("unauthenticated session response = %d, cookies = %d", recorder.Code, len(recorder.Result().Cookies()))
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

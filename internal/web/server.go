package web

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"time"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/cockpit"
	"github.com/sachahjkl/dw/internal/execution"
	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/runtimeconfig"
	"github.com/sachahjkl/dw/internal/webservice"
)

type Dependencies struct {
	Executor      execution.Executor
	Actor         execution.Actor
	Localizer     l10n.Localizer
	Cockpit       *cockpit.Service
	ProjectResult func(action.Result) []string
	Store         *webservice.Store
	Config        webservice.WebConfigV1
	Settings      runtimeconfig.Web
}

type Server struct {
	deps       Dependencies
	auth       *authState
	serverID   webservice.ServerID
	origin     string
	shutdown   chan struct{}
	httpServer *http.Server
}

func New(dependencies Dependencies) (*Server, error) {
	if dependencies.Executor == nil || dependencies.Actor.Principal == "" || dependencies.Cockpit == nil || dependencies.ProjectResult == nil || dependencies.Store == nil || dependencies.Localizer == nil {
		return nil, fmt.Errorf("web.invalid-server-dependency")
	}
	if err := dependencies.Config.Validate(); err != nil {
		return nil, err
	}
	if dependencies.Settings.MaxRequestBodyBytes == 0 {
		dependencies.Settings = runtimeconfig.Default().Web
	}
	if err := runtimeconfig.ValidateWeb(dependencies.Settings); err != nil {
		return nil, err
	}
	serverID, err := webservice.NewServerID()
	if err != nil {
		return nil, err
	}
	dependencies.Actor.Origin = execution.OriginWeb
	return &Server{
		deps: dependencies,
		auth: newAuthState(
			dependencies.Config.ServiceSecret,
			dependencies.Settings,
			dependencies.Config.EffectiveAuthMode(),
			dependencies.Config.AccessTokenDigest,
		),
		serverID: serverID,
		shutdown: make(chan struct{}, 1),
	}, nil
}

func (server *Server) Serve(ctx context.Context) error {
	address := net.JoinHostPort("127.0.0.1", fmt.Sprintf("%d", server.deps.Config.Port))
	listener, err := net.Listen("tcp", address)
	if err != nil {
		return fmt.Errorf("web.port-unavailable:%s", address)
	}
	actualAddress := listener.Addr().String()
	server.origin = "http://" + actualAddress
	state := webservice.WebStateV1{
		Schema: webservice.SchemaV1, ServerID: server.serverID, PID: os.Getpid(), Address: actualAddress,
		StartedAt: time.Now().UTC(), Executable: server.deps.Config.Executable,
	}
	if err = server.deps.Store.SaveState(state); err != nil {
		_ = listener.Close()
		return err
	}
	defer server.deps.Store.RemoveState()

	server.httpServer = &http.Server{
		Handler:           server.securityHeaders(server.routes()),
		ReadHeaderTimeout: runtimeconfig.Seconds(server.deps.Settings.ReadHeaderTimeoutSeconds),
		IdleTimeout:       runtimeconfig.Seconds(server.deps.Settings.IdleTimeoutSeconds),
		MaxHeaderBytes:    server.deps.Settings.MaxHeaderBytes,
	}
	result := make(chan error, 1)
	go func() { result <- server.httpServer.Serve(listener) }()

	select {
	case err = <-result:
		if errors.Is(err, http.ErrServerClosed) {
			return nil
		}
		return err
	case <-ctx.Done():
	case <-server.shutdown:
	}
	shutdownContext, cancel := context.WithTimeout(context.Background(), runtimeconfig.Seconds(server.deps.Settings.ShutdownTimeoutSeconds))
	defer cancel()
	if err = server.httpServer.Shutdown(shutdownContext); err != nil {
		return err
	}
	err = <-result
	if errors.Is(err, http.ErrServerClosed) {
		return nil
	}
	return err
}

func (server *Server) routes() http.Handler {
	mux := http.NewServeMux()
	mux.HandleFunc("GET /healthz", server.handleHealth)
	mux.HandleFunc("POST /admin/tickets", server.handleCreateTicket)
	mux.HandleFunc("POST /admin/shutdown", server.handleShutdown)
	mux.HandleFunc("GET /", server.handleIndex)
	mux.HandleFunc("GET /assets/{name}", server.handleAsset)
	mux.HandleFunc("GET /events", server.handlePageEvents)
	mux.HandleFunc("POST /operations", server.handleSubmit)
	mux.HandleFunc("GET /executions/{id}", server.handleGetExecution)
	mux.HandleFunc("GET /executions/{id}/events", server.handleExecutionEvents)
	mux.HandleFunc("POST /executions/{id}/cancel", server.handleCancelExecution)
	mux.HandleFunc("POST /executions/{id}/responses/{promptID}", server.handleResponse)
	return mux
}

func (server *Server) securityHeaders(next http.Handler) http.Handler {
	return http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		writer.Header().Set("Content-Security-Policy", "default-src 'self'; script-src 'self' 'unsafe-eval'; style-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'")
		writer.Header().Set("Referrer-Policy", "no-referrer")
		writer.Header().Set("X-Content-Type-Options", "nosniff")
		next.ServeHTTP(writer, request)
	})
}

func (server *Server) handleHealth(writer http.ResponseWriter, request *http.Request) {
	if !server.auth.authorizeAdmin(request) {
		http.Error(writer, "unauthorized", http.StatusUnauthorized)
		return
	}
	writeJSON(writer, http.StatusOK, healthV1{Schema: schemaV1, ServerID: server.serverID})
}

func (server *Server) handleCreateTicket(writer http.ResponseWriter, request *http.Request) {
	if !server.auth.authorizeAdmin(request) {
		http.Error(writer, "unauthorized", http.StatusUnauthorized)
		return
	}
	if server.auth.mode != webservice.AuthTicket {
		http.Error(writer, "ticket authentication disabled", http.StatusConflict)
		return
	}
	var value TicketRequestV1
	if err := decodeRequest(writer, request, &value, server.deps.Settings.MaxRequestBodyBytes); err != nil {
		return
	}
	if value.Schema != schemaV1 {
		http.Error(writer, "invalid schema", http.StatusBadRequest)
		return
	}
	created, err := server.auth.createTicket(value.NoExpiry)
	if err != nil {
		http.Error(writer, "ticket generation failed", http.StatusInternalServerError)
		return
	}
	var expiresAt *time.Time
	if !created.expiresAt.IsZero() {
		expiresAt = &created.expiresAt
	}
	writeJSON(writer, http.StatusCreated, TicketV1{
		Schema: schemaV1, Ticket: encodeToken(created.value), ExpiresAt: expiresAt,
	})
}

func (server *Server) handleShutdown(writer http.ResponseWriter, request *http.Request) {
	if !server.auth.authorizeAdmin(request) {
		http.Error(writer, "unauthorized", http.StatusUnauthorized)
		return
	}
	var value ShutdownV1
	if err := decodeRequest(writer, request, &value, server.deps.Settings.MaxRequestBodyBytes); err != nil {
		return
	}
	if value.Schema != schemaV1 || value.ServerID != server.serverID {
		http.Error(writer, "stale server", http.StatusConflict)
		return
	}
	writer.WriteHeader(http.StatusNoContent)
	select {
	case server.shutdown <- struct{}{}:
	default:
	}
}

func (server *Server) handleIndex(writer http.ResponseWriter, request *http.Request) {
	writer.Header().Set("Cache-Control", "no-store")
	switch server.auth.mode {
	case webservice.AuthTicket:
		if encodedTicket := request.URL.Query().Get("ticket"); encodedTicket != "" {
			sessionToken, _, ok := server.auth.consumeTicket(encodedTicket)
			if !ok {
				http.Error(writer, "invalid ticket", http.StatusUnauthorized)
				return
			}
			setSessionCookie(writer, sessionToken)
			http.Redirect(writer, request, "/", http.StatusSeeOther)
			return
		}
	case webservice.AuthToken:
		if token := request.URL.Query().Get("token"); token != "" {
			if !server.auth.authenticateAccessToken(token) {
				http.Error(writer, "invalid access token", http.StatusUnauthorized)
				return
			}
			sessionToken, _, ok := server.auth.createSession()
			if !ok {
				http.Error(writer, "session generation failed", http.StatusInternalServerError)
				return
			}
			setSessionCookie(writer, sessionToken)
			http.Redirect(writer, request, "/", http.StatusSeeOther)
			return
		}
	case webservice.AuthNone:
		if _, ok := server.auth.authenticate(request); !ok {
			sessionToken, _, created := server.auth.createSession()
			if !created {
				http.Error(writer, "session generation failed", http.StatusInternalServerError)
				return
			}
			setSessionCookie(writer, sessionToken)
			http.Redirect(writer, request, "/", http.StatusSeeOther)
			return
		}
	default:
		http.Error(writer, "invalid authentication mode", http.StatusInternalServerError)
		return
	}
	value, ok := server.auth.authenticate(request)
	if !ok {
		http.Error(writer, "unauthorized", http.StatusUnauthorized)
		return
	}
	server.renderIndex(writer, request, encodeToken(value.csrf))
}

func setSessionCookie(writer http.ResponseWriter, sessionToken string) {
	http.SetCookie(writer, &http.Cookie{
		Name: sessionCookieName, Value: sessionToken, Path: "/",
		HttpOnly: true, SameSite: http.SameSiteStrictMode, Secure: false,
	})
}

func (server *Server) requireSession(writer http.ResponseWriter, request *http.Request) bool {
	if _, ok := server.auth.authenticate(request); !ok {
		http.Error(writer, "unauthorized", http.StatusUnauthorized)
		return false
	}
	return true
}

func (server *Server) requireMutation(writer http.ResponseWriter, request *http.Request) bool {
	if !server.auth.authorizeMutation(request, server.origin) {
		http.Error(writer, "forbidden", http.StatusForbidden)
		return false
	}
	return true
}

func decodeRequest[T requestDTO](writer http.ResponseWriter, request *http.Request, target *T, maxBytes int64) error {
	request.Body = http.MaxBytesReader(writer, request.Body, maxBytes)
	decoder := json.NewDecoder(request.Body)
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(target); err != nil {
		http.Error(writer, "invalid request", http.StatusBadRequest)
		return err
	}
	if err := decoder.Decode(&struct{}{}); !errors.Is(err, io.EOF) {
		http.Error(writer, "invalid request", http.StatusBadRequest)
		return fmt.Errorf("web.trailing-request-json")
	}
	return nil
}

type requestDTO interface {
	OperationSubmitV1 | TextResponseV1 | SecretResponseV1 | ConfirmResponseV1 | SelectOneResponseV1 | SelectManyResponseV1 | TicketRequestV1 | ShutdownV1
}

func writeJSON[T responseDTO](writer http.ResponseWriter, status int, value T) {
	writer.Header().Set("Content-Type", "application/json")
	writer.WriteHeader(status)
	_ = json.NewEncoder(writer).Encode(value)
}

type responseDTO interface {
	TicketV1 | ExecutionRefV1 | RecordV1 | EventV1 | healthV1
}

type healthV1 struct {
	Schema   uint16              `json:"schema"`
	ServerID webservice.ServerID `json:"serverId"`
}

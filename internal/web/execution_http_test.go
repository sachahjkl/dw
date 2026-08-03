package web

import (
	"bytes"
	"context"
	"errors"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/execution"
	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/runtimeconfig"
	"github.com/sachahjkl/dw/internal/webservice"
)

type promptResponseExecutor struct {
	execution.Executor
	record     execution.Record
	respondErr error
	responded  bool
}

func (executor *promptResponseExecutor) Get(context.Context, execution.Actor, execution.ExecutionID) (execution.Record, error) {
	return executor.record, nil
}

func (executor *promptResponseExecutor) Respond(context.Context, execution.Actor, execution.ExecutionID, action.PromptID, action.Response) error {
	if executor.respondErr != nil {
		return executor.respondErr
	}
	executor.responded = true
	return nil
}

func TestPromptResponseBroadcastsOnlyAfterAcceptance(t *testing.T) {
	for _, test := range []struct {
		name       string
		respondErr error
		status     int
		broadcast  bool
	}{
		{name: "accepted", status: http.StatusNoContent, broadcast: true},
		{name: "rejected", respondErr: errors.New("rejected"), status: http.StatusConflict, broadcast: false},
	} {
		t.Run(test.name, func(t *testing.T) {
			secret, err := webservice.NewServiceSecret()
			if err != nil {
				t.Fatal(err)
			}
			auth := newAuthState(secret, runtimeconfig.Default().Web, webservice.AuthTicket, "")
			ticket, err := auth.createTicket(false)
			if err != nil {
				t.Fatal(err)
			}
			session, csrf, ok := auth.consumeTicket(encodeToken(ticket.value))
			if !ok {
				t.Fatal("ticket exchange failed")
			}
			id, err := execution.NewExecutionID()
			if err != nil {
				t.Fatal(err)
			}
			prompt, err := execution.EncodePrompt(action.ConfirmPrompt{Meta: action.PromptMeta{ID: "confirm-workspace", Label: l10n.M("execution.event.input-required")}})
			if err != nil {
				t.Fatal(err)
			}
			executor := &promptResponseExecutor{record: execution.Record{ExecutionID: id, PendingPrompt: &prompt}, respondErr: test.respondErr}
			server := &Server{auth: auth, origin: "http://127.0.0.1:7331", deps: Dependencies{Executor: executor, Settings: runtimeconfig.Default().Web}}
			updates, unsubscribe := server.subscribeActionUpdates()
			defer unsubscribe()
			request := httptest.NewRequest(http.MethodPost, "http://127.0.0.1:7331/executions/response", bytes.NewBufferString(`{"schema":1,"accepted":true}`))
			request.SetPathValue("id", id.String())
			request.SetPathValue("promptID", string(prompt.ID))
			request.AddCookie(&http.Cookie{Name: sessionCookieName, Value: session})
			request.Header.Set("Origin", server.origin)
			request.Header.Set("X-DW-CSRF", csrf)
			request.Header.Set("Content-Type", "application/json")
			recorder := httptest.NewRecorder()
			server.handleResponse(recorder, request)
			if recorder.Code != test.status || executor.responded != test.broadcast {
				t.Fatalf("response status = %d, responded = %t", recorder.Code, executor.responded)
			}
			select {
			case <-updates:
				if !test.broadcast {
					t.Fatal("rejected prompt response produced an action update")
				}
			case <-time.After(runtimeconfig.Milliseconds(runtimeconfig.Default().Web.EventSettleMilliseconds * 2)):
				if test.broadcast {
					t.Fatal("accepted prompt response did not produce an action update")
				}
			}
		})
	}
}

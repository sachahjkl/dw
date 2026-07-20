package ado

import (
	"context"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"html"
	"io"
	"net"
	"net/http"
	"net/url"
	"strings"
	"time"
)

func (a *Authenticator) LoginBrowser(ctx context.Context) (Token, error) {
	tenant, clientID, err := a.tenantAndClient()
	if err != nil {
		return Token{}, err
	}
	port, err := reserveLoopbackPort()
	if err != nil {
		return Token{}, browserError(err)
	}
	redirectURI := "http://localhost:" + port
	state, err := randomURLToken(32)
	if err != nil {
		return Token{}, browserError(err)
	}
	verifier, err := randomURLToken(64)
	if err != nil {
		return Token{}, browserError(err)
	}
	challengeBytes := sha256.Sum256([]byte(verifier))
	challenge := base64.RawURLEncoding.EncodeToString(challengeBytes[:])
	authorizationURL := "https://login.microsoftonline.com/" + tenant + "/oauth2/v2.0/authorize"
	query := url.Values{
		"client_id":             {clientID},
		"response_type":         {"code"},
		"redirect_uri":          {redirectURI},
		"response_mode":         {"query"},
		"scope":                 {strings.Join(a.scopes(true), " ")},
		"state":                 {state},
		"code_challenge":        {challenge},
		"code_challenge_method": {"S256"},
	}
	authorizationURL += "?" + query.Encode()
	callback := make(chan callbackResult, 1)
	go serveBrowserCallback(port, state, callback)
	if a.OpenURL == nil {
		a.OpenURL = openURL
	}
	if err := a.OpenURL(authorizationURL); err != nil {
		return Token{}, browserError(err)
	}
	var result callbackResult
	timer := time.NewTimer(180 * time.Second)
	defer timer.Stop()
	select {
	case <-ctx.Done():
		return Token{}, ctx.Err()
	case <-timer.C:
		return Token{}, &Error{Kind: ErrorLoginExpired}
	case result = <-callback:
		if result.Err != nil {
			return Token{}, result.Err
		}
	}
	form := url.Values{
		"client_id":     {clientID},
		"scope":         {strings.Join(a.scopes(true), " ")},
		"code":          {result.Code},
		"redirect_uri":  {redirectURI},
		"grant_type":    {"authorization_code"},
		"code_verifier": {verifier},
	}
	var token oauthTokenResponse
	if err := a.postBrowserTokenForm(ctx, "https://login.microsoftonline.com/"+tenant+"/oauth2/v2.0/token", form, &token); err != nil {
		return Token{}, err
	}
	if token.RefreshToken == "" {
		return Token{}, &Error{Kind: ErrorBrowserLogin, Detail: "Microsoft did not return a refresh_token."}
	}
	if err := a.storeRefreshToken(ctx, token.RefreshToken); err != nil {
		return Token{}, err
	}
	return *a.tokenResult(token, "browser"), nil
}

type callbackResult struct {
	Code string
	Err  error
}

func reserveLoopbackPort() (string, error) {
	listener, err := net.Listen("tcp4", "127.0.0.1:0")
	if err != nil {
		return "", err
	}
	port := listener.Addr().(*net.TCPAddr).Port
	if err := listener.Close(); err != nil {
		return "", err
	}
	return strconvItoa(port), nil
}

func serveBrowserCallback(port, expectedState string, result chan<- callbackResult) {
	listener, err := net.Listen("tcp4", "127.0.0.1:"+port)
	if err != nil {
		result <- callbackResult{Err: browserError(err)}
		return
	}
	server := new(http.Server)
	server.ReadHeaderTimeout = 180 * time.Second
	server.Handler = http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		callback := parseCallback(request.URL.Query(), expectedState)
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		if callback.Err == nil {
			_, _ = io.WriteString(w, successPage())
		} else {
			_, _ = io.WriteString(w, errorPage(callback.Err.Error()))
		}
		select {
		case result <- callback:
		default:
		}
		go server.Close()
	})
	if err := server.Serve(listener); err != nil && err != http.ErrServerClosed {
		select {
		case result <- callbackResult{Err: browserError(err)}:
		default:
		}
	}
}

func parseCallback(query url.Values, expectedState string) callbackResult {
	if name := query.Get("error"); name != "" {
		detail := query.Get("error_description")
		if detail == "" {
			detail = name
		}
		return callbackResult{Err: &Error{Kind: ErrorBrowserLogin, Detail: detail}}
	}
	state := query.Get("state")
	if state == "" {
		return callbackResult{Err: &Error{Kind: ErrorBrowserLogin, Detail: "OAuth callback is missing state."}}
	}
	if state != expectedState {
		return callbackResult{Err: &Error{Kind: ErrorBrowserLogin, Detail: "Invalid OAuth callback state."}}
	}
	code := query.Get("code")
	if code == "" {
		return callbackResult{Err: &Error{Kind: ErrorBrowserLogin, Detail: "OAuth callback is missing code."}}
	}
	return callbackResult{Code: code}
}

func randomURLToken(count int) (string, error) {
	data := make([]byte, count)
	if _, err := rand.Read(data); err != nil {
		return "", err
	}
	return base64.RawURLEncoding.EncodeToString(data), nil
}

func (a *Authenticator) postBrowserTokenForm(ctx context.Context, endpoint string, form url.Values, target any) error {
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, endpoint, strings.NewReader(form.Encode()))
	if err != nil {
		return browserError(err)
	}
	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	response, err := a.client().Do(req)
	if err != nil {
		return browserError(err)
	}
	defer response.Body.Close()
	body, err := io.ReadAll(response.Body)
	if err != nil {
		return browserError(err)
	}
	if response.StatusCode != http.StatusOK {
		return &Error{Kind: ErrorBrowserLogin, Detail: safeOAuthBody(body)}
	}
	if err := json.Unmarshal(body, target); err != nil {
		return &Error{Kind: ErrorBrowserLogin, Detail: "Invalid OAuth token response: " + err.Error(), Cause: err}
	}
	return nil
}

func safeOAuthBody(body []byte) string {
	var value oauthErrorResponse
	if json.Unmarshal(body, &value) == nil && value.Error != "" {
		if value.ErrorDescription != "" {
			return value.Error + ": " + value.ErrorDescription
		}
		return value.Error
	}
	return string(body)
}

func browserError(err error) error {
	return &Error{Kind: ErrorBrowserLogin, Detail: err.Error(), Cause: err}
}
func strconvItoa(value int) string {
	if value == 0 {
		return "0"
	}
	var buffer [20]byte
	index := len(buffer)
	for value > 0 {
		index--
		buffer[index] = byte('0' + value%10)
		value /= 10
	}
	return string(buffer[index:])
}

func successPage() string {
	return `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>DevWorkflow connected</title></head><body><main><div>DevWorkflow — Connected</div><h1>Success.</h1><p>You may close this tab.</p></main></body></html>`
}

func errorPage(message string) string {
	return `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>DevWorkflow - error</title></head><body><main><div>DevWorkflow — Error</div><h1>Unable to connect.</h1><p>` + html.EscapeString(message) + `</p></main></body></html>`
}

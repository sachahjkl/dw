package ado

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/url"
	"os"
	"os/exec"
	"runtime"
	"strconv"
	"strings"
	"time"

	"github.com/sachahjkl/dw/internal/contract"
)

const (
	DefaultTenantID       = "organizations"
	DefaultPublicClientID = "04b07795-8ddb-461a-bbee-02f9e1bf7b46"
	ADOResourceID         = "499b84ac-1321-427f-aa17-267ca6975798"
	DefaultADOScope       = ADOResourceID + "/.default"
)

type Authenticator struct {
	Options   *AuthOptions
	Store     contract.SecretStore
	NewClient func() HTTPDoer
	OpenURL   func(string) error
	Now       func() time.Time
}

func NewAuthenticator(options *AuthOptions, store contract.SecretStore) *Authenticator {
	return &Authenticator{
		Options:   options,
		Store:     store,
		NewClient: newDefaultHTTPClient,
		OpenURL:   openURL,
		Now:       time.Now,
	}
}

func EnvironmentToken() *Token {
	for _, name := range [...]string{"DW_ADO_TOKEN", "AZURE_DEVOPS_EXT_PAT"} {
		if value, ok := os.LookupEnv(name); ok && strings.TrimSpace(value) != "" {
			return &Token{AccessToken: value, Source: "environment PAT", Scheme: AuthBasic}
		}
	}
	return nil
}

func EnvironmentPATVariable() string {
	for _, name := range [...]string{"DW_ADO_TOKEN", "AZURE_DEVOPS_EXT_PAT"} {
		if value, ok := os.LookupEnv(name); ok && strings.TrimSpace(value) != "" {
			return name
		}
	}
	return ""
}

func (a *Authenticator) client() HTTPDoer {
	if a != nil && a.NewClient != nil {
		return a.NewClient()
	}
	return newDefaultHTTPClient()
}

func (a *Authenticator) now() time.Time {
	if a != nil && a.Now != nil {
		return a.Now()
	}
	return time.Now()
}

func (a *Authenticator) scopes(interactive bool) []string {
	var values []string
	if a != nil && a.Options != nil && len(a.Options.Scopes) != 0 {
		values = append(values, a.Options.Scopes...)
	} else {
		values = []string{DefaultADOScope}
	}
	if interactive {
		found := false
		for _, value := range values {
			if value == "offline_access" {
				found = true
				break
			}
		}
		if !found {
			values = append(values, "offline_access")
		}
	}
	return values
}

func (a *Authenticator) tenantAndClient() (string, string, error) {
	if a == nil || a.Options == nil {
		return "", "", &Error{Kind: ErrorInvalidInput, Detail: "ADO auth is not configured. Add auth to workflow.json or set DW_ADO_TOKEN."}
	}
	tenant, client := a.Options.TenantID, a.Options.ClientID
	if tenant == "" {
		tenant = DefaultTenantID
	}
	if client == "" {
		client = DefaultPublicClientID
	}
	return tenant, client, nil
}

func (a *Authenticator) SilentOrEnvironment(ctx context.Context) (*Token, error) {
	if token := EnvironmentToken(); token != nil {
		return token, nil
	}
	if a == nil || a.Options == nil {
		return nil, nil
	}
	refreshToken, ok, err := a.readRefreshToken(ctx)
	if err != nil || !ok {
		return nil, err
	}
	token, err := a.refresh(ctx, refreshToken)
	if err != nil {
		return nil, err
	}
	if token.RefreshToken != "" {
		if err := a.storeRefreshToken(ctx, token.RefreshToken); err != nil {
			return nil, err
		}
	}
	return a.tokenResult(token, "keyring"), nil
}

func (a *Authenticator) RequireToken(ctx context.Context) (Token, error) {
	token, err := a.SilentOrEnvironment(ctx)
	if err != nil {
		return Token{}, err
	}
	if token == nil {
		return Token{}, &Error{Kind: ErrorMissingAuth}
	}
	return *token, nil
}

func (a *Authenticator) Status(ctx context.Context) (AuthStatus, error) {
	token, err := a.SilentOrEnvironment(ctx)
	if err != nil {
		return AuthStatus{}, err
	}
	if token == nil {
		return AuthStatus{Connected: false}, nil
	}
	source := token.Source
	return AuthStatus{Connected: true, Source: &source, ExpiresOn: token.ExpiresOn}, nil
}

func (a *Authenticator) Logout(ctx context.Context) (bool, error) {
	return a.deleteStoredRefreshToken(ctx)
}

type oauthTokenResponse struct {
	AccessToken  string `json:"access_token"`
	RefreshToken string `json:"refresh_token"`
	ExpiresIn    uint32 `json:"expires_in"`
}

type oauthErrorResponse struct {
	Error            string `json:"error"`
	ErrorDescription string `json:"error_description"`
}

type deviceAuthorizationResponse struct {
	DeviceCode      string  `json:"device_code"`
	UserCode        string  `json:"user_code"`
	VerificationURI string  `json:"verification_uri"`
	ExpiresIn       uint32  `json:"expires_in"`
	Interval        *uint32 `json:"interval"`
}

func (a *Authenticator) LoginDeviceCode(ctx context.Context, onInstructions func(DeviceLoginInstructions)) (Token, error) {
	tenant, clientID, err := a.tenantAndClient()
	if err != nil {
		return Token{}, err
	}
	flowURL := "https://login.microsoftonline.com/" + tenant + "/oauth2/v2.0/devicecode"
	var flow deviceAuthorizationResponse
	if err := a.postOAuthForm(ctx, flowURL, url.Values{"client_id": {clientID}, "scope": {strings.Join(a.scopes(false), " ")}}, &flow); err != nil {
		return Token{}, err
	}
	if a.OpenURL != nil {
		_ = a.OpenURL(flow.VerificationURI)
	}
	intervalSeconds := uint32(5)
	if flow.Interval != nil {
		intervalSeconds = *flow.Interval
	}
	if intervalSeconds < 1 {
		intervalSeconds = 1
	}
	if onInstructions != nil {
		onInstructions(DeviceLoginInstructions{VerificationURI: flow.VerificationURI, UserCode: flow.UserCode, ExpiresInSeconds: flow.ExpiresIn, PollIntervalSeconds: intervalSeconds})
	}
	interval := time.Duration(intervalSeconds) * time.Second
	deadline := a.now().Add(time.Duration(flow.ExpiresIn) * time.Second)
	tokenURL := "https://login.microsoftonline.com/" + tenant + "/oauth2/v2.0/token"
	for {
		var token oauthTokenResponse
		err = a.postOAuthForm(ctx, tokenURL, url.Values{
			"client_id":   {clientID},
			"grant_type":  {"urn:ietf:params:oauth:grant-type:device_code"},
			"device_code": {flow.DeviceCode},
		}, &token)
		if err == nil {
			if token.RefreshToken != "" {
				if err := a.storeRefreshToken(ctx, token.RefreshToken); err != nil {
					return Token{}, err
				}
			}
			return *a.tokenResult(token, "device code"), nil
		}
		var adoErr *Error
		if !errors.As(err, &adoErr) || adoErr.Kind != ErrorOAuth {
			return Token{}, err
		}
		if strings.Contains(adoErr.Detail, "slow_down") {
			interval += 5 * time.Second
		} else if !strings.Contains(adoErr.Detail, "authorization_pending") && !strings.Contains(adoErr.Detail, "AADSTS70016") {
			return Token{}, err
		}
		if !a.now().Add(interval).Before(deadline) {
			return Token{}, &Error{Kind: ErrorLoginExpired}
		}
		timer := time.NewTimer(interval)
		select {
		case <-ctx.Done():
			timer.Stop()
			return Token{}, ctx.Err()
		case <-timer.C:
		}
	}
}

func (a *Authenticator) refresh(ctx context.Context, refreshToken string) (oauthTokenResponse, error) {
	tenant, clientID, err := a.tenantAndClient()
	if err != nil {
		return oauthTokenResponse{}, err
	}
	var token oauthTokenResponse
	err = a.postOAuthForm(ctx, "https://login.microsoftonline.com/"+tenant+"/oauth2/v2.0/token", url.Values{
		"client_id":     {clientID},
		"scope":         {strings.Join(a.scopes(false), " ")},
		"refresh_token": {refreshToken},
		"grant_type":    {"refresh_token"},
	}, &token)
	return token, err
}

func (a *Authenticator) tokenResult(token oauthTokenResponse, source string) *Token {
	expires := a.now().UTC().Add(time.Duration(token.ExpiresIn) * time.Second).Format(time.RFC3339)
	return &Token{AccessToken: token.AccessToken, Source: source, Scheme: AuthBearer, ExpiresOn: &expires}
}

func (a *Authenticator) postOAuthForm(ctx context.Context, endpoint string, form url.Values, target any) error {
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, endpoint, strings.NewReader(form.Encode()))
	if err != nil {
		return &Error{Kind: ErrorOAuth, Detail: err.Error(), Cause: err}
	}
	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	response, err := a.client().Do(req)
	if err != nil {
		return &Error{Kind: ErrorOAuth, Detail: err.Error(), Cause: err}
	}
	defer response.Body.Close()
	body, err := io.ReadAll(response.Body)
	if err != nil {
		return &Error{Kind: ErrorOAuth, Detail: err.Error(), Cause: err}
	}
	if response.StatusCode != http.StatusOK {
		return &Error{Kind: ErrorOAuth, Detail: oauthErrorMessage(body)}
	}
	if err := json.Unmarshal(body, target); err != nil {
		return &Error{Kind: ErrorOAuth, Detail: "Invalid OAuth response: " + err.Error(), Cause: err}
	}
	return nil
}

func oauthErrorMessage(body []byte) string {
	var value oauthErrorResponse
	if json.Unmarshal(body, &value) == nil && value.Error != "" {
		if value.ErrorDescription != "" {
			return value.Error + ": " + value.ErrorDescription
		}
		return value.Error
	}
	return string(body)
}

func openURL(value string) error {
	var command *exec.Cmd
	switch runtime.GOOS {
	case "windows":
		command = exec.Command("rundll32", "url.dll,FileProtocolHandler", value)
	case "linux":
		command = exec.Command("xdg-open", value)
	default:
		return &Error{Kind: ErrorBrowserLogin, Detail: "browser opening is unsupported on this platform"}
	}
	return command.Start()
}

func uint32Value(value uint32) string { return strconv.FormatUint(uint64(value), 10) }

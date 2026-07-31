package ado

import (
	"context"
	"errors"
	"io"
	"net"
	"net/http"
	"net/url"
	"strings"
	"testing"
	"time"
)

type httpDoerFunc func(*http.Request) (*http.Response, error)

func (function httpDoerFunc) Do(request *http.Request) (*http.Response, error) {
	return function(request)
}

func TestLoginDeviceCodeRequestsOfflineAccess(t *testing.T) {
	var scopes []string
	requests := 0
	authenticator := NewAuthenticator(&AuthOptions{TenantID: "tenant", ClientID: "client"}, nil)
	authenticator.OpenURL = nil
	authenticator.NewClient = func() HTTPDoer {
		return httpDoerFunc(func(request *http.Request) (*http.Response, error) {
			requests++
			body, err := io.ReadAll(request.Body)
			if err != nil {
				t.Fatal(err)
			}
			form, err := url.ParseQuery(string(body))
			if err != nil {
				t.Fatal(err)
			}
			if requests == 1 {
				scopes = strings.Fields(form.Get("scope"))
				return oauthResponse(http.StatusOK, `{"device_code":"device","user_code":"user","verification_uri":"https://example.test","expires_in":60,"interval":1}`), nil
			}
			return oauthResponse(http.StatusOK, `{"access_token":"access","expires_in":3600}`), nil
		})
	}

	if _, err := authenticator.LoginDeviceCode(context.Background(), nil); err != nil {
		t.Fatal(err)
	}
	if !containsString(scopes, "offline_access") {
		t.Fatalf("device authorization scopes = %q, want offline_access", scopes)
	}
}

func TestLoginBrowserClosesLoopbackServer(t *testing.T) {
	tests := []struct {
		name string
		run  func(*Authenticator, *string) error
	}{
		{
			name: "callback",
			run: func(authenticator *Authenticator, authorizationURL *string) error {
				authenticator.OpenURL = func(value string) error {
					*authorizationURL = value
					callbackURL := browserRedirectURI(t, value)
					callbackURL.RawQuery = url.Values{"state": {"invalid"}}.Encode()
					response, err := http.Get(callbackURL.String())
					if err == nil {
						response.Body.Close()
					}
					return err
				}
				_, err := authenticator.loginBrowser(context.Background(), time.Second)
				return err
			},
		},
		{
			name: "open URL error",
			run: func(authenticator *Authenticator, authorizationURL *string) error {
				authenticator.OpenURL = func(value string) error {
					*authorizationURL = value
					return errors.New("cannot open browser")
				}
				_, err := authenticator.loginBrowser(context.Background(), time.Second)
				return err
			},
		},
		{
			name: "cancellation",
			run: func(authenticator *Authenticator, authorizationURL *string) error {
				ctx, cancel := context.WithCancel(context.Background())
				authenticator.OpenURL = func(value string) error {
					*authorizationURL = value
					cancel()
					return nil
				}
				_, err := authenticator.loginBrowser(ctx, time.Second)
				return err
			},
		},
		{
			name: "timeout",
			run: func(authenticator *Authenticator, authorizationURL *string) error {
				authenticator.OpenURL = func(value string) error {
					*authorizationURL = value
					return nil
				}
				_, err := authenticator.loginBrowser(context.Background(), time.Millisecond)
				return err
			},
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			authenticator := NewAuthenticator(&AuthOptions{TenantID: "tenant", ClientID: "client"}, nil)
			var authorizationURL string
			if err := test.run(authenticator, &authorizationURL); err == nil {
				t.Fatal("loginBrowser returned no error")
			}
			redirectURI := browserRedirectURI(t, authorizationURL)
			listener, err := net.Listen("tcp4", "127.0.0.1:"+redirectURI.Port())
			if err != nil {
				t.Fatalf("callback port %s was not released: %v", redirectURI.Port(), err)
			}
			listener.Close()
		})
	}
}

func TestOAuthResponseBodiesAreLimited(t *testing.T) {
	authenticator := NewAuthenticator(&AuthOptions{}, nil)
	authenticator.NewClient = func() HTTPDoer {
		return httpDoerFunc(func(*http.Request) (*http.Response, error) {
			return oauthResponse(http.StatusOK, strings.Repeat("x", oauthResponseBodyLimit+1)), nil
		})
	}

	tests := []struct {
		name string
		post func() error
	}{
		{"device OAuth", func() error {
			return authenticator.postOAuthForm(context.Background(), "https://example.test/token", nil, &oauthTokenResponse{})
		}},
		{"browser OAuth", func() error {
			return authenticator.postBrowserTokenForm(context.Background(), "https://example.test/token", nil, &oauthTokenResponse{})
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			err := test.post()
			var adoError *Error
			if !errors.As(err, &adoError) || !strings.Contains(adoError.Detail, "1 MiB limit") {
				t.Fatalf("error = %#v, want explicit OAuth body limit", err)
			}
		})
	}
}

func browserRedirectURI(t *testing.T, authorizationURL string) *url.URL {
	t.Helper()
	parsed, err := url.Parse(authorizationURL)
	if err != nil {
		t.Fatal(err)
	}
	redirectURI, err := url.Parse(parsed.Query().Get("redirect_uri"))
	if err != nil || redirectURI.Port() == "" {
		t.Fatalf("redirect URI in %q is invalid: %v", authorizationURL, err)
	}
	return redirectURI
}

func oauthResponse(status int, body string) *http.Response {
	return &http.Response{StatusCode: status, Body: io.NopCloser(strings.NewReader(body))}
}

func containsString(values []string, expected string) bool {
	for _, value := range values {
		if value == expected {
			return true
		}
	}
	return false
}

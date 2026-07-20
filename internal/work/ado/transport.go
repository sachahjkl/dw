package ado

import (
	"bytes"
	"context"
	"encoding/base64"
	"encoding/json"
	"io"
	"net/http"
	"time"
)

type HTTPDoer interface {
	Do(*http.Request) (*http.Response, error)
}

const DefaultHTTPTimeout = 30 * time.Second

func newDefaultHTTPClient() HTTPDoer { return &http.Client{Timeout: DefaultHTTPTimeout} }

type Transport struct {
	NewClient func() HTTPDoer
}

func NewTransport() *Transport {
	return &Transport{NewClient: newDefaultHTTPClient}
}

func (t *Transport) client() HTTPDoer {
	if t != nil && t.NewClient != nil {
		return t.NewClient()
	}
	return newDefaultHTTPClient()
}

func authorizationHeader(token Token) string {
	if token.Scheme == AuthBasic {
		encoded := base64.StdEncoding.EncodeToString([]byte(":" + token.AccessToken))
		return "Basic " + encoded
	}
	return "Bearer " + token.AccessToken
}

func (t *Transport) Get(ctx context.Context, url string, token Token) (json.RawMessage, error) {
	return t.request(ctx, http.MethodGet, url, token, nil, "", false)
}

func (t *Transport) GetOptional404(ctx context.Context, url string, token Token) (json.RawMessage, bool, error) {
	body, err := t.request(ctx, http.MethodGet, url, token, nil, "", true)
	if err != nil {
		return nil, false, err
	}
	if body == nil {
		return nil, false, nil
	}
	return body, true, nil
}

func (t *Transport) Post(ctx context.Context, url string, token Token, body any) (json.RawMessage, error) {
	return t.request(ctx, http.MethodPost, url, token, body, "application/json", false)
}

func (t *Transport) PostWithContentType(ctx context.Context, url string, token Token, body any, contentType string) (json.RawMessage, error) {
	return t.request(ctx, http.MethodPost, url, token, body, contentType, false)
}

func (t *Transport) Patch(ctx context.Context, url string, token Token, body any, contentType string) (json.RawMessage, error) {
	return t.request(ctx, http.MethodPatch, url, token, body, contentType, false)
}

func (t *Transport) request(ctx context.Context, method, url string, token Token, body any, contentType string, optional404 bool) (json.RawMessage, error) {
	var encoded []byte
	var err error
	if body != nil {
		encoded, err = json.Marshal(body)
		if err != nil {
			return nil, &Error{Kind: ErrorJSON, Detail: err.Error(), Cause: err}
		}
	}
	req, err := http.NewRequestWithContext(ctx, method, url, bytes.NewReader(encoded))
	if err != nil {
		return nil, &Error{Kind: ErrorRequest, Detail: err.Error(), Cause: err}
	}
	req.Header.Set("Accept", "application/json")
	req.Header.Set("Authorization", authorizationHeader(token))
	if body != nil {
		req.Header.Set("Content-Type", contentType)
	}
	response, err := t.client().Do(req)
	if err != nil {
		return nil, &Error{Kind: ErrorRequest, Detail: err.Error(), Cause: err}
	}
	defer response.Body.Close()
	responseBody, err := io.ReadAll(response.Body)
	if err != nil {
		return nil, &Error{Kind: ErrorRequest, Detail: err.Error(), Cause: err}
	}
	if optional404 && response.StatusCode == http.StatusNotFound {
		return nil, nil
	}
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		return nil, &Error{Kind: ErrorHTTP, Status: response.StatusCode, Body: string(responseBody)}
	}
	if !json.Valid(responseBody) {
		var value any
		err = json.Unmarshal(responseBody, &value)
		return nil, &Error{Kind: ErrorJSON, Detail: err.Error(), Cause: err}
	}
	return json.RawMessage(responseBody), nil
}

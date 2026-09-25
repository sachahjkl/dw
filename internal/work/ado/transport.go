package ado

import (
	"bytes"
	"context"
	"encoding/base64"
	"encoding/json"
	"io"
	"net/http"
	"strings"
	"time"
)

type HTTPDoer interface {
	Do(*http.Request) (*http.Response, error)
}

const DefaultHTTPTimeout = 30 * time.Second
const maximumResponseBodyBytes = 8 << 20
const maximumErrorDetailLength = 500

var sharedHTTPClient = &http.Client{Timeout: DefaultHTTPTimeout}

func newDefaultHTTPClient() HTTPDoer { return sharedHTTPClient }

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
	body, _, err := t.request(ctx, http.MethodGet, url, token, nil, "", false)
	return body, err
}

func (t *Transport) GetOptional404(ctx context.Context, url string, token Token) (json.RawMessage, bool, error) {
	return t.request(ctx, http.MethodGet, url, token, nil, "", true)
}

func (t *Transport) Post(ctx context.Context, url string, token Token, body any) (json.RawMessage, error) {
	response, _, err := t.request(ctx, http.MethodPost, url, token, body, "application/json", false)
	return response, err
}

func (t *Transport) PostWithContentType(ctx context.Context, url string, token Token, body any, contentType string) (json.RawMessage, error) {
	response, _, err := t.request(ctx, http.MethodPost, url, token, body, contentType, false)
	return response, err
}

func (t *Transport) Patch(ctx context.Context, url string, token Token, body any, contentType string) (json.RawMessage, error) {
	response, _, err := t.request(ctx, http.MethodPatch, url, token, body, contentType, false)
	return response, err
}

func (t *Transport) request(ctx context.Context, method, url string, token Token, body any, contentType string, optional404 bool) (json.RawMessage, bool, error) {
	var encoded []byte
	var err error
	if body != nil {
		encoded, err = json.Marshal(body)
		if err != nil {
			return nil, false, &Error{Kind: ErrorJSON, Detail: err.Error(), Cause: err}
		}
	}
	req, err := http.NewRequestWithContext(ctx, method, url, bytes.NewReader(encoded))
	if err != nil {
		return nil, false, &Error{Kind: ErrorRequest, Detail: err.Error(), Cause: err}
	}
	req.Header.Set("Accept", "application/json")
	req.Header.Set("Authorization", authorizationHeader(token))
	if body != nil {
		req.Header.Set("Content-Type", contentType)
	}
	response, err := t.client().Do(req)
	if err != nil {
		return nil, false, &Error{Kind: ErrorRequest, Detail: err.Error(), Cause: err}
	}
	defer response.Body.Close()
	responseBody, err := io.ReadAll(io.LimitReader(response.Body, maximumResponseBodyBytes+1))
	if err != nil {
		return nil, false, &Error{Kind: ErrorRequest, Detail: err.Error(), Cause: err}
	}
	if len(responseBody) > maximumResponseBodyBytes {
		return nil, false, &Error{Kind: ErrorRequest, Detail: "response body exceeds 8 MiB"}
	}
	if optional404 && response.StatusCode == http.StatusNotFound {
		return nil, false, nil
	}
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		return nil, false, &Error{Kind: ErrorHTTP, Status: response.StatusCode, Body: httpErrorDetail(responseBody)}
	}
	if len(responseBody) == 0 {
		return nil, true, nil
	}
	if !json.Valid(responseBody) {
		var value any
		err = json.Unmarshal(responseBody, &value)
		return nil, false, &Error{Kind: ErrorJSON, Detail: err.Error(), Cause: err}
	}
	return json.RawMessage(responseBody), true, nil
}

func httpErrorDetail(body []byte) string {
	var payload struct {
		Message string `json:"message"`
		TypeKey string `json:"typeKey"`
	}
	detail := strings.TrimSpace(string(body))
	if json.Unmarshal(body, &payload) == nil && strings.TrimSpace(payload.Message) != "" {
		detail = strings.TrimSpace(payload.Message)
		if payload.TypeKey != "" {
			detail += " (" + payload.TypeKey + ")"
		}
	}
	return truncateDetail(detail, maximumErrorDetailLength)
}

func truncateDetail(value string, limit int) string {
	runes := []rune(value)
	if len(runes) <= limit {
		return value
	}
	return string(runes[:limit]) + "..."
}

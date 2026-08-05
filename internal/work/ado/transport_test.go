package ado

import (
	"context"
	"io"
	"net/http"
	"strings"
	"testing"
)

func TestTransportAcceptsSuccessfulEmptyResponse(t *testing.T) {
	transport := &Transport{NewClient: func() HTTPDoer {
		return httpDoerFunc(func(*http.Request) (*http.Response, error) {
			return &http.Response{StatusCode: http.StatusNoContent, Body: http.NoBody, Header: make(http.Header)}, nil
		})
	}}
	if _, err := transport.Patch(context.Background(), "https://example.test", Token{}, map[string]string{"state": "done"}, "application/json"); err != nil {
		t.Fatal(err)
	}
}

func TestTransportRejectsOversizedResponse(t *testing.T) {
	transport := &Transport{NewClient: func() HTTPDoer {
		return httpDoerFunc(func(*http.Request) (*http.Response, error) {
			body := io.NopCloser(strings.NewReader(strings.Repeat("x", maximumResponseBodyBytes+1)))
			return &http.Response{StatusCode: http.StatusOK, Body: body, Header: make(http.Header)}, nil
		})
	}}
	if _, err := transport.Get(context.Background(), "https://example.test", Token{}); err == nil {
		t.Fatal("oversized response was accepted")
	}
}

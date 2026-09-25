package ado

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
	"strconv"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/work"
)

type batchCall struct {
	IDs         []uint64 `json:"ids"`
	Fields      []string `json:"fields"`
	Expand      string   `json:"$expand"`
	ErrorPolicy string   `json:"errorPolicy"`
}

func batchServer(t *testing.T, inaccessible map[uint64]bool, parents map[uint64]uint64) (*httptest.Server, func() []batchCall) {
	t.Helper()
	var mu sync.Mutex
	calls := make([]batchCall, 0)
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		if !strings.HasSuffix(request.URL.Path, "/_apis/wit/workitemsbatch") || request.Method != http.MethodPost {
			http.NotFound(w, request)
			return
		}
		var call batchCall
		if err := json.NewDecoder(request.Body).Decode(&call); err != nil {
			t.Error(err)
		}
		mu.Lock()
		calls = append(calls, call)
		mu.Unlock()
		if len(call.IDs) > workItemsBatchLimit {
			w.WriteHeader(http.StatusBadRequest)
			_, _ = io.WriteString(w, `{"message":"VS403474: too many ids","typeKey":"VssPropertyValidationException"}`)
			return
		}
		values := make([]any, 0, len(call.IDs))
		for _, id := range call.IDs {
			if inaccessible[id] {
				values = append(values, nil)
				continue
			}
			item := map[string]any{"id": id, "fields": map[string]any{"System.Title": "Item " + strconv.FormatUint(id, 10), "System.WorkItemType": "Task", "System.State": "Active"}}
			if parent, found := parents[id]; found && call.Expand == "relations" {
				item["relations"] = []any{map[string]any{"rel": RelationHierarchyReverse, "url": "https://example.test/_apis/wit/workItems/" + strconv.FormatUint(parent, 10)}}
			}
			values = append(values, item)
		}
		_ = json.NewEncoder(w).Encode(map[string]any{"count": len(values), "value": values})
	}))
	t.Cleanup(server.Close)
	return server, func() []batchCall {
		mu.Lock()
		defer mu.Unlock()
		return append([]batchCall(nil), calls...)
	}
}

func TestGetWorkItemsBatchChunksAndOmitsInaccessibleItems(t *testing.T) {
	server, calls := batchServer(t, map[uint64]bool{7: true}, nil)
	provider := &Provider{Transport: NewTransport()}
	ids := make([]string, 0, 450)
	for id := 1; id <= 450; id++ {
		ids = append(ids, strconv.Itoa(id))
	}
	ids = append(ids, "not-a-number", "3")
	items, err := provider.GetWorkItemsBatch(context.Background(), Options{Organization: server.URL, Project: "p"}, ids, Token{})
	if err != nil {
		t.Fatal(err)
	}
	if len(items) != 449 {
		t.Fatalf("items = %d, want 449", len(items))
	}
	if items[0].ID != "1" || items[6].ID != "8" {
		t.Fatalf("items are not in request order: %s, %s", items[0].ID, items[6].ID)
	}
	recorded := calls()
	if len(recorded) != 3 {
		t.Fatalf("batch calls = %d, want 3", len(recorded))
	}
	total := 0
	for _, call := range recorded {
		if call.ErrorPolicy != "Omit" || len(call.Fields) == 0 || call.Expand != "" {
			t.Fatalf("unexpected batch request %+v", call)
		}
		total += len(call.IDs)
	}
	if total != 450 {
		t.Fatalf("requested ids = %d, want 450", total)
	}
}

func TestReadItemsWithRelationsUsesBatchExpand(t *testing.T) {
	server, calls := batchServer(t, nil, map[uint64]uint64{10: 1})
	provider := &Provider{Transport: NewTransport(), Options: Options{Organization: server.URL, Project: "p"}, Auth: &Authenticator{Options: &AuthOptions{}}}
	t.Setenv("DW_ADO_TOKEN", "pat")
	items, err := provider.ReadItems(context.Background(), work.ProjectRef{}, []work.ItemID{"10", "11"}, work.ReadOptions{IncludeRelations: true})
	if err != nil {
		t.Fatal(err)
	}
	if parent, ok := items[0].ParentID.Get(); !ok || parent != "1" {
		t.Fatalf("parent of 10 = %v, want 1", items[0].ParentID)
	}
	if _, ok := items[1].ParentID.Get(); ok {
		t.Fatal("item 11 has an unexpected parent")
	}
	for _, call := range calls() {
		if call.Expand != "relations" || len(call.Fields) != 0 {
			t.Fatalf("relations batch combined fields and expand: %+v", call)
		}
	}
}

func TestGroupWorkItemsByParentBatchesAndKeepsMissingParents(t *testing.T) {
	server, calls := batchServer(t, map[uint64]bool{99: true}, map[uint64]uint64{10: 1, 11: 1, 12: 99})
	provider := &Provider{Transport: NewTransport()}
	items := []WorkItemSnapshot{{ID: "10"}, {ID: "11"}, {ID: "12"}, {ID: "13"}}
	groups, err := provider.GroupWorkItemsByParent(context.Background(), Options{Organization: server.URL, Project: "p"}, items, Token{})
	if err != nil {
		t.Fatal(err)
	}
	if len(calls()) != 2 {
		t.Fatalf("batch calls = %d, want 2", len(calls()))
	}
	if len(groups) != 3 || groups[0].Parent.ID != "1" || len(groups[0].Items) != 2 || groups[1].Parent.ID != "13" || groups[2].Parent.ID != "" || groups[2].Items[0].ID != "12" {
		t.Fatalf("groups = %+v", groups)
	}
}

func TestHTTPErrorRendersADOMessage(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		w.WriteHeader(http.StatusNotFound)
		_, _ = io.WriteString(w, `{"message":"TF401232: Work item 7 does not exist`+strings.Repeat("x", 2000)+`","typeKey":"WorkItemUnauthorizedAccessException"}`)
	}))
	defer server.Close()
	_, err := NewTransport().Get(context.Background(), server.URL, Token{})
	var adoErr *Error
	if !errors.As(err, &adoErr) || adoErr.Status != http.StatusNotFound {
		t.Fatalf("error = %v", err)
	}
	message := err.Error()
	if !strings.Contains(message, "TF401232") || !strings.Contains(message, "404") || len(message) > 700 {
		t.Fatalf("error message = %q", message)
	}
	if strings.Contains(message, "typeKey") {
		t.Fatalf("raw JSON leaked into error: %q", message)
	}
}

func TestGetOptional404DistinguishesEmptySuccess(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		if request.URL.Path == "/missing" {
			w.WriteHeader(http.StatusNotFound)
			return
		}
		w.WriteHeader(http.StatusNoContent)
	}))
	defer server.Close()
	transport := NewTransport()
	if _, found, err := transport.GetOptional404(context.Background(), server.URL+"/empty", Token{}); err != nil || !found {
		t.Fatalf("empty success found=%v err=%v", found, err)
	}
	if _, found, err := transport.GetOptional404(context.Background(), server.URL+"/missing", Token{}); err != nil || found {
		t.Fatalf("404 found=%v err=%v", found, err)
	}
}

func TestExtractWorkItemIDsFromCommitMessages(t *testing.T) {
	log := strings.Join([]string{
		"fix(#59012 #59015): handle nulls",
		"bug(#59104) regression",
		"#58536 Bypass cache",
		"Merged PR 24693: feature #60001",
		"feat: AB#61234 link",
		"style: color #123456; background: #abcdef",
		"Update README (#42)",
		"refs #123456789 too long",
		"html &#1234; entity, issue#77, #12ab",
		"chore: #7",
	}, "\n")
	got := ExtractWorkItemIDsFromCommitMessages(log)
	want := []string{"59012", "59015", "59104", "58536", "60001", "61234", "7"}
	if strings.Join(got, ",") != strings.Join(want, ",") {
		t.Fatalf("ids = %v, want %v", got, want)
	}
}

type memoryStore struct {
	mu     sync.Mutex
	values map[contract.SecretKey]string
	fail   bool
}

func (s *memoryStore) Get(_ context.Context, key contract.SecretKey) (contract.SecretValue, bool, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	value, found := s.values[key]
	return contract.NewSecretValue(value), found, nil
}

func (s *memoryStore) Set(_ context.Context, key contract.SecretKey, value contract.SecretValue) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.fail {
		return errors.New("store is read-only")
	}
	s.values[key] = value.Reveal()
	return nil
}

func (s *memoryStore) Delete(_ context.Context, key contract.SecretKey) (bool, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	_, found := s.values[key]
	delete(s.values, key)
	return found, nil
}

func refreshingAuthenticator(store *memoryStore, refreshes *int, mu *sync.Mutex) *Authenticator {
	authenticator := NewAuthenticator(&AuthOptions{TenantID: "tenant/with space", ClientID: "client"}, store)
	authenticator.Lock = nil
	authenticator.NewClient = func() HTTPDoer {
		return httpDoerFunc(func(request *http.Request) (*http.Response, error) {
			if !strings.Contains(request.URL.EscapedPath(), "tenant%2Fwith%20space") {
				return oauthResponse(http.StatusBadRequest, `{"error":"bad tenant"}`), nil
			}
			mu.Lock()
			*refreshes++
			mu.Unlock()
			return oauthResponse(http.StatusOK, `{"access_token":"access","refresh_token":"rotated","expires_in":3600}`), nil
		})
	}
	return authenticator
}

func TestAccessTokenIsCachedUntilExpiry(t *testing.T) {
	accessTokens.clear()
	t.Cleanup(accessTokens.clear)
	t.Setenv("DW_ADO_TOKEN", "")
	t.Setenv("AZURE_DEVOPS_EXT_PAT", "")
	store := &memoryStore{values: map[contract.SecretKey]string{KeyringAccount: "refresh"}}
	var mu sync.Mutex
	refreshes := 0
	now := time.Now()
	authenticator := refreshingAuthenticator(store, &refreshes, &mu)
	authenticator.Now = func() time.Time { return now }
	provider := &Provider{Options: Options{Organization: "https://dev.azure.com/org", Project: "p"}, Auth: authenticator}
	var group sync.WaitGroup
	for range 8 {
		group.Add(1)
		go func() {
			defer group.Done()
			if _, _, err := provider.session(context.Background(), work.ProjectRef{}); err != nil {
				t.Error(err)
			}
		}()
	}
	group.Wait()
	if refreshes != 1 {
		t.Fatalf("refreshes = %d, want 1", refreshes)
	}
	now = now.Add(59*time.Minute + time.Second)
	if _, _, err := provider.session(context.Background(), work.ProjectRef{}); err != nil {
		t.Fatal(err)
	}
	if refreshes != 2 {
		t.Fatalf("refreshes after expiry skew = %d, want 2", refreshes)
	}
}

func TestRefreshTokenSaveFailureWarns(t *testing.T) {
	t.Setenv("DW_ADO_TOKEN", "")
	t.Setenv("AZURE_DEVOPS_EXT_PAT", "")
	store := &memoryStore{values: map[contract.SecretKey]string{KeyringAccount: "refresh"}, fail: true}
	var mu sync.Mutex
	refreshes := 0
	authenticator := refreshingAuthenticator(store, &refreshes, &mu)
	warned := 0
	authenticator.Warn = func(l10n.Message) { warned++ }
	token, err := authenticator.RequireToken(context.Background())
	if err != nil || token.AccessToken != "access" {
		t.Fatalf("token = %+v, err = %v", token, err)
	}
	if warned != 1 {
		t.Fatalf("warnings = %d, want 1", warned)
	}
}

func TestFileLockSerializesAndRecoversStaleLocks(t *testing.T) {
	path := t.TempDir() + "/refresh.lock"
	release, err := acquireFileLock(context.Background(), path, time.Now)
	if err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 150*time.Millisecond)
	defer cancel()
	if _, err := acquireFileLock(ctx, path, time.Now); !errors.Is(err, context.DeadlineExceeded) {
		t.Fatalf("second acquire error = %v, want deadline", err)
	}
	stale, err := acquireFileLock(context.Background(), path, func() time.Time { return time.Now().Add(refreshTokenLockStale + time.Minute) })
	if err != nil {
		t.Fatal(err)
	}
	stale()
	release()
}

func TestBrowserCallbackIgnoresUnrelatedRequests(t *testing.T) {
	authenticator := NewAuthenticator(&AuthOptions{TenantID: "tenant", ClientID: "client"}, nil)
	statuses := make([]int, 0)
	done := make(chan struct{})
	authenticator.OpenURL = func(value string) error {
		callback := browserRedirectURI(t, value)
		base := "http://" + callback.Host
		requests := []string{base + "/favicon.ico", base + "/?state=wrong&code=x", base + "/?error=access_denied&error_description=denied"}
		go func() {
			defer close(done)
			for _, target := range requests {
				response, err := http.Get(target)
				if err != nil {
					return
				}
				statuses = append(statuses, response.StatusCode)
				response.Body.Close()
			}
		}()
		return nil
	}
	started := time.Now()
	_, err := authenticator.loginBrowser(context.Background(), 5*time.Second, true, nil)
	var adoErr *Error
	if !errors.As(err, &adoErr) || adoErr.Kind != ErrorBrowserLogin || !strings.Contains(adoErr.Detail, "denied") {
		t.Fatalf("error = %v, want explicit browser error", err)
	}
	if time.Since(started) > 4*time.Second {
		t.Fatal("explicit OAuth error did not end login")
	}
	<-done
	if len(statuses) < 2 || statuses[0] != http.StatusNotFound || statuses[1] != http.StatusBadRequest {
		t.Fatalf("statuses = %v", statuses)
	}
}

func TestOpenURLRejectsNonHTTPS(t *testing.T) {
	for _, value := range []string{"file:///etc/passwd", "http://example.test", "calc.exe"} {
		if err := openURL(value); err == nil {
			t.Fatalf("openURL(%q) accepted", value)
		}
	}
}

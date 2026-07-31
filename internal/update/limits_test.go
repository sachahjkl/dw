package update

import (
	"archive/tar"
	"archive/zip"
	"bytes"
	"compress/gzip"
	"context"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestParseManifestRejectsOversizedInput(t *testing.T) {
	_, err := ParseManifest(io.LimitReader(zeroReader{}, maxManifestSize+1))
	if err == nil || !strings.Contains(err.Error(), "manifest-too-large") {
		t.Fatalf("ParseManifest error = %v, want manifest-too-large", err)
	}
}

func TestDiscoverReleaseRejectsOversizedContentLength(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(response http.ResponseWriter, _ *http.Request) {
		response.Header().Set("Content-Length", "2097153")
		response.WriteHeader(http.StatusOK)
	}))
	defer server.Close()
	service := NewService(server.Client())
	service.APIBaseURL = server.URL
	_, err := service.DiscoverRelease(context.Background(), Config{Owner: "owner", Repository: "repository"})
	if err == nil || !strings.Contains(err.Error(), "github-response-too-large") {
		t.Fatalf("DiscoverRelease error = %v, want github-response-too-large", err)
	}
}

func TestDiscoverReleaseRejectsOversizedChunkedResponse(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(response http.ResponseWriter, _ *http.Request) {
		response.(http.Flusher).Flush()
		_, _ = io.Copy(response, io.LimitReader(zeroReader{}, maxGitHubResponseSize+1))
	}))
	defer server.Close()
	service := NewService(server.Client())
	service.APIBaseURL = server.URL
	_, err := service.DiscoverRelease(context.Background(), Config{Owner: "owner", Repository: "repository"})
	if err == nil || !strings.Contains(err.Error(), "github-response-too-large") {
		t.Fatalf("DiscoverRelease error = %v, want github-response-too-large", err)
	}
}

func TestDownloadAssetRejectsOversizedContentLength(t *testing.T) {
	service := NewService(&http.Client{Transport: roundTripFunc(func(*http.Request) (*http.Response, error) {
		return &http.Response{StatusCode: http.StatusOK, ContentLength: maxAssetSize + 1, Body: io.NopCloser(bytes.NewReader(nil))}, nil
	})})
	_, err := service.downloadAsset(context.Background(), Asset{URL: "https://example.test/dw", FileName: "dw"}, nil)
	if err == nil || !strings.Contains(err.Error(), "asset-too-large") {
		t.Fatalf("downloadAsset error = %v, want asset-too-large", err)
	}
}

func TestPrepareReplacementRejectsArchiveEntryLimit(t *testing.T) {
	archivePath := filepath.Join(t.TempDir(), "dw-linux-x64.tar.gz")
	file, err := os.Create(archivePath)
	if err != nil {
		t.Fatal(err)
	}
	gz := gzip.NewWriter(file)
	tw := tar.NewWriter(gz)
	for index := 0; index <= maxArchiveEntries; index++ {
		if err := tw.WriteHeader(&tar.Header{Name: strings.Repeat("x", index%2+1), Mode: 0o644, Size: 0}); err != nil {
			t.Fatal(err)
		}
	}
	if err := tw.Close(); err != nil {
		t.Fatal(err)
	}
	if err := gz.Close(); err != nil {
		t.Fatal(err)
	}
	if err := file.Close(); err != nil {
		t.Fatal(err)
	}
	_, err = extractUnixExecutable(archivePath, "linux-x64", t.TempDir())
	if err == nil || !strings.Contains(err.Error(), "archive-too-many-entries") {
		t.Fatalf("extractUnixExecutable error = %v, want archive-too-many-entries", err)
	}
}

func TestPrepareReplacementRejectsDecompressedSizeLimit(t *testing.T) {
	archivePath := filepath.Join(t.TempDir(), "dw-win-x64.zip")
	file, err := os.Create(archivePath)
	if err != nil {
		t.Fatal(err)
	}
	zw := zip.NewWriter(file)
	header := &zip.FileHeader{Name: "dw.exe", Method: zip.Store, UncompressedSize64: uint64(maxArchiveSize + 1), CompressedSize64: 0}
	if _, err := zw.CreateRaw(header); err != nil {
		t.Fatal(err)
	}
	if err := zw.Close(); err != nil {
		t.Fatal(err)
	}
	if err := file.Close(); err != nil {
		t.Fatal(err)
	}
	_, err = extractWindowsExecutable(archivePath, t.TempDir())
	if err == nil || !strings.Contains(err.Error(), "archive-too-large") {
		t.Fatalf("extractWindowsExecutable error = %v, want archive-too-large", err)
	}
}

type zeroReader struct{}

func (zeroReader) Read(buffer []byte) (int, error) {
	for index := range buffer {
		buffer[index] = 0
	}
	return len(buffer), nil
}

type roundTripFunc func(*http.Request) (*http.Response, error)

func (function roundTripFunc) RoundTrip(request *http.Request) (*http.Response, error) {
	return function(request)
}

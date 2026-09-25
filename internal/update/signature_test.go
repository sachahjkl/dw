package update

import (
	"bytes"
	"context"
	"net/http"
	"net/http/httptest"
	"os"
	"strings"
	"testing"
)

const testSigningKey = "RWSFwkYBoW3Sb3h1AHkkSVyF7v4380suC19y0EE/57IV7DK+yelbyugT"

func readSignatureFixtures(t *testing.T) ([]byte, []byte) {
	t.Helper()
	manifest, err := os.ReadFile("testdata/release.json")
	if err != nil {
		t.Fatal(err)
	}
	signature, err := os.ReadFile("testdata/release.json.minisig")
	if err != nil {
		t.Fatal(err)
	}
	return manifest, signature
}

func TestVerifyManifestSignatureAcceptsMinisignSignature(t *testing.T) {
	manifest, signature := readSignatureFixtures(t)
	if err := VerifyManifestSignature(manifest, signature, []string{testSigningKey}); err != nil {
		t.Fatal(err)
	}
}

func TestVerifyManifestSignatureRejectsTampering(t *testing.T) {
	manifest, signature := readSignatureFixtures(t)
	tampered := bytes.Replace(manifest, []byte("1.2.3"), []byte("9.9.9"), 1)
	if err := VerifyManifestSignature(tampered, signature, []string{testSigningKey}); err == nil {
		t.Fatal("tampered manifest accepted")
	}
	commentTampered := bytes.Replace(signature, []byte("trusted comment: dw test"), []byte("trusted comment: evil"), 1)
	if err := VerifyManifestSignature(manifest, commentTampered, []string{testSigningKey}); err == nil {
		t.Fatal("tampered trusted comment accepted")
	}
}

func TestVerifyManifestSignatureRejectsUntrustedKey(t *testing.T) {
	manifest, signature := readSignatureFixtures(t)
	err := VerifyManifestSignature(manifest, signature, TrustedPublicKeys)
	if err == nil || !strings.Contains(err.Error(), "untrusted-key") {
		t.Fatalf("err = %v, want untrusted key", err)
	}
}

func TestTrustedPublicKeysParse(t *testing.T) {
	for _, key := range TrustedPublicKeys {
		if _, err := parseMinisignPublicKey(key); err != nil {
			t.Fatalf("%s: %v", key, err)
		}
	}
}

func TestFetchManifestRequiresValidSignature(t *testing.T) {
	manifest, signature := readSignatureFixtures(t)
	files := map[string][]byte{"/release.json": manifest, "/release.json.minisig": signature}
	server := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		contents, ok := files[request.URL.Path]
		if !ok {
			http.NotFound(writer, request)
			return
		}
		_, _ = writer.Write(contents)
	}))
	defer server.Close()
	release := GitHubRelease{TagName: "v1.2.3", Assets: []GitHubAsset{
		{Name: "release.json", URL: server.URL + "/release.json"},
		{Name: "release.json.minisig", URL: server.URL + "/release.json.minisig"},
	}}
	service := &Service{HTTPClient: server.Client(), TrustedPublicKeys: []string{testSigningKey}}
	parsed, err := service.FetchManifest(context.Background(), release, "release.json")
	if err != nil {
		t.Fatal(err)
	}
	if parsed.Version != "1.2.3" {
		t.Fatalf("version = %q", parsed.Version)
	}

	files["/release.json"] = bytes.Replace(manifest, []byte("1.2.3"), []byte("6.6.6"), 1)
	if _, err := service.FetchManifest(context.Background(), release, "release.json"); err == nil {
		t.Fatal("tampered manifest accepted")
	}

	unsigned := GitHubRelease{TagName: "v1.2.3", Assets: release.Assets[:1]}
	if _, err := service.FetchManifest(context.Background(), unsigned, "release.json"); err == nil {
		t.Fatal("unsigned release accepted")
	}
}

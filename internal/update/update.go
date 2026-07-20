package update

import (
	"context"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/platform"
)

func NewService(client *http.Client) *Service {
	if client == nil {
		client = &http.Client{Timeout: 30 * time.Second}
	}
	return &Service{HTTPClient: client, APIBaseURL: DefaultGitHubAPI, UserAgent: "dw/1.0"}
}

type NixInstallationError struct{}

func (*NixInstallationError) Error() string { return "update.nix-installation-unsupported" }
func (*NixInstallationError) Localized() l10n.Message {
	return l10n.M("upgrade.error.nix-installation")
}

// HostRIDMismatchError rejects a release asset built for another operating system.
type HostRIDMismatchError struct {
	Requested string
	Host      string
}

func (err *HostRIDMismatchError) Error() string {
	return "update.foreign-rid:" + err.Requested + ":" + err.Host
}
func (err *HostRIDMismatchError) Localized() l10n.Message {
	return l10n.M("upgrade.error.rid-mismatch", l10n.A("requested", err.Requested), l10n.A("host", err.Host))
}

// ResolveInstallRID returns the native published RID and rejects every foreign
// override. Callers must use it before acquiring or preparing an install asset.
func ResolveInstallRID(requested string) (string, error) {
	hostRID, err := platform.RuntimeIdentifier()
	if err != nil {
		return "", err
	}
	if requested == "" {
		return hostRID, nil
	}
	rid, err := platform.NormalizeRuntimeIdentifier(requested)
	if err != nil {
		return "", err
	}
	if rid != hostRID {
		return "", &HostRIDMismatchError{Requested: rid, Host: hostRID}
	}
	return rid, nil
}

func EnsureSupportedHost(executablePath string) error {
	if strings.Contains(filepath.ToSlash(executablePath), "/nix/store/") {
		return &NixInstallationError{}
	}
	return nil
}

func (service *Service) Run(ctx context.Context, request Request, emit EmitFunc) (Report, error) {
	emitEvent(emit, Event{Kind: "checking-host"})
	executable := request.ExecutablePath
	if executable == "" {
		var err error
		executable, err = os.Executable()
		if err != nil {
			return Report{}, fmt.Errorf("update: resolve-current-executable: %w", err)
		}
	}
	if err := EnsureSupportedHost(executable); err != nil {
		return Report{}, err
	}
	installRID := ""
	if !request.Check {
		var err error
		installRID, err = ResolveInstallRID(request.RID)
		if err != nil {
			return Report{}, err
		}
	}

	emitEvent(emit, Event{Kind: "resolving-config"})
	config, err := normalizeConfig(request.Config)
	if err != nil {
		return Report{}, err
	}
	emitEvent(emit, Event{Kind: "fetching-release", Owner: config.Owner, Repository: config.Repository})
	release, err := service.DiscoverRelease(ctx, config)
	if err != nil {
		return Report{}, err
	}
	emitEvent(emit, Event{Kind: "fetching-manifest", AssetName: config.AssetName})
	manifest, err := service.FetchManifest(ctx, release, config.AssetName)
	if err != nil {
		return Report{}, err
	}

	if request.Check {
		assets := make([]AssetSummary, 0, len(manifest.Assets))
		for _, asset := range manifest.Assets {
			assets = append(assets, AssetSummary{RID: asset.RID, FileName: asset.FileName, SHA256: asset.SHA256})
		}
		emitEvent(emit, Event{Kind: "completed", Version: manifest.Version})
		return Report{Kind: "check", Check: &CheckReport{ReleaseTag: release.TagName, Version: manifest.Version, Commit: manifest.Commit, Assets: assets}}, nil
	}

	rid := installRID
	emitEvent(emit, Event{Kind: "selecting-asset", RID: rid})
	asset, err := selectAsset(manifest, rid)
	if err != nil {
		return Report{}, err
	}
	if strings.TrimSpace(asset.URL) == "" {
		return Report{}, fmt.Errorf("update: manifest-asset-url-required")
	}

	emitEvent(emit, Event{Kind: "downloading-asset", FileName: asset.FileName})
	downloaded, err := service.downloadAsset(ctx, asset, emit)
	if err != nil {
		return Report{}, err
	}
	defer os.Remove(downloaded)
	emitEvent(emit, Event{Kind: "verifying-checksum", FileName: asset.FileName, ExpectedSHA256: asset.SHA256})
	digest, err := FileSHA256(downloaded)
	if err != nil {
		return Report{}, err
	}
	if !strings.EqualFold(digest, asset.SHA256) {
		return Report{}, fmt.Errorf("update: invalid-sha256 expected=%s actual=%s file=%s", asset.SHA256, digest, downloaded)
	}

	emitEvent(emit, Event{Kind: "preparing-executable", FileName: asset.FileName, RID: rid})
	replacement, err := PrepareReplacement(asset.FileName, downloaded, rid, service.TempDir)
	if err != nil {
		return Report{}, err
	}
	defer os.Remove(replacement)
	emitEvent(emit, Event{Kind: "replacing-executable", ExecutablePath: executable})
	replaced, err := platform.ReplaceExecutable(executable, replacement)
	if err != nil {
		return Report{}, err
	}
	emitEvent(emit, Event{Kind: "completed", Version: manifest.Version})
	return Report{Kind: "installed", Installed: &InstallReport{
		Version:                    manifest.Version,
		Commit:                     manifest.Commit,
		ExecutablePath:             replaced.ExecutablePath,
		DeferredWindowsReplacement: replaced.DeferredWindowsReplacement,
	}}, nil
}

func selectAsset(manifest Manifest, rid string) (Asset, error) {
	for _, asset := range manifest.Assets {
		if strings.EqualFold(asset.RID, rid) {
			return asset, nil
		}
	}
	return Asset{}, fmt.Errorf("update: no-asset-for-rid %q", rid)
}

func (service *Service) downloadAsset(ctx context.Context, asset Asset, emit EmitFunc) (fileName string, resultErr error) {
	response, err := service.doGET(ctx, asset.URL)
	if err != nil {
		return "", err
	}
	defer response.Body.Close()
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		return "", fmt.Errorf("update: asset-http-%d", response.StatusCode)
	}
	tempDir := service.TempDir
	if tempDir == "" {
		tempDir = os.TempDir()
	}
	output, err := os.CreateTemp(tempDir, "dw-upgrade-*"+extensionSuffix(asset.FileName))
	if err != nil {
		return "", fmt.Errorf("update: create-download: %w", err)
	}
	fileName = output.Name()
	defer func() {
		if resultErr != nil {
			_ = output.Close()
			_ = os.Remove(fileName)
		}
	}()
	var total *int64
	if response.ContentLength >= 0 {
		value := response.ContentLength
		total = &value
	}
	buffer := make([]byte, 64*1024)
	var received int64
	for {
		count, readErr := response.Body.Read(buffer)
		if count > 0 {
			written, writeErr := output.Write(buffer[:count])
			if writeErr != nil {
				return "", fmt.Errorf("update: write-download: %w", writeErr)
			}
			if written != count {
				return "", fmt.Errorf("update: short-download-write")
			}
			received += int64(count)
			emitEvent(emit, Event{Kind: "downloaded-asset-bytes", FileName: asset.FileName, Received: received, Total: total})
		}
		if readErr == io.EOF {
			break
		}
		if readErr != nil {
			return "", fmt.Errorf("update: read-download: %w", readErr)
		}
	}
	if err := output.Sync(); err != nil {
		return "", fmt.Errorf("update: sync-download: %w", err)
	}
	if err := output.Close(); err != nil {
		return "", fmt.Errorf("update: close-download: %w", err)
	}
	return fileName, nil
}

func emitEvent(emit EmitFunc, event Event) {
	if emit != nil {
		emit(event)
	}
}

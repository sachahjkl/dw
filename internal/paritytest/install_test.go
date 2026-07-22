package paritytest_test

import (
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func TestUnixInstallerNormalizesReleaseTag(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("Unix installer test requires Unix path semantics")
	}
	root := repositoryRoot(t)
	installer := filepath.Join(root, "scripts", "install.sh")
	for _, test := range []struct {
		version string
		wantTag string
	}{
		{version: "2026.07.20.1", wantTag: "v2026.07.20.1"},
		{version: "v2026.07.20.1", wantTag: "v2026.07.20.1"},
	} {
		t.Run(test.version, func(t *testing.T) {
			temp := t.TempDir()
			bin := filepath.Join(temp, "bin")
			if err := os.Mkdir(bin, 0o755); err != nil {
				t.Fatal(err)
			}
			writeExecutable(t, filepath.Join(bin, "curl"), `#!/bin/sh
for argument do
  case "$argument" in
    https://*) printf '%s' "$argument" > "$DW_INSTALL_CAPTURE" ;;
  esac
done
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-o" ]; then
    : > "$2"
    break
  fi
  shift
done
`)
			writeExecutable(t, filepath.Join(bin, "tar"), `#!/bin/sh
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-C" ]; then
    install_dir="$2"
    break
  fi
  shift
done
cat > "$install_dir/dw" <<'EOF'
#!/bin/sh
printf 'dw test\n'
EOF
`)
			capture := filepath.Join(temp, "url")
			installDir := filepath.Join(temp, "install")
			command := exec.Command("sh", installer, "--version", test.version, "--install-dir", installDir, "--no-path-update")
			command.Env = append(os.Environ(), "PATH="+bin+string(os.PathListSeparator)+os.Getenv("PATH"), "DW_INSTALL_CAPTURE="+capture)
			if output, err := command.CombinedOutput(); err != nil {
				t.Fatalf("installer failed: %v\n%s", err, output)
			}
			url, err := os.ReadFile(capture)
			if err != nil {
				t.Fatal(err)
			}
			want := "https://github.com/sachahjkl/dw/releases/download/" + test.wantTag + "/dw-linux-x64.tar.gz"
			if string(url) != want {
				t.Fatalf("asset URL = %q, want %q", url, want)
			}
		})
	}
}

func TestPowerShellInstallerNormalizesReleaseTag(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("the test double uses the local Unix executable format")
	}
	pwsh, err := exec.LookPath("pwsh")
	if err != nil {
		t.Skip("PowerShell is unavailable")
	}
	root := repositoryRoot(t)
	installer := filepath.Join(root, "scripts", "install.ps1")
	wrapper := filepath.Join(t.TempDir(), "invoke-installer.ps1")
	if err := os.WriteFile(wrapper, []byte(`function Invoke-WebRequest {
    param([string]$Uri, [string]$OutFile, [hashtable]$Headers)
    Set-Content -LiteralPath $env:DW_INSTALL_CAPTURE -Value $Uri -NoNewline
    New-Item -ItemType File -Force -Path $OutFile | Out-Null
}
function Expand-Archive {
    param([string]$LiteralPath, [string]$DestinationPath, [switch]$Force)
    Copy-Item -LiteralPath /bin/true -Destination (Join-Path $DestinationPath "dw.exe")
}
& $env:DW_INSTALL_SCRIPT -Version $env:DW_INSTALL_VERSION -InstallDir $env:DW_INSTALL_DIR -NoPathUpdate
`), 0o644); err != nil {
		t.Fatal(err)
	}
	for _, version := range []string{"2026.07.20.1", "v2026.07.20.1"} {
		t.Run(version, func(t *testing.T) {
			temp := t.TempDir()
			capture := filepath.Join(temp, "url")
			command := exec.Command(pwsh, "-NoProfile", "-File", wrapper)
			command.Env = append(os.Environ(),
				"DW_INSTALL_SCRIPT="+installer,
				"DW_INSTALL_VERSION="+version,
				"DW_INSTALL_DIR="+filepath.Join(temp, "install"),
				"DW_INSTALL_CAPTURE="+capture,
			)
			if output, err := command.CombinedOutput(); err != nil {
				t.Fatalf("PowerShell installer failed: %v\n%s", err, output)
			}
			url, err := os.ReadFile(capture)
			if err != nil {
				t.Fatal(err)
			}
			want := "https://github.com/sachahjkl/dw/releases/download/v2026.07.20.1/dw-win-x64.zip"
			if strings.TrimSpace(string(url)) != want {
				t.Fatalf("asset URL = %q, want %q", url, want)
			}
		})
	}
}

func repositoryRoot(t *testing.T) string {
	t.Helper()
	workingDirectory, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	return filepath.Clean(filepath.Join(workingDirectory, "..", ".."))
}

func writeExecutable(t *testing.T, path, content string) {
	t.Helper()
	if err := os.WriteFile(path, []byte(content), 0o755); err != nil {
		t.Fatal(err)
	}
}

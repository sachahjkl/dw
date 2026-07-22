package complete

import (
	"os"
	"os/exec"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
)

func TestBashCompletionPreservesQuotedArguments(t *testing.T) {
	if _, err := exec.LookPath("bash"); err != nil {
		t.Skip("bash is unavailable")
	}
	installed, err := Install(Bash)
	if err != nil {
		t.Fatal(err)
	}
	capture := filepath.Join(t.TempDir(), "arguments")
	script := `complete() { :; }
dw() {
  printf '%s\n' "$@" > "$DW_COMPLETION_CAPTURE"
  printf '%s\n' candidate
}
` + installed + `
COMP_WORDS=(dw workspace pr start --repo "platform/front end")
COMP_CWORD=5
_dw_complete
printf '%s\n' "${COMPREPLY[@]}"
`
	command := exec.Command("bash", "-c", script)
	command.Env = append(os.Environ(), "DW_COMPLETION_CAPTURE="+capture)
	output, err := command.CombinedOutput()
	if err != nil {
		t.Fatalf("bash completion failed: %v\n%s", err, output)
	}
	if got := strings.TrimSpace(string(output)); got != "candidate" {
		t.Fatalf("COMPREPLY = %q, want candidate", got)
	}
	captured, err := os.ReadFile(capture)
	if err != nil {
		t.Fatal(err)
	}
	got := strings.Split(strings.TrimSuffix(string(captured), "\n"), "\n")
	want := []string{"completion", "complete", "--format", "bash", "--", "workspace", "pr", "start", "--repo", "platform/front end"}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("completion argv = %#v, want %#v", got, want)
	}
}

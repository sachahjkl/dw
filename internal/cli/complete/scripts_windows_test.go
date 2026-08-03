//go:build windows

package complete

import (
	"encoding/json"
	"os/exec"
	"reflect"
	"strings"
	"testing"
)

func TestPowerShellCompletionPreservesQuotedArguments(t *testing.T) {
	installed, err := Install(PowerShell)
	if err != nil {
		t.Fatal(err)
	}
	script := `
function Register-ArgumentCompleter {
  param([switch]$Native, [string]$CommandName, [scriptblock]$ScriptBlock)
  $global:dwCompleter = $ScriptBlock
}
function dw {
  $global:dwArguments = @($args)
  '[{"label":"candidate","description":"description"}]'
}
` + installed + `
$tokens = $null
$errors = $null
$scriptAst = [System.Management.Automation.Language.Parser]::ParseInput('dw workspace pr start --repo "platform/front end"', [ref]$tokens, [ref]$errors)
$commandAst = $scriptAst.EndBlock.Statements[0].PipelineElements[0]
$result = @(& $global:dwCompleter 'platform/front end' $commandAst $commandAst.Extent.EndOffset)
$result.CompletionText
Write-Output ('CAPTURE:' + ($global:dwArguments | ConvertTo-Json -Compress))
`
	command := exec.Command("powershell.exe", "-NoLogo", "-NoProfile", "-NonInteractive", "-Command", script)
	output, err := command.CombinedOutput()
	if err != nil {
		t.Fatalf("PowerShell completion failed: %v\n%s", err, output)
	}
	lines := strings.Split(strings.TrimSpace(string(output)), "\n")
	if got := strings.TrimSpace(lines[0]); got != "candidate" {
		t.Fatalf("completion = %q, want candidate", got)
	}
	if len(lines) != 2 || !strings.HasPrefix(strings.TrimSpace(lines[1]), "CAPTURE:") {
		t.Fatalf("unexpected PowerShell output: %q", output)
	}
	var got []string
	if err := json.Unmarshal([]byte(strings.TrimPrefix(strings.TrimSpace(lines[1]), "CAPTURE:")), &got); err != nil {
		t.Fatalf("decode captured arguments: %v\n%s", err, output)
	}
	want := []string{"completion", "complete", "--format", "json", "--", "workspace", "pr", "start", "--repo", `"platform/front end"`}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("completion argv = %#v, want %#v", got, want)
	}
}

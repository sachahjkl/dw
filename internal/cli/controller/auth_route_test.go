package controller

import (
	"testing"

	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/workapp"
)

func TestProviderLoginUsesEnvironmentModeWithoutInteractiveTerminal(t *testing.T) {
	request := workapp.AuthLoginRequest{Provider: "azure-devops"}
	for _, streams := range []console.Streams{
		{StdinTTY: false, StderrTTY: true},
		{StdinTTY: true, StderrTTY: false},
	} {
		resolved := applyCLIDefaults("provider.auth.login", request, console.Policy{Streams: streams}).(workapp.AuthLoginRequest)
		if resolved.Mode != workapp.AuthLoginEnvironmentPAT || resolved.OpenBrowser {
			t.Fatalf("non-interactive login = %#v", resolved)
		}
	}
}

func TestProviderLoginKeepsInteractiveModeWithTerminal(t *testing.T) {
	request := workapp.AuthLoginRequest{Provider: "azure-devops"}
	resolved := applyCLIDefaults("provider.auth.login", request, console.Policy{Streams: console.Streams{StdinTTY: true, StderrTTY: true}}).(workapp.AuthLoginRequest)
	if resolved.Mode != "" || !resolved.OpenBrowser {
		t.Fatalf("interactive login = %#v", resolved)
	}
}

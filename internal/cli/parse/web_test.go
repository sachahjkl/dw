package parse

import (
	"testing"

	"github.com/sachahjkl/dw/internal/cli/spec"
)

func TestWebPortAcceptsCompleteRange(t *testing.T) {
	for _, value := range []string{"0", "7331", "65535"} {
		result, err := Parse(spec.Root(nil), []string{"web", "start", "--port", value, "--no-open"})
		if err != nil {
			t.Fatalf("port %s: %v", value, err)
		}
		if result.Command.Key != "web.start" {
			t.Fatalf("port %s command = %s", value, result.Command.Key)
		}
	}
}

func TestWebPortRejectsOutOfRangeValues(t *testing.T) {
	for _, value := range []string{"-1", "65536", "not-a-port"} {
		if _, err := Parse(spec.Root(nil), []string{"web", "register", "--port", value}); err == nil {
			t.Fatalf("port %s was accepted", value)
		}
	}
}

func TestWebServeIsHidden(t *testing.T) {
	root := spec.Root(nil)
	web, ok := root.Child("web")
	if !ok {
		t.Fatal("web command is missing")
	}
	serve, ok := web.Child("serve")
	if !ok || !serve.Hidden {
		t.Fatalf("web serve = %#v", serve)
	}
}

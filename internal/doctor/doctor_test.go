package doctor

import (
	"context"
	"testing"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/contract"
)

type explicitRootConfig struct {
	defaultRoot string
	agentRoot   string
}

func (config *explicitRootConfig) ResolveRoot() string { return config.defaultRoot }
func (*explicitRootConfig) UserSettingsPath() string   { return "" }
func (config *explicitRootConfig) DefaultAgent(root string) contract.Agent {
	config.agentRoot = root
	return contract.AgentOpenCode
}
func (*explicitRootConfig) InitRoot(context.Context, InitRequest) error { return nil }

type successfulProcess struct{}

func (successfulProcess) Output(context.Context, string, ...string) (CommandOutput, error) {
	return CommandOutput{Stdout: []byte("1.0\n")}, nil
}

func TestDoctorHandlerHonorsExplicitRequestRoot(t *testing.T) {
	explicitRoot := t.TempDir()
	config := &explicitRootConfig{defaultRoot: "/configured/default"}
	handler := Handlers(New(config, successfulProcess{}))[0]

	result, err := handler.Execute(context.Background(), Request{Root: explicitRoot}, action.Runtime{})
	if err != nil {
		t.Fatalf("Doctor execution failed: %v", err)
	}
	report, ok := result.(Report)
	if !ok {
		t.Fatalf("Doctor result type = %T, want Report", result)
	}
	if report.Root != explicitRoot {
		t.Fatalf("Doctor report root = %q, want %q", report.Root, explicitRoot)
	}
	if report.Checks[0].Detail == nil || report.Checks[0].Detail.Path != explicitRoot {
		t.Fatalf("root check detail = %#v, want %q", report.Checks[0].Detail, explicitRoot)
	}
	if config.agentRoot != explicitRoot {
		t.Fatalf("default-agent check root = %q, want %q", config.agentRoot, explicitRoot)
	}
}

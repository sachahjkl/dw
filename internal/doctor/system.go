package doctor

import (
	"context"

	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/process"
)

type SystemConfig struct{}

func (SystemConfig) ResolveRoot() string      { return config.ResolveRoot("") }
func (SystemConfig) UserSettingsPath() string { return config.UserSettingsPath() }
func (SystemConfig) DefaultAgent(root string) contract.Agent {
	return contract.Agent(config.DefaultAgent(root))
}
func (SystemConfig) InitRoot(ctx context.Context, request InitRequest) error {
	if err := ctx.Err(); err != nil {
		return err
	}
	_, err := config.InitRoot(config.InitRequest{
		Root: request.Root, Profile: request.Profile, NoSave: request.NoSave, DryRun: request.DryRun,
	})
	return err
}

type SystemProcess struct{}

func (SystemProcess) Output(ctx context.Context, fileName string, arguments ...string) (CommandOutput, error) {
	result, err := process.Output(ctx, process.Command{FileName: fileName, Arguments: arguments})
	return CommandOutput{Stdout: result.Stdout, Stderr: result.Stderr, ExitCode: result.ExitCode}, err
}

func NewSystem() *Service { return New(SystemConfig{}, SystemProcess{}) }

var _ Config = SystemConfig{}
var _ Process = SystemProcess{}

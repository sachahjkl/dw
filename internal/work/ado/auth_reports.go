package ado

import "context"

type AuthLoginMode string

const (
	AuthLoginBrowser        AuthLoginMode = "Browser"
	AuthLoginDeviceCode     AuthLoginMode = "DeviceCode"
	AuthLoginEnvironmentPAT AuthLoginMode = "EnvironmentPat"
)

type AuthLoginReport struct {
	Mode               AuthLoginMode `json:"mode"`
	Source             *string       `json:"source,omitempty"`
	ExpiresOn          *string       `json:"expires_on,omitempty"`
	UsesEnvironmentPAT bool          `json:"uses_environment_pat"`
}

type AuthStatusReport struct {
	Connected bool    `json:"connected"`
	Source    *string `json:"source,omitempty"`
	ExpiresOn *string `json:"expires_on,omitempty"`
}

type AuthLogoutReport struct {
	RemovedLocalSession bool `json:"removed_local_session"`
}

func (a *Authenticator) LoginReport(ctx context.Context, mode AuthLoginMode, emit func(Event)) (AuthLoginReport, error) {
	if mode == AuthLoginEnvironmentPAT {
		return AuthLoginReport{Mode: mode, UsesEnvironmentPAT: true}, nil
	}
	var token Token
	var err error
	switch mode {
	case AuthLoginBrowser:
		token, err = a.LoginBrowser(ctx)
	case AuthLoginDeviceCode:
		token, err = a.LoginDeviceCode(ctx, func(instructions DeviceLoginInstructions) {
			if emit != nil {
				emit(Event{Kind: "device-login-required", VerificationURI: instructions.VerificationURI, UserCode: instructions.UserCode, ExpiresInSeconds: instructions.ExpiresInSeconds, PollIntervalSeconds: instructions.PollIntervalSeconds})
			}
		})
	default:
		return AuthLoginReport{}, &Error{Kind: ErrorInvalidInput, Detail: "Unsupported ADO authentication mode: " + string(mode)}
	}
	if err != nil {
		return AuthLoginReport{}, err
	}
	source := token.Source
	return AuthLoginReport{Mode: mode, Source: &source, ExpiresOn: token.ExpiresOn}, nil
}

func (a *Authenticator) StatusReport(ctx context.Context) (AuthStatusReport, error) {
	status, err := a.Status(ctx)
	if err != nil {
		return AuthStatusReport{}, err
	}
	return AuthStatusReport{Connected: status.Connected, Source: status.Source, ExpiresOn: status.ExpiresOn}, nil
}

func (a *Authenticator) LogoutReport(ctx context.Context) (AuthLogoutReport, error) {
	removed, err := a.Logout(ctx)
	return AuthLogoutReport{RemovedLocalSession: removed}, err
}

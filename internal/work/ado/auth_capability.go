package ado

import (
	"context"
	"github.com/sachahjkl/dw/internal/contract"

	"github.com/sachahjkl/dw/internal/work"
)

func (p *Provider) AuthStatus(ctx context.Context, project work.ProjectRef) (work.AuthStatus, error) {
	_, auth, err := p.resolve(ctx, project)
	if err != nil {
		return work.AuthStatus{}, err
	}
	if auth == nil {
		return work.AuthStatus{}, nil
	}
	status, err := auth.Status(ctx)
	if err != nil {
		return work.AuthStatus{}, err
	}
	result := work.AuthStatus{Authenticated: status.Connected}
	if status.Source != nil {
		result.Source = *status.Source
	}
	if status.ExpiresOn != nil {
		result.ExpiresOn = contract.Some(contract.Timestamp(*status.ExpiresOn))
	}
	return result, nil
}

func (p *Provider) Login(ctx context.Context, project work.ProjectRef, mode work.AuthMode, onDevice func(work.DeviceLogin) error) (work.AuthStatus, error) {
	_, auth, err := p.resolve(ctx, project)
	if err != nil {
		return work.AuthStatus{}, err
	}
	var token Token
	var callbackErr error
	switch mode {
	case work.AuthEnvironment:
		environmentToken := EnvironmentToken()
		if environmentToken == nil {
			return work.AuthStatus{}, &Error{Kind: ErrorMissingAuth}
		}
		return work.AuthStatus{Authenticated: true, Source: environmentToken.Source}, nil
	case work.AuthBrowser:
		if auth == nil {
			return work.AuthStatus{}, &Error{Kind: ErrorMissingAuth}
		}
		token, err = auth.LoginBrowser(ctx)
	case work.AuthDevice:
		if auth == nil {
			return work.AuthStatus{}, &Error{Kind: ErrorMissingAuth}
		}
		token, err = auth.LoginDeviceCode(ctx, func(instructions DeviceLoginInstructions) {
			if onDevice != nil && callbackErr == nil {
				callbackErr = onDevice(work.DeviceLogin{VerificationURI: instructions.VerificationURI, UserCode: instructions.UserCode, ExpiresInSeconds: instructions.ExpiresInSeconds, PollIntervalSeconds: instructions.PollIntervalSeconds})
			}
		})
	default:
		return work.AuthStatus{}, &Error{Kind: ErrorInvalidInput, Detail: "Unsupported ADO authentication mode: " + string(mode)}
	}
	if callbackErr != nil {
		return work.AuthStatus{}, callbackErr
	}
	if err != nil {
		return work.AuthStatus{}, err
	}
	result := work.AuthStatus{Authenticated: true, Source: token.Source}
	if token.ExpiresOn != nil {
		result.ExpiresOn = contract.Some(contract.Timestamp(*token.ExpiresOn))
	}
	return result, nil
}

func (p *Provider) Logout(ctx context.Context, project work.ProjectRef) (bool, error) {
	_, auth, err := p.resolve(ctx, project)
	if err != nil {
		return false, err
	}
	if auth == nil {
		return false, nil
	}
	return auth.Logout(ctx)
}

func EnvironmentPATLoginStatus() AuthStatus {
	return AuthStatus{Connected: true}
}

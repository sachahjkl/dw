package controller

import (
	"strings"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/cli/parse"
	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/workapp"
)

func providerAuthLoginRoute() Route {
	return Route{Key: "provider.auth.login", Build: buildProviderAuthLogin, Project: humanProject}
}

func buildProviderAuthLogin(invocation *parse.Result) (action.Request, error) {
	return workapp.AuthLoginRequest{Provider: strings.TrimSpace(invocation.Values.String("provider")), Root: resolvedRoot(invocation.Values)}, nil
}

func applyCLIDefaults(routeKey string, request action.Request, policy console.Policy) action.Request {
	if routeKey != "provider.auth.login" {
		return request
	}
	login, ok := request.(workapp.AuthLoginRequest)
	if !ok {
		return request
	}
	if policy.Interactive() {
		login.OpenBrowser = true
	} else {
		login.Mode = workapp.AuthLoginEnvironmentPAT
	}
	return login
}

package ado

import "encoding/json"

func (e Event) MarshalJSON() ([]byte, error) {
	switch e.Kind {
	case "authenticating":
		return json.Marshal(struct {
			Kind    string  `json:"kind"`
			Project *string `json:"project"`
		}{e.Kind, e.Project})
	case "device-login-required":
		return json.Marshal(struct {
			Kind                string `json:"kind"`
			VerificationURI     string `json:"verification_uri"`
			UserCode            string `json:"user_code"`
			ExpiresInSeconds    uint32 `json:"expires_in_seconds"`
			PollIntervalSeconds uint32 `json:"poll_interval_seconds"`
		}{e.Kind, e.VerificationURI, e.UserCode, e.ExpiresInSeconds, e.PollIntervalSeconds})
	case "loading-assigned-work-items":
		return json.Marshal(struct {
			Kind    string  `json:"kind"`
			Project *string `json:"project"`
			Top     int     `json:"top"`
		}{e.Kind, e.Project, e.Top})
	case "grouping-assigned-work-items", "loading-pull-requests":
		return json.Marshal(struct {
			Kind    string  `json:"kind"`
			Project *string `json:"project"`
		}{e.Kind, e.Project})
	case "resolving-pull-request-work-items":
		return json.Marshal(struct {
			Kind         string   `json:"kind"`
			Repositories []string `json:"repositories"`
		}{e.Kind, nonNilStrings(e.Repositories)})
	case "extracting-git-work-items":
		return json.Marshal(struct {
			Kind  string `json:"kind"`
			GitTo string `json:"git_to"`
		}{e.Kind, e.GitTo})
	case "loading-work-item", "loading-work-item-context":
		return json.Marshal(struct {
			Kind string `json:"kind"`
			ID   string `json:"id"`
		}{e.Kind, e.ID})
	case "loading-work-items", "loading-changelog", "loading-changelog-items":
		return json.Marshal(struct {
			Kind string   `json:"kind"`
			IDs  []string `json:"ids"`
		}{e.Kind, nonNilStrings(e.IDs)})
	case "updating-work-item-state":
		return json.Marshal(struct {
			Kind  string   `json:"kind"`
			IDs   []string `json:"ids"`
			State string   `json:"state"`
		}{e.Kind, nonNilStrings(e.IDs), e.State})
	case "updated-work-item-state":
		return json.Marshal(struct {
			Kind  string `json:"kind"`
			ID    string `json:"id"`
			State string `json:"state"`
		}{e.Kind, e.ID, e.State})
	default:
		return json.Marshal(struct {
			Kind string `json:"kind"`
		}{e.Kind})
	}
}

func nonNilStrings(values []string) []string {
	if values == nil {
		return []string{}
	}
	return values
}

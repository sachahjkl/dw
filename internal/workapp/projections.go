package workapp

const (
	RichContextSchemaVersion  = "dw.work.rich-context.v1"
	AttachmentDirectoryPrefix = "attachments/work/"
)

type Event struct {
	Kind                string   `json:"kind"`
	Project             *string  `json:"project,omitempty"`
	VerificationURI     string   `json:"verification_uri,omitempty"`
	UserCode            string   `json:"user_code,omitempty"`
	ExpiresInSeconds    uint32   `json:"expires_in_seconds,omitempty"`
	PollIntervalSeconds uint32   `json:"poll_interval_seconds,omitempty"`
	Top                 int      `json:"top,omitempty"`
	Repositories        []string `json:"repositories,omitempty"`
	GitTo               string   `json:"git_to,omitempty"`
	ID                  string   `json:"id,omitempty"`
	IDs                 []string `json:"ids,omitempty"`
	State               string   `json:"state,omitempty"`
}

func (e Event) ActionID() string {
	switch e.Kind {
	case "authenticating":
		return "work.auth"
	case "device-login-required":
		return "work.auth.device.login"
	case "loading-assigned-work-items":
		return "work.assigned.load"
	case "grouping-assigned-work-items":
		return "work.assigned.group"
	case "loading-pull-requests":
		return "work.pr.load"
	case "resolving-pull-request-work-items":
		return "work.pr.resolve.workitems"
	case "extracting-git-work-items":
		return "work.git.extract.workitems"
	case "loading-work-item":
		return "work.item.load"
	case "loading-work-items":
		return "work.items.load"
	case "loading-work-item-context":
		return "work.item.context.load"
	case "loading-changelog":
		return "work.changelog.load"
	case "loading-changelog-items":
		return "work.changelog.items.load"
	case "updating-work-item-state":
		return "work.item.state.update"
	case "updated-work-item-state":
		return "work.item.state.updated"
	default:
		return ""
	}
}

type ItemSnapshot struct {
	ID    string  `json:"id"`
	Type  *string `json:"type"`
	State *string `json:"state"`
	Title *string `json:"title"`
	URL   *string `json:"url"`
}
type ItemGroup struct {
	Parent ItemSnapshot   `json:"parent"`
	Items  []ItemSnapshot `json:"items"`
}
type ChildCreateResult struct {
	Repository string `json:"repository"`
	ID         string `json:"id"`
	Title      string `json:"title"`
}
type PullRequestItem struct {
	Repository    string   `json:"repository"`
	PullRequestID int64    `json:"pullRequestId"`
	Title         *string  `json:"title"`
	Status        *string  `json:"status"`
	SourceRefName *string  `json:"sourceRefName"`
	TargetRefName *string  `json:"targetRefName"`
	IsDraft       bool     `json:"isDraft"`
	CreatedBy     *string  `json:"createdBy"`
	URL           *string  `json:"url"`
	WebURL        *string  `json:"webUrl"`
	WorkItemIDs   []string `json:"workItemIds"`
}

type RichContextItem struct {
	SchemaVersion string                 `json:"schemaVersion"`
	WorkItem      RichContextWorkItem    `json:"workItem"`
	Core          RichContextCore        `json:"core"`
	Content       RichContextContent     `json:"content"`
	Links         RichContextLinks       `json:"links"`
	Attachments   RichContextAttachments `json:"attachments"`
	Relations     []RichContextRelation  `json:"relations"`
	Comments      []RichContextComment   `json:"comments"`
}
type RichContextWorkItem struct {
	ID            string   `json:"id"`
	URL           *string  `json:"url"`
	Title         *string  `json:"title"`
	Type          *string  `json:"type"`
	State         *string  `json:"state"`
	AssignedTo    *string  `json:"assignedTo"`
	AreaPath      *string  `json:"areaPath"`
	IterationPath *string  `json:"iterationPath"`
	Tags          []string `json:"tags"`
}
type RichContextCore struct {
	CreatedBy   *string `json:"createdBy"`
	CreatedDate *string `json:"createdDate"`
	ChangedBy   *string `json:"changedBy"`
	ChangedDate *string `json:"changedDate"`
	Priority    *string `json:"priority"`
	ValueArea   *string `json:"valueArea"`
}
type RichContextContent struct {
	Description        *string           `json:"description"`
	AcceptanceCriteria *string           `json:"acceptanceCriteria"`
	ProductContext     map[string]string `json:"productContext"`
}
type RichContextLinks struct {
	ParentIDs      []string `json:"parentIds"`
	ChildIDs       []string `json:"childIds"`
	PredecessorIDs []string `json:"predecessorIds"`
	SuccessorIDs   []string `json:"successorIds"`
}
type RichContextAttachments struct {
	DirectoryHint string                  `json:"directoryHint"`
	Items         []RichContextAttachment `json:"items"`
}
type RichContextAttachment struct {
	Name          *string `json:"name"`
	URL           *string `json:"url"`
	Comment       *string `json:"comment"`
	DirectoryHint string  `json:"directoryHint"`
}
type RichContextRelation struct {
	Kind       string  `json:"kind"`
	Rel        *string `json:"rel"`
	WorkItemID *string `json:"workItemId"`
	Name       *string `json:"name"`
	URL        *string `json:"url"`
	Comment    *string `json:"comment"`
	Artifact   *string `json:"artifact"`
}
type RichContextComment struct {
	Author      *string `json:"author"`
	CreatedDate *string `json:"createdDate"`
	Text        *string `json:"text"`
}

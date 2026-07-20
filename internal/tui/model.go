package tui

import (
	"context"
	"strings"
	"time"

	"charm.land/bubbles/v2/spinner"
	"charm.land/bubbles/v2/viewport"
	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/l10n"
)

// Stable action slot IDs let presentation projectors attach concrete requests
// without teaching the TUI domain request types.
const (
	WorkspaceOpenSlot       action.ID = "tui.workspace.open"
	WorkspacePreflightSlot  action.ID = "tui.workspace.preflight"
	WorkspaceSyncSlot       action.ID = "tui.workspace.sync"
	WorkspaceLatestSlot     action.ID = "tui.workspace.latest"
	WorkspaceHandoffSlot    action.ID = "tui.workspace.handoff"
	WorkspaceCommitSlot     action.ID = "tui.workspace.commit-preview"
	WorkspaceFinishPlanSlot action.ID = "tui.workspace.finish-preview"
	WorkspaceFinishSlot     action.ID = "tui.workspace.finish-execute"
	WorkspaceRemovePlanSlot action.ID = "tui.workspace.remove-preview"
	WorkspaceRemoveSlot     action.ID = "tui.workspace.remove-execute"
	ADOStartPlanSlot        action.ID = "tui.ado.start-preview"
	ADOStartSlot            action.ID = "tui.ado.start-execute"
	ADOOpenAgentSlot        action.ID = "tui.ado.open-agent"
	ADOContextSlot          action.ID = "tui.ado.context"
	ADOWorkItemSlot         action.ID = "tui.ado.work-item"
	ADOSetStateSlot         action.ID = "tui.ado.set-state"
	ADOOpenURLSlot          action.ID = "tui.ado.open-url"
	PRStartPlanSlot         action.ID = "tui.pr.start-preview"
	PRStartSlot             action.ID = "tui.pr.start-execute"
	PROpenAgentSlot         action.ID = "tui.pr.open-agent"
	PRFinishPlanSlot        action.ID = "tui.pr.finish-preview"
	PRFinishSlot            action.ID = "tui.pr.finish-execute"
	PRChangelogSlot         action.ID = "tui.pr.changelog"
	PRDiffSlot              action.ID = "tui.pr.diff-preview"
	PROpenURLSlot           action.ID = "tui.pr.open-url"
	DBSchemaSlot            action.ID = "tui.db.schema"
)

type KeyKind uint8

const (
	KeyPress KeyKind = iota
	KeyRepeat
	KeyRelease
)

// Key is the public, terminal-independent parity surface.
type Key struct {
	Code  string
	Text  string
	Kind  KeyKind
	Ctrl  bool
	Alt   bool
	Shift bool
}

type EffectKind uint8

const (
	NoEffect EffectKind = iota
	QuitEffect
	ReloadEffect
	StartActionEffect
	AnswerInputEffect
)

// Effect describes side effects produced by a pure HandleKey transition.
type Effect struct {
	Kind     EffectKind
	Action   Action
	Response action.Response
	input    chan action.Response
}

type modalKind uint8

const (
	menuModal modalKind = iota
	menuSectionModal
	helpModal
	stateModal
	journalModal
	detailModal
	progressModal
)

type loaderState struct {
	generation uint64
	running    bool
	started    time.Time
	errorText  string
}

type queuedAction struct {
	action Action
}

type activeAction struct {
	id         uint64
	action     Action
	generation uint64
	started    time.Time
}

type inputPrompt struct {
	runID        uint64
	prompt       action.Prompt
	label        string
	help         string
	choices      []string
	value        string
	selected     int
	selectedMany []bool
	response     chan action.Response
}

type detailState struct {
	title  string
	lines  []string
	scroll int
}

// Model contains all pure navigation, modal, form, queue, and history state.
type Model struct {
	deps Dependencies
	ctx  context.Context
	l10n l10n.Localizer

	snapshot Snapshot
	view     View
	width    int
	height   int

	selectedAction      int
	selectedCockpit     int
	selectedWorkspace   int
	selectedADOProject  int
	selectedADOItem     int
	selectedPR          int
	selectedDB          int
	selectedMenuSection int
	selectedMenuItem    int

	filter       string
	filterActive bool
	confirmation *Action
	prompt       *inputPrompt
	form         *FormState
	composer     FormState
	modalStack   []modalKind
	detail       *detailState
	progressRun  uint64
	stateScroll  int

	messages         []string
	history          History
	queue            []queuedAction
	active           *activeAction
	nextRunID        uint64
	actionGeneration uint64
	reloadAfterQueue bool
	actionUpdates    <-chan actionUpdate
	pendingExternal  *externalRun

	snapshotLoad loaderState
	assignedLoad loaderState
	prLoad       loaderState

	quit     bool
	spinner  spinner.Model
	viewport viewport.Model
}

// NewModel constructs the pure model. Init starts available loaders.
func NewModel(deps Dependencies) *Model {
	localizer := newTUILocalizer(deps.Localizer)
	root := deps.Root
	model := &Model{
		deps:     deps,
		ctx:      context.Background(),
		l10n:     localizer,
		snapshot: Snapshot{Root: root},
		view:     Dashboard,
		messages: []string{localizer.Text(msgReady)},
		history:  newHistory(),
		spinner:  spinner.New(spinner.WithSpinner(spinner.MiniDot)),
		viewport: viewport.New(viewport.WithWidth(80), viewport.WithHeight(20)),
	}
	return model
}

// NewModelWithSnapshot constructs a loader-free pure model for parity and
// embedding while preserving the same key transition implementation.
func NewModelWithSnapshot(deps Dependencies, snapshot Snapshot) *Model {
	model := NewModel(deps)
	model.snapshot = snapshot
	activateRequestlessParityActions(&model.snapshot)
	if model.snapshot.Root == "" {
		model.snapshot.Root = deps.Root
	}
	model.clampSelections()
	return model
}

func activateRequestlessParityActions(snapshot *Snapshot) {
	activate := func(actions []Action) {
		for index := range actions {
			if actions[index].Request == nil {
				actions[index].Active = true
			}
		}
	}
	activate(snapshot.Actions)
	for index := range snapshot.Workspaces {
		activate(snapshot.Workspaces[index].Actions)
	}
	for project := range snapshot.ADOProjects {
		for item := range snapshot.ADOProjects[project].Items {
			activate(snapshot.ADOProjects[project].Items[item].Actions)
		}
	}
	for index := range snapshot.PullRequests {
		activate(snapshot.PullRequests[index].Actions)
	}
	for index := range snapshot.Databases {
		activate(snapshot.Databases[index].Actions)
	}
}

type SelectionState struct {
	Action, Cockpit, Workspace, ADOProject, ADOItem, PullRequest, Database int
}

func (m *Model) Selection() SelectionState {
	return SelectionState{m.selectedAction, m.selectedCockpit, m.selectedWorkspace, m.selectedADOProject, m.selectedADOItem, m.selectedPR, m.selectedDB}
}

func (m *Model) ConfirmationOpen() bool { return m.confirmation != nil }
func (m *Model) FormOpen() bool         { return m.form != nil }
func (m *Model) Filter() (string, bool) { return m.filter, m.filterActive }
func (m *Model) QueueLength() int       { return len(m.queue) }
func (m *Model) ActiveAction() (Action, bool) {
	if m.active == nil {
		return Action{}, false
	}
	return m.active.action, true
}

func (m *Model) CurrentView() View  { return m.view }
func (m *Model) Snapshot() Snapshot { return m.snapshot }
func (m *Model) ShouldQuit() bool   { return m.quit }
func (m *Model) History() History   { return m.history }

func (m *Model) ModalStack() []string {
	result := make([]string, 0, len(m.modalStack))
	for _, modal := range m.modalStack {
		switch modal {
		case menuModal:
			result = append(result, "menu")
		case menuSectionModal:
			result = append(result, "menu-section")
		case helpModal:
			result = append(result, "help")
		case stateModal:
			result = append(result, "state")
		case journalModal:
			result = append(result, "journal")
		case detailModal:
			result = append(result, "detail")
		case progressModal:
			result = append(result, "progress")
		}
	}
	return result
}

func (m *Model) message(id l10n.ID, args ...l10n.Arg) string {
	return m.l10n.Render(l10n.M(id, args...))
}

func (m *Model) addMessage(message string) {
	m.messages = append(m.messages, message)
	if len(m.messages) > 80 {
		m.messages = append([]string(nil), m.messages[len(m.messages)-80:]...)
	}
}

func (m *Model) pushModal(kind modalKind) {
	kept := m.modalStack[:0]
	for _, current := range m.modalStack {
		if current != kind {
			kept = append(kept, current)
		}
	}
	m.modalStack = append(kept, kind)
}

func (m *Model) closeTopModal() {
	if len(m.modalStack) == 0 {
		m.detail = nil
		return
	}
	closed := m.modalStack[len(m.modalStack)-1]
	m.modalStack = m.modalStack[:len(m.modalStack)-1]
	switch closed {
	case detailModal:
		m.detail = nil
	case progressModal:
		m.progressRun = 0
	}
}

func (m *Model) removeModal(kind modalKind) {
	for i := len(m.modalStack) - 1; i >= 0; i-- {
		if m.modalStack[i] == kind {
			m.modalStack = append(m.modalStack[:i], m.modalStack[i+1:]...)
		}
	}
}

func (m *Model) topModal() (modalKind, bool) {
	if len(m.modalStack) == 0 {
		return 0, false
	}
	return m.modalStack[len(m.modalStack)-1], true
}

func (m *Model) setView(view View) {
	m.view = view
	m.selectedAction = 0
	m.confirmation = nil
	m.form = nil
	m.filterActive = false
	m.removeModal(menuModal)
	m.removeModal(menuSectionModal)
	m.removeModal(helpModal)
	m.clampSelections()
}

func (m *Model) cycleView(delta int) {
	index := 0
	for i, view := range allViews {
		if view == m.view {
			index = i
			break
		}
	}
	index = (index + delta + len(allViews)) % len(allViews)
	m.setView(allViews[index])
}

func (m *Model) HandleKey(key Key) []Effect {
	if key.Kind == KeyRelease {
		return nil
	}
	if key.Ctrl && strings.EqualFold(key.Code, "c") || key.Code == "ctrl+c" {
		m.quit = true
		return []Effect{{Kind: QuitEffect}}
	}
	if m.snapshot.NeedsInit {
		return m.handleInitKey(key)
	}
	if m.prompt != nil {
		return m.handleInputKey(key)
	}
	if m.form != nil {
		return m.handleFormKey(key)
	}
	if modal, ok := m.topModal(); ok {
		switch modal {
		case menuModal:
			return m.handleMenuKey(key)
		case menuSectionModal:
			return m.handleMenuSectionKey(key)
		case helpModal:
			return m.handleHelpKey(key)
		case stateModal:
			return m.handleStateKey(key)
		case journalModal:
			return m.handleJournalKey(key)
		case detailModal:
			return m.handleDetailKey(key)
		case progressModal:
			return nil
		}
	}
	if m.detail != nil {
		return m.handleDetailKey(key)
	}
	if m.filterActive {
		return m.handleFilterKey(key)
	}
	if m.confirmation != nil {
		return m.handleConfirmationKey(key)
	}
	if m.view == Composer {
		if effects, handled := m.handleComposerKey(key); handled {
			return effects
		}
	}
	if m.handleNavigationKey(key) {
		return nil
	}

	switch key.Code {
	case "q", "esc":
		m.quit = true
		return []Effect{{Kind: QuitEffect}}
	case "tab", "right":
		m.cycleView(1)
		return nil
	case "shift+tab", "backtab", "left":
		m.cycleView(-1)
		return nil
	case "/":
		m.filterActive = true
		m.confirmation = nil
		return nil
	case "n":
		if m.view != ADO && m.view != PullRequests {
			m.openForm("")
			return nil
		}
	case "m":
		m.openMenu()
		return nil
	case "r":
		return []Effect{{Kind: ReloadEffect}}
	case "1", "2", "3", "4", "5", "6":
		m.setView(allViews[int(key.Code[0]-'1')])
		return nil
	case "?":
		m.pushModal(helpModal)
		return nil
	}
	if effects, handled := m.handleViewActionKey(key); handled {
		return effects
	}
	if key.Code == "enter" {
		switch m.view {
		case Dashboard:
			if len(m.snapshot.Cockpit) != 0 {
				return m.requestAction(m.snapshot.Cockpit[m.selectedCockpit].Primary)
			}
		case Workspaces:
			return m.workspaceAction(WorkspaceOpenSlot)
		default:
			if action, ok := m.selectedVisibleAction(); ok {
				return m.requestAction(action)
			}
		}
		m.addMessage(m.l10n.Text("tui.message.no-selection"))
	}
	return nil
}

func (m *Model) handleInitKey(key Key) []Effect {
	switch key.Code {
	case "q":
		m.quit = true
		return []Effect{{Kind: QuitEffect}}
	case "enter", "i":
		if m.snapshot.InitAction != nil {
			return m.requestAction(*m.snapshot.InitAction)
		}
		m.addMessage(m.l10n.Text("tui.message.unavailable"))
	}
	return nil
}

func (m *Model) handleInputKey(key Key) []Effect {
	prompt := m.prompt
	switch key.Code {
	case "esc":
		close(prompt.response)
		m.prompt = nil
		m.addMessage(m.l10n.Text("tui.message.input-canceled"))
		return nil
	case "up", "k":
		if prompt.selected > 0 {
			prompt.selected--
		}
	case "down", "j":
		if prompt.selected+1 < len(prompt.choices) {
			prompt.selected++
		}
	case "space":
		if prompt.prompt.Kind == action.PromptSelectMany && prompt.selected < len(prompt.selectedMany) {
			prompt.selectedMany[prompt.selected] = !prompt.selectedMany[prompt.selected]
		} else if prompt.prompt.Kind == action.PromptText || prompt.prompt.Kind == action.PromptSecret {
			prompt.value += " "
		}
	case "backspace":
		value := []rune(prompt.value)
		if len(value) != 0 {
			prompt.value = string(value[:len(value)-1])
		}
	case "y", "Y":
		if prompt.prompt.Kind == action.PromptConfirm {
			return m.answerInput(true)
		}
	case "n", "N":
		if prompt.prompt.Kind == action.PromptConfirm {
			return m.answerInput(false)
		}
	case "enter":
		return m.answerInput(true)
	default:
		if (prompt.prompt.Kind == action.PromptText || prompt.prompt.Kind == action.PromptSecret) && key.Text != "" {
			prompt.value += key.Text
		}
	}
	return nil
}

func (m *Model) answerInput(accepted bool) []Effect {
	prompt := m.prompt
	response := action.Response{Kind: prompt.prompt.Kind, Accepted: accepted}
	switch prompt.prompt.Kind {
	case action.PromptSelectOne:
		if prompt.selected < len(prompt.prompt.Choices) {
			response.Value = prompt.prompt.Choices[prompt.selected].Value
		}
	case action.PromptSelectMany:
		for i, selected := range prompt.selectedMany {
			if selected {
				response.Values = append(response.Values, prompt.prompt.Choices[i].Value)
			}
		}
	case action.PromptText:
		response.Text = prompt.value
	case action.PromptSecret:
		response.Secret = contract.NewSecretValue(prompt.value)
	}
	m.prompt = nil
	m.addMessage(m.l10n.Text("tui.message.input-sent"))
	return []Effect{{Kind: AnswerInputEffect, Response: response, input: prompt.response}}
}

func (m *Model) handleFormKey(key Key) []Effect {
	form := m.form
	if form.Mode == ChooseTemplate {
		switch key.Code {
		case "q":
			m.quit = true
			return []Effect{{Kind: QuitEffect}}
		case "esc":
			m.form = nil
		case "up", "k":
			form.move(-1)
		case "down", "j":
			form.move(1)
		case "enter":
			form.begin(m.snapshot)
		}
		return nil
	}
	switch key.Code {
	case "esc":
		m.form = nil
	case "up", "shift+tab", "backtab":
		form.move(-1)
	case "down", "tab":
		form.move(1)
	case "backspace":
		form.backspace()
	case "ctrl+space":
		m.applySuggestion(form)
	case "space":
		form.toggle()
	case "enter":
		generated, ok := form.action(m.l10n)
		if !ok {
			m.addMessage(m.l10n.Text("tui.message.form-invalid"))
			return nil
		}
		m.form = nil
		return m.requestAction(generated)
	default:
		if key.Text != "" {
			form.input(key.Text)
		}
	}
	return nil
}

func (m *Model) handleFilterKey(key Key) []Effect {
	switch key.Code {
	case "esc":
		m.filterActive = false
		m.filter = ""
		m.selectedAction = 0
	case "enter":
		m.filterActive = false
		m.clampSelections()
	case "backspace":
		value := []rune(m.filter)
		if len(value) != 0 {
			m.filter = string(value[:len(value)-1])
		}
		m.selectedAction = 0
	default:
		if key.Text != "" {
			m.filter += key.Text
			m.selectedAction = 0
		}
	}
	return nil
}

func (m *Model) handleConfirmationKey(key Key) []Effect {
	switch key.Code {
	case "enter", "y", "Y":
		action := *m.confirmation
		m.confirmation = nil
		return m.startOrQueue(action)
	case "esc", "n", "N":
		m.confirmation = nil
		m.addMessage(m.l10n.Text("tui.message.canceled"))
	}
	return nil
}

func (m *Model) handleHelpKey(key Key) []Effect {
	if key.Code == "esc" || key.Code == "?" || key.Code == "q" || key.Code == "enter" {
		m.closeTopModal()
	}
	return nil
}

func (m *Model) handleStateKey(key Key) []Effect {
	switch key.Code {
	case "esc", "i", "q":
		m.closeTopModal()
	case "up", "k":
		if m.stateScroll > 0 {
			m.stateScroll--
		}
	case "down", "j":
		m.stateScroll++
	case "home":
		m.stateScroll = 0
	case "end":
		m.stateScroll = int(^uint(0) >> 1)
	}
	return nil
}

func (m *Model) handleDetailKey(key Key) []Effect {
	if m.detail == nil {
		return nil
	}
	switch key.Code {
	case "esc", "q", "enter":
		m.closeTopModal()
	case "up", "k":
		if m.detail.scroll > 0 {
			m.detail.scroll--
		}
	case "down", "j":
		m.detail.scroll++
	case "home":
		m.detail.scroll = 0
	case "end":
		m.detail.scroll = len(m.detail.lines)
	}
	return nil
}

func (m *Model) handleJournalKey(key Key) []Effect {
	switch key.Code {
	case "esc", "h":
		m.closeTopModal()
	case "q":
		m.quit = true
		return []Effect{{Kind: QuitEffect}}
	case "up", "k":
		if m.history.Scroll > 0 {
			m.history.Scroll--
		}
	case "down", "j":
		m.history.Scroll++
	case "left", "[":
		m.history.selectRun(-1)
	case "right", "]":
		m.history.selectRun(1)
	case "home":
		m.history.Scroll = 0
	case "end":
		m.history.Scroll = int(^uint(0) >> 1)
	case "f":
		m.history.Fullscreen = !m.history.Fullscreen
		m.history.Scroll = 0
	case "a":
		m.history.enableAll()
	case "e":
		m.history.toggleLevel(ErrorLevel)
	case "w":
		m.history.toggleLevel(WarningLevel)
	case "i":
		m.history.toggleLevel(InfoLevel)
	case "d":
		m.history.toggleLevel(DebugLevel)
	case "o":
		m.history.toggleLevel(OtherLevel)
	}
	return nil
}

func (m *Model) openMenu() {
	m.selectedMenuSection = min(m.selectedMenuSection, 3)
	m.selectedMenuItem = 0
	m.filterActive = false
	m.confirmation = nil
	m.form = nil
	m.pushModal(menuModal)
}

func (m *Model) handleMenuKey(key Key) []Effect {
	switch key.Code {
	case "q":
		m.quit = true
		return []Effect{{Kind: QuitEffect}}
	case "esc", "m":
		m.closeTopModal()
	case "up", "k":
		if m.selectedMenuSection > 0 {
			m.selectedMenuSection--
		}
		m.selectedMenuItem = 0
	case "down", "j":
		if m.selectedMenuSection < 3 {
			m.selectedMenuSection++
		}
		m.selectedMenuItem = 0
	case "enter":
		m.pushModal(menuSectionModal)
	case "?":
		m.pushModal(helpModal)
	}
	return nil
}

func (m *Model) handleMenuSectionKey(key Key) []Effect {
	items := m.menuItems()
	switch key.Code {
	case "q":
		m.quit = true
		return []Effect{{Kind: QuitEffect}}
	case "esc":
		m.closeTopModal()
	case "m":
		m.removeModal(menuSectionModal)
		m.removeModal(menuModal)
	case "up", "k":
		if m.selectedMenuItem > 0 {
			m.selectedMenuItem--
		}
	case "down", "j":
		if m.selectedMenuItem+1 < len(items) {
			m.selectedMenuItem++
		}
	case "?":
		m.pushModal(helpModal)
	case "enter":
		return m.runSelectedMenuItem(items)
	default:
		for _, item := range items {
			if item.action != nil && item.action.Hotkey == key.Code {
				return m.requestAction(*item.action)
			}
		}
	}
	return nil
}

type menuItem struct {
	label, description, key string
	action                  *Action
	open                    modalKind
}

func (m *Model) menuItems() []menuItem {
	if m.selectedMenuSection == 0 {
		return []menuItem{
			{label: m.l10n.Text(msgJournal), description: m.l10n.Text("tui.panel.operations"), key: "h", open: journalModal},
			{label: m.l10n.Text(msgState), description: m.l10n.Text("tui.panel.loads"), key: "i", open: stateModal},
			{label: m.l10n.Text(msgHelp), description: m.l10n.Text("tui.help.accessibility"), key: "?", open: helpModal},
		}
	}
	section := [...]string{"", "configuration", "default-agent", "terminal-color"}[m.selectedMenuSection]
	var result []menuItem
	for i := range m.snapshot.Actions {
		item := &m.snapshot.Actions[i]
		if item.MenuSection == section {
			result = append(result, menuItem{label: item.Label, description: item.Description, key: item.Hotkey, action: item})
		}
	}
	return result
}

func (m *Model) runSelectedMenuItem(items []menuItem) []Effect {
	if m.selectedMenuItem >= len(items) {
		m.addMessage(m.l10n.Text("tui.message.no-selection"))
		return nil
	}
	item := items[m.selectedMenuItem]
	if item.action != nil {
		return m.requestAction(*item.action)
	}
	m.pushModal(item.open)
	return nil
}

func (m *Model) handleComposerKey(key Key) ([]Effect, bool) {
	if m.composer.Mode == ChooseTemplate {
		if key.Code == "enter" {
			m.composer.begin(m.snapshot)
			return nil, true
		}
		return nil, false
	}
	switch key.Code {
	case "esc":
		m.composer = FormState{}
		return nil, true
	case "backspace":
		m.composer.backspace()
		return nil, true
	case "ctrl+space":
		m.applySuggestion(&m.composer)
		return nil, true
	case "space":
		m.composer.toggle()
		return nil, true
	case "enter":
		generated, ok := m.composer.action(m.l10n)
		if !ok {
			m.addMessage(m.l10n.Text("tui.message.form-invalid"))
			return nil, true
		}
		return m.requestAction(generated), true
	default:
		if key.Text != "" {
			m.composer.input(key.Text)
			return nil, true
		}
	}
	return nil, false
}

func (m *Model) applySuggestion(form *FormState) {
	if value, ok := form.suggest(); ok {
		m.addMessage(m.message("tui.message.suggestion", l10n.A("value", value)))
	} else {
		m.addMessage(m.l10n.Text("tui.message.no-suggestion"))
	}
}

func (m *Model) handleNavigationKey(key Key) bool {
	delta := 0
	switch key.Code {
	case "up", "k":
		delta = -1
	case "down", "j":
		delta = 1
	}
	if delta != 0 {
		switch m.view {
		case Dashboard:
			m.selectedCockpit = clampIndex(m.selectedCockpit+delta, len(m.snapshot.Cockpit))
		case Workspaces:
			m.selectedWorkspace = clampIndex(m.selectedWorkspace+delta, len(m.snapshot.Workspaces))
		case ADO:
			m.selectedADOItem = clampIndex(m.selectedADOItem+delta, m.adoItemCount())
		case PullRequests:
			m.selectedPR = clampIndex(m.selectedPR+delta, len(m.snapshot.PullRequests))
		case Databases:
			m.selectedDB = clampIndex(m.selectedDB+delta, len(m.snapshot.Databases))
		case Composer:
			m.composer.move(delta)
		}
		return true
	}
	if key.Code == "K" || (key.Code == "[" && m.view == ADO) {
		if m.view == ADO {
			m.cycleADOProject(-1)
		} else {
			m.selectedWorkspace = clampIndex(m.selectedWorkspace-1, len(m.snapshot.Workspaces))
		}
		return true
	}
	if key.Code == "J" || (key.Code == "]" && m.view == ADO) {
		if m.view == ADO {
			m.cycleADOProject(1)
		} else {
			m.selectedWorkspace = clampIndex(m.selectedWorkspace+1, len(m.snapshot.Workspaces))
		}
		return true
	}
	return false
}

func (m *Model) handleViewActionKey(key Key) ([]Effect, bool) {
	switch m.view {
	case ADO:
		switch key.Code {
		case "enter", "n", "s":
			return m.adoAction(ADOStartPlanSlot), true
		case "x":
			return m.adoAction(ADOStartSlot), true
		case "c":
			return m.adoAction(ADOContextSlot), true
		case "w":
			return m.adoAction(ADOWorkItemSlot), true
		case "e":
			return m.adoAction(ADOSetStateSlot), true
		case "E":
			m.openADOStateForm()
			return nil, true
		case "o":
			return m.adoAction(ADOOpenAgentSlot), true
		case "u":
			return m.adoAction(ADOOpenURLSlot), true
		}
	case Workspaces:
		switch key.Code {
		case "enter", "o":
			return m.workspaceAction(WorkspaceOpenSlot), true
		case "p":
			return m.workspaceAction(WorkspacePreflightSlot), true
		case "s":
			return m.workspaceAction(WorkspaceSyncSlot), true
		case "l":
			return m.workspaceAction(WorkspaceLatestSlot), true
		case "v":
			return m.workspaceAction(WorkspaceHandoffSlot), true
		case "c":
			return m.workspaceAction(WorkspaceCommitSlot), true
		case "f":
			return m.workspaceAction(WorkspaceFinishPlanSlot), true
		case "F":
			return m.workspaceAction(WorkspaceFinishSlot), true
		case "t":
			return m.workspaceAction(WorkspaceRemovePlanSlot), true
		case "x":
			return m.workspaceAction(WorkspaceRemoveSlot), true
		}
	case PullRequests:
		switch key.Code {
		case "enter", "n", "s":
			return m.prAction(PRStartPlanSlot), true
		case "x":
			return m.prAction(PRStartSlot), true
		case "f":
			return m.prAction(PRFinishPlanSlot), true
		case "F":
			return m.prAction(PRFinishSlot), true
		case "c":
			return m.prAction(PRChangelogSlot), true
		case "d":
			return m.prAction(PRDiffSlot), true
		case "o":
			return m.prAction(PROpenAgentSlot), true
		case "N":
			m.openPRForm()
			return nil, true
		case "u":
			return m.prAction(PROpenURLSlot), true
		}
	case Databases:
		switch key.Code {
		case "enter", "s":
			return m.dbAction(DBSchemaSlot), true
		case "d":
			m.openDBForm("db-describe")
			return nil, true
		case "e":
			m.openDBForm("db-query")
			return nil, true
		}
	}
	return nil, false
}

func (m *Model) workspaceAction(id action.ID) []Effect {
	if m.selectedWorkspace < len(m.snapshot.Workspaces) {
		if item, ok := findAction(m.snapshot.Workspaces[m.selectedWorkspace].Actions, id); ok {
			return m.requestAction(item)
		}
	}
	m.addMessage(m.l10n.Text("tui.message.unavailable"))
	return nil
}
func (m *Model) adoAction(id action.ID) []Effect {
	if p := m.selectedADOProject; p < len(m.snapshot.ADOProjects) && m.selectedADOItem < len(m.snapshot.ADOProjects[p].Items) {
		if item, ok := findAction(m.snapshot.ADOProjects[p].Items[m.selectedADOItem].Actions, id); ok {
			return m.requestAction(item)
		}
	}
	m.addMessage(m.l10n.Text("tui.message.unavailable"))
	return nil
}
func (m *Model) prAction(id action.ID) []Effect {
	if m.selectedPR < len(m.snapshot.PullRequests) {
		if item, ok := findAction(m.snapshot.PullRequests[m.selectedPR].Actions, id); ok {
			return m.requestAction(item)
		}
	}
	m.addMessage(m.l10n.Text("tui.message.unavailable"))
	return nil
}
func (m *Model) dbAction(id action.ID) []Effect {
	if m.selectedDB < len(m.snapshot.Databases) {
		if item, ok := findAction(m.snapshot.Databases[m.selectedDB].Actions, id); ok {
			return m.requestAction(item)
		}
	}
	m.addMessage(m.l10n.Text("tui.message.unavailable"))
	return nil
}

func (m *Model) requestAction(item Action) []Effect {
	if !item.Active {
		m.addMessage(m.l10n.Text("tui.message.unavailable"))
		return nil
	}
	if item.Risk == Destructive || item.Risk == External {
		copy := item
		m.confirmation = &copy
		m.addMessage(m.message("tui.message.confirmation", l10n.A("label", item.Label)))
		return nil
	}
	return m.startOrQueue(item)
}

func (m *Model) startOrQueue(item Action) []Effect {
	if m.active != nil {
		m.queue = append(m.queue, queuedAction{action: item})
		m.addMessage(m.message("tui.message.queued", l10n.A("position", len(m.queue)), l10n.A("label", item.Label)))
		return nil
	}
	m.nextRunID++
	m.actionGeneration++
	m.active = &activeAction{id: m.nextRunID, action: item, generation: m.actionGeneration, started: time.Now()}
	m.history.start(m.nextRunID, item.Label)
	if item.BlocksUntilDone {
		m.progressRun = m.nextRunID
		m.pushModal(progressModal)
	}
	m.addMessage(m.message("tui.message.started", l10n.A("label", item.Label)))
	return []Effect{{Kind: StartActionEffect, Action: item}}
}

func (m *Model) startNextQueued() []Effect {
	if m.active != nil || len(m.queue) == 0 {
		return nil
	}
	item := m.queue[0].action
	m.queue = m.queue[1:]
	return m.startOrQueue(item)
}

func (m *Model) openForm(templateID string) {
	form := FormState{}
	if templateID != "" {
		for i := range formTemplates {
			if formTemplates[i].ID == templateID {
				form.TemplateIndex = i
				break
			}
		}
		form.begin(m.snapshot)
	}
	m.form = &form
	m.filterActive = false
	m.confirmation = nil
	m.removeModal(menuModal)
	m.removeModal(menuSectionModal)
	m.removeModal(helpModal)
}

func (m *Model) openADOStateForm() {
	m.openForm("ado-set-state")
	if m.form == nil || m.selectedADOProject >= len(m.snapshot.ADOProjects) {
		return
	}
	project := m.snapshot.ADOProjects[m.selectedADOProject]
	if m.selectedADOItem >= len(project.Items) {
		return
	}
	item := project.Items[m.selectedADOItem]
	setField(m.form.Fields, "workItemIds", item.ID)
	setField(m.form.Fields, "project", project.Key)
	if actionItem, ok := findAction(item.Actions, ADOSetStateSlot); ok {
		for _, parameter := range requestParameters(actionItem.Request) {
			if parameter.Name == "state" {
				setField(m.form.Fields, "state", parameterString(parameter.Value))
			}
		}
	}
}

func (m *Model) openPRForm() {
	m.openForm("task-start-pr")
	if m.form == nil || m.selectedPR >= len(m.snapshot.PullRequests) {
		return
	}
	item := m.snapshot.PullRequests[m.selectedPR]
	setField(m.form.Fields, "pullRequest", item.ID)
	setField(m.form.Fields, "project", item.Project)
	setField(m.form.Fields, "repositories", item.Repository)
}

func (m *Model) openDBForm(template string) {
	m.openForm(template)
	if m.form == nil || m.selectedDB >= len(m.snapshot.Databases) {
		return
	}
	item := m.snapshot.Databases[m.selectedDB]
	setField(m.form.Fields, "project", item.Project)
	setField(m.form.Fields, "database", item.Key)
}

func requestParameters(request action.Request) []Parameter {
	if request == nil {
		return nil
	}
	if form, ok := request.(FormRequest); ok {
		return form.Parameters
	}
	if form, ok := request.(*FormRequest); ok {
		return form.Parameters
	}
	return nil
}
func parameterString(value any) string {
	if text, ok := value.(string); ok {
		return text
	}
	return ""
}
func setField(fields []FormField, id, value string) {
	for i := range fields {
		if fields[i].ID == id {
			fields[i].Value = value
			return
		}
	}
}

func (m *Model) selectedVisibleAction() (Action, bool) {
	items := m.visibleActions()
	if m.selectedAction >= len(items) {
		return Action{}, false
	}
	return items[m.selectedAction], true
}

func (m *Model) visibleActions() []Action {
	filter := strings.ToLower(strings.TrimSpace(m.filter))
	var result []Action
	for _, item := range m.snapshot.Actions {
		if item.MenuSection != "" || !item.Active {
			continue
		}
		if filter == "" || strings.Contains(strings.ToLower(item.Label), filter) || strings.Contains(strings.ToLower(item.Description), filter) {
			result = append(result, item)
		}
	}
	return result
}

func (m *Model) adoItemCount() int {
	if m.selectedADOProject >= len(m.snapshot.ADOProjects) {
		return 0
	}
	return len(m.snapshot.ADOProjects[m.selectedADOProject].Items)
}

func (m *Model) cycleADOProject(delta int) {
	count := len(m.snapshot.ADOProjects)
	if count == 0 {
		return
	}
	m.selectedADOProject = (m.selectedADOProject + delta + count) % count
	m.selectedADOItem = clampIndex(m.selectedADOItem, m.adoItemCount())
}

func (m *Model) clampSelections() {
	m.selectedAction = clampIndex(m.selectedAction, len(m.visibleActions()))
	m.selectedCockpit = clampIndex(m.selectedCockpit, len(m.snapshot.Cockpit))
	m.selectedWorkspace = clampIndex(m.selectedWorkspace, len(m.snapshot.Workspaces))
	m.selectedADOProject = clampIndex(m.selectedADOProject, len(m.snapshot.ADOProjects))
	m.selectedADOItem = clampIndex(m.selectedADOItem, m.adoItemCount())
	m.selectedPR = clampIndex(m.selectedPR, len(m.snapshot.PullRequests))
	m.selectedDB = clampIndex(m.selectedDB, len(m.snapshot.Databases))
}

func clampIndex(value, count int) int {
	if count <= 0 {
		return 0
	}
	if value < 0 {
		return 0
	}
	if value >= count {
		return count - 1
	}
	return value
}

// HandleWheel is the only mouse transition; click, release, drag, and motion
// messages are intentionally ignored by the Bubble Tea adapter.
func (m *Model) HandleWheel(delta int) {
	if delta == 0 {
		return
	}
	if modal, ok := m.topModal(); ok {
		switch modal {
		case journalModal:
			m.history.Scroll = max(0, m.history.Scroll+delta)
		case stateModal:
			m.stateScroll = max(0, m.stateScroll+delta)
		case detailModal:
			if m.detail != nil {
				m.detail.scroll = max(0, m.detail.scroll+delta)
			}
		case menuModal:
			m.selectedMenuSection = clampIndex(m.selectedMenuSection+delta, 4)
			m.selectedMenuItem = 0
		case menuSectionModal:
			m.selectedMenuItem = clampIndex(m.selectedMenuItem+delta, len(m.menuItems()))
		}
		return
	}
	key := Key{Kind: KeyRepeat, Code: "down"}
	if delta < 0 {
		key.Code = "up"
	}
	for range max(1, abs(delta)) {
		m.handleNavigationKey(key)
	}
}

func abs(value int) int {
	if value < 0 {
		return -value
	}
	return value
}

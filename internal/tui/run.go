package tui

import (
	"context"
	"errors"
	"fmt"
	"time"

	"charm.land/bubbles/v2/spinner"
	tea "charm.land/bubbletea/v2"
	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/l10n"
)

type snapshotLoadedMsg struct {
	generation uint64
	snapshot   Snapshot
	err        error
}
type workLoadedMsg struct {
	generation uint64
	items      []WorkProject
	err        error
}
type prsLoadedMsg struct {
	generation uint64
	items      []PullRequest
	err        error
}

type actionUpdate struct {
	runID      uint64
	generation uint64
	event      *action.EventEnvelope
	prompt     *actionPromptUpdate
	result     action.Result
	err        error
	done       bool
}

type actionPromptUpdate struct {
	prompt   action.Prompt
	response chan action.Response
}

type actionUpdateMsg struct{ update actionUpdate }
type externalRun struct {
	runID      uint64
	generation uint64
	item       Action
	result     action.Result
	process    ExternalProcess
}
type externalFinishedMsg struct {
	runID      uint64
	generation uint64
	err        error
}

// Run starts the Bubble Tea v2 program. Bubble Tea owns raw mode, alternate
// screen, mouse reporting, panic cleanup, and restoration on all return paths.
func Run(ctx context.Context, deps Dependencies) error {
	if deps.Runner == nil {
		return errors.New("tui.runner-required")
	}
	if deps.Snapshot == nil {
		return errors.New("tui.snapshot-loader-required")
	}
	if deps.ProjectResult == nil {
		return errors.New("tui.result-projector-required")
	}
	runContext, cancel := context.WithCancel(ctx)
	defer cancel()
	model := NewModel(deps)
	model.ctx = runContext
	options := []tea.ProgramOption{tea.WithContext(runContext)}
	if deps.Input != nil {
		options = append(options, tea.WithInput(deps.Input))
	}
	if deps.Output != nil {
		options = append(options, tea.WithOutput(deps.Output))
	}
	_, err := tea.NewProgram(model, options...).Run()
	return err
}

func (m *Model) Init() tea.Cmd {
	commands := []tea.Cmd{func() tea.Msg { return m.spinner.Tick() }}
	if command := m.startSnapshotLoad(); command != nil {
		commands = append(commands, command)
	}
	return tea.Batch(commands...)
}

func (m *Model) Update(message tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := message.(type) {
	case tea.WindowSizeMsg:
		m.width, m.height = msg.Width, msg.Height
		m.viewport.SetWidth(max(1, msg.Width-8))
		m.viewport.SetHeight(max(1, msg.Height-8))
		return m, nil
	case tea.KeyPressMsg:
		return m, m.applyEffects(m.HandleKey(keyFromPress(msg)))
	case tea.KeyReleaseMsg:
		m.HandleKey(Key{Code: msg.String(), Kind: KeyRelease})
		return m, nil
	case tea.PasteMsg:
		return m, m.applyEffects(m.HandleKey(Key{Code: "text", Text: msg.Content, Kind: KeyPress}))
	case tea.MouseWheelMsg:
		if msg.Button == tea.MouseWheelUp {
			m.HandleWheel(-1)
		}
		if msg.Button == tea.MouseWheelDown {
			m.HandleWheel(1)
		}
		return m, nil
	case snapshotLoadedMsg:
		return m, m.acceptSnapshot(msg)
	case workLoadedMsg:
		m.acceptWork(msg)
		return m, nil
	case prsLoadedMsg:
		m.acceptPullRequests(msg)
		return m, nil
	case actionUpdateMsg:
		return m, m.acceptActionUpdate(msg.update)
	case externalFinishedMsg:
		return m, m.acceptExternalFinished(msg)
	case spinner.TickMsg:
		var command tea.Cmd
		m.spinner, command = m.spinner.Update(msg)
		return m, command
	}
	return m, nil
}

func keyFromPress(msg tea.KeyPressMsg) Key {
	code := msg.String()
	text := msg.Text
	if text != "" && !msg.Mod.Contains(tea.ModCtrl) && !msg.Mod.Contains(tea.ModAlt) {
		if text == " " {
			code = "space"
		} else if len([]rune(text)) == 1 {
			code = text
		}
	}
	if msg.Mod.Contains(tea.ModCtrl) && msg.Code == ' ' {
		code = "ctrl+space"
	}
	if msg.Code == tea.KeyTab && msg.Mod.Contains(tea.ModShift) {
		code = "shift+tab"
	}
	kind := KeyPress
	if msg.IsRepeat {
		kind = KeyRepeat
	}
	return Key{Code: code, Text: text, Kind: kind, Ctrl: msg.Mod.Contains(tea.ModCtrl), Alt: msg.Mod.Contains(tea.ModAlt), Shift: msg.Mod.Contains(tea.ModShift)}
}

func (m *Model) applyEffects(effects []Effect) tea.Cmd {
	var commands []tea.Cmd
	for _, effect := range effects {
		switch effect.Kind {
		case QuitEffect:
			commands = append(commands, tea.Quit)
		case ReloadEffect:
			if command := m.startSnapshotLoad(); command != nil {
				commands = append(commands, command)
			}
		case StartActionEffect:
			if command := m.startActionRun(); command != nil {
				commands = append(commands, command)
			}
		case AnswerInputEffect:
			if effect.input != nil {
				select {
				case effect.input <- effect.Response:
				default:
				}
			}
		}
	}
	return tea.Batch(commands...)
}

func (m *Model) startSnapshotLoad() tea.Cmd {
	if m.deps.Snapshot == nil || m.snapshotLoad.running {
		return nil
	}
	m.snapshotLoad.generation++
	generation := m.snapshotLoad.generation
	m.snapshotLoad.running, m.snapshotLoad.started, m.snapshotLoad.errorText = true, time.Now(), ""
	m.workLoad.generation++
	m.prLoad.generation++
	m.workLoad.running, m.prLoad.running = false, false
	root, loader, ctx := m.snapshot.Root, m.deps.Snapshot, m.ctx
	if root == "" {
		root = m.deps.Root
	}
	m.addMessage(m.l10n.Text("tui.message.reload"))
	return func() tea.Msg {
		snapshot, err := loader(ctx, root)
		return snapshotLoadedMsg{generation: generation, snapshot: snapshot, err: err}
	}
}

func (m *Model) startWorkLoad() tea.Cmd {
	if m.deps.WorkItems == nil || m.workLoad.running || m.snapshot.NeedsInit {
		return nil
	}
	m.workLoad.generation++
	generation := m.workLoad.generation
	m.workLoad.running, m.workLoad.started, m.workLoad.errorText = true, time.Now(), ""
	loader, snapshot, ctx := m.deps.WorkItems, m.snapshot, m.ctx
	return func() tea.Msg {
		items, err := loader(ctx, snapshot)
		return workLoadedMsg{generation: generation, items: items, err: err}
	}
}

func (m *Model) startPRLoad() tea.Cmd {
	if m.deps.PullRequests == nil || m.prLoad.running || m.snapshot.NeedsInit {
		return nil
	}
	m.prLoad.generation++
	generation := m.prLoad.generation
	m.prLoad.running, m.prLoad.started, m.prLoad.errorText = true, time.Now(), ""
	loader, snapshot, ctx := m.deps.PullRequests, m.snapshot, m.ctx
	return func() tea.Msg {
		items, err := loader(ctx, snapshot)
		return prsLoadedMsg{generation: generation, items: items, err: err}
	}
}

func (m *Model) acceptSnapshot(msg snapshotLoadedMsg) tea.Cmd {
	if !m.snapshotLoad.running || msg.generation != m.snapshotLoad.generation {
		return nil
	}
	m.snapshotLoad.running = false
	if msg.err != nil {
		m.snapshotLoad.errorText = msg.err.Error()
		m.addMessage(m.message("tui.message.load-failed", l10n.A("label", m.l10n.Text("tui.status.snapshot")), l10n.A("error", msg.err)))
		return nil
	}
	m.snapshot = msg.snapshot
	if m.snapshot.Root == "" {
		m.snapshot.Root = m.deps.Root
	}
	m.clampSelections()
	m.addMessage(m.message("tui.message.loaded", l10n.A("label", m.l10n.Text("tui.status.snapshot")), l10n.A("count", len(m.snapshot.Workspaces))))
	if m.snapshot.NeedsInit {
		return nil
	}
	return tea.Batch(m.startWorkLoad(), m.startPRLoad())
}

func (m *Model) acceptWork(msg workLoadedMsg) {
	if !m.workLoad.running || msg.generation != m.workLoad.generation {
		return
	}
	m.workLoad.running = false
	if msg.err != nil {
		m.workLoad.errorText = msg.err.Error()
		m.addMessage(m.message("tui.message.load-failed", l10n.A("label", m.l10n.Text("tui.status.work")), l10n.A("error", msg.err)))
		return
	}
	m.snapshot.WorkProjects = msg.items
	m.clampSelections()
	count := 0
	for _, project := range msg.items {
		count += len(project.Items)
	}
	m.addMessage(m.message("tui.message.loaded", l10n.A("label", m.l10n.Text("tui.status.work")), l10n.A("count", count)))
}

func (m *Model) acceptPullRequests(msg prsLoadedMsg) {
	if !m.prLoad.running || msg.generation != m.prLoad.generation {
		return
	}
	m.prLoad.running = false
	if msg.err != nil {
		m.prLoad.errorText = msg.err.Error()
		m.addMessage(m.message("tui.message.load-failed", l10n.A("label", m.l10n.Text("tui.status.prs")), l10n.A("error", msg.err)))
		return
	}
	m.snapshot.PullRequests = msg.items
	m.clampSelections()
	m.addMessage(m.message("tui.message.loaded", l10n.A("label", m.l10n.Text("tui.status.prs")), l10n.A("count", len(msg.items))))
}

func (m *Model) startActionRun() tea.Cmd {
	if m.active == nil || m.deps.Runner == nil {
		return nil
	}
	active := *m.active
	updates := make(chan actionUpdate, 16)
	m.actionUpdates = updates
	runner, ctx := m.deps.Runner, m.ctx
	project := m.deps.ProjectEvent
	localizer := m.l10n
	return func() tea.Msg {
		go func() {
			runtime := action.Runtime{
				Events: action.EventSinkFunc(func(eventCtx context.Context, event action.EventEnvelope) error {
					select {
					case updates <- actionUpdate{runID: active.id, generation: active.generation, event: &event}:
						return nil
					case <-eventCtx.Done():
						return eventCtx.Err()
					}
				}),
				Input: action.InputPortFunc(func(inputCtx context.Context, prompt action.Prompt) (action.Response, error) {
					responses := make(chan action.Response, 1)
					select {
					case updates <- actionUpdate{runID: active.id, generation: active.generation, prompt: &actionPromptUpdate{prompt: prompt, response: responses}}:
					case <-inputCtx.Done():
						return action.Response{}, inputCtx.Err()
					}
					select {
					case response, ok := <-responses:
						if !ok {
							return action.Response{}, fmt.Errorf("tui.input-canceled:%s", prompt.ID)
						}
						return response, nil
					case <-inputCtx.Done():
						return action.Response{}, inputCtx.Err()
					}
				}),
			}
			result, err := runner.Run(ctx, active.action.Request, runtime)
			updates <- actionUpdate{runID: active.id, generation: active.generation, result: result, err: err, done: true}
			close(updates)
		}()
		_ = project
		_ = localizer
		update, ok := <-updates
		if !ok {
			return actionUpdateMsg{update: actionUpdate{runID: active.id, generation: active.generation, err: errors.New("tui.action-stream-closed"), done: true}}
		}
		return actionUpdateMsg{update: update}
	}
}

func waitForAction(updates <-chan actionUpdate) tea.Cmd {
	if updates == nil {
		return nil
	}
	return func() tea.Msg {
		update, ok := <-updates
		if !ok {
			return nil
		}
		return actionUpdateMsg{update: update}
	}
}

func (m *Model) acceptActionUpdate(update actionUpdate) tea.Cmd {
	if m.active == nil || update.runID != m.active.id || update.generation != m.active.generation {
		return nil
	}
	if update.event != nil {
		level, scope, text := InfoLevel, string(update.event.Action), m.l10n.Render(update.event.Message)
		if m.deps.ProjectEvent != nil {
			level, scope, text = m.deps.ProjectEvent(*update.event)
		}
		m.history.appendEvent(update.runID, RecordedEvent{At: time.Now().UTC(), Raw: *update.event, Level: level, Scope: scope, Text: text})
		return waitForAction(m.actionUpdates)
	}
	if update.prompt != nil {
		m.openInputPrompt(update.runID, update.prompt)
		return waitForAction(m.actionUpdates)
	}
	if !update.done {
		return waitForAction(m.actionUpdates)
	}
	if update.err != nil {
		return m.finishActionFailure(update.runID, update.err)
	}
	lines := m.deps.ProjectResult(update.result)
	var external *ExternalProcess
	if m.deps.ProjectExternal != nil {
		if process, ok := m.deps.ProjectExternal(update.result); ok {
			external = &process
		}
	}
	if external != nil {
		m.pendingExternal = &externalRun{runID: update.runID, generation: update.generation, item: m.active.action, result: update.result, process: *external}
		process := external.command()
		return tea.ExecProcess(process, func(err error) tea.Msg {
			return externalFinishedMsg{runID: update.runID, generation: update.generation, err: err}
		})
	}
	return m.finishActionSuccess(update.runID, update.result, lines, nil)
}

func (m *Model) openInputPrompt(runID uint64, update *actionPromptUpdate) {
	prompt := update.prompt
	choices := make([]string, 0, len(prompt.Choices))
	for _, choice := range prompt.Choices {
		choices = append(choices, m.l10n.Render(choice.Label))
	}
	label := m.l10n.Render(prompt.Label)
	help := ""
	if prompt.Help != nil {
		help = m.l10n.Render(*prompt.Help)
	}
	selected := 0
	if prompt.Default != nil {
		for i := range prompt.Choices {
			if prompt.Choices[i].Value == *prompt.Default {
				selected = i
				break
			}
		}
	}
	m.prompt = &inputPrompt{runID: runID, prompt: prompt, label: label, help: help, choices: choices, selected: selected, selectedMany: make([]bool, len(choices)), response: update.response}
	m.addMessage(m.message("tui.message.input", l10n.A("label", label)))
}

func (m *Model) finishActionFailure(runID uint64, err error) tea.Cmd {
	label := m.active.action.Label
	m.history.finish(runID, RunFailed, nil, err.Error(), nil)
	m.addMessage(m.message("tui.message.failed", l10n.A("label", label), l10n.A("error", err)))
	m.progressRun = 0
	m.removeModal(progressModal)
	m.active, m.actionUpdates = nil, nil
	m.pushModal(journalModal)
	return m.continueQueue()
}

func (m *Model) finishActionSuccess(runID uint64, result action.Result, lines []string, external *ExternalProcess) tea.Cmd {
	item := m.active.action
	m.history.finish(runID, RunSucceeded, lines, "", external)
	m.addMessage(m.message("tui.message.done", l10n.A("label", item.Label), l10n.A("status", m.l10n.Text("tui.status.ok"))))
	m.progressRun = 0
	m.removeModal(progressModal)
	if m.deps.ProjectState != nil {
		m.applyStateEffect(m.deps.ProjectState(result))
	}
	if item.OpenResult && len(lines) != 0 {
		m.detail = &detailState{title: item.Label, lines: append([]string(nil), lines...)}
		m.pushModal(detailModal)
	}
	if item.RefreshAfterSuccess {
		m.reloadAfterQueue = true
	}
	m.active, m.actionUpdates = nil, nil
	return m.continueQueue()
}

func (m *Model) acceptExternalFinished(msg externalFinishedMsg) tea.Cmd {
	pending := m.pendingExternal
	if pending == nil || pending.runID != msg.runID || pending.generation != msg.generation || m.active == nil {
		return nil
	}
	m.pendingExternal = nil
	if msg.err != nil {
		m.addMessage(m.message("tui.message.external-failed", l10n.A("label", pending.item.Label), l10n.A("error", msg.err)))
		return m.finishActionFailure(msg.runID, msg.err)
	}
	m.addMessage(m.message("tui.message.external-finished", l10n.A("label", pending.item.Label)))
	return m.finishActionSuccess(msg.runID, pending.result, nil, &pending.process)
}

func (m *Model) continueQueue() tea.Cmd {
	effects := m.startNextQueued()
	if len(effects) != 0 {
		return m.applyEffects(effects)
	}
	if m.reloadAfterQueue {
		m.reloadAfterQueue = false
		return m.startSnapshotLoad()
	}
	return nil
}

func (m *Model) applyStateEffect(effect *StateEffect) {
	if effect == nil {
		return
	}
	if effect.Root != nil {
		m.snapshot.Root = *effect.Root
		m.deps.Root = *effect.Root
	}
	if effect.DefaultAgent != nil {
		m.snapshot.DefaultAgent = *effect.DefaultAgent
	}
	if effect.ColorMode != nil {
		m.snapshot.ColorMode = *effect.ColorMode
	}
	if effect.Initialized {
		m.snapshot.NeedsInit = false
	}
}

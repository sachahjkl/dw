package tui

import (
	"context"
	"errors"
	"time"

	"charm.land/bubbles/v2/spinner"
	tea "charm.land/bubbletea/v2"
	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/cockpit"
	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/execution"
	"github.com/sachahjkl/dw/internal/l10n"
)

type snapshotLoadedMsg struct {
	generation uint64
	snapshot   cockpit.Snapshot
	err        error
}
type workLoadedMsg struct {
	generation uint64
	items      []cockpit.WorkProject
	err        error
}
type prsLoadedMsg struct {
	generation uint64
	items      []cockpit.PullRequest
	err        error
}

type persistedHistory struct {
	record execution.Record
	events []execution.Event
}

type historyLoadedMsg struct {
	items []persistedHistory
	err   error
}

type actionUpdate struct {
	runID      execution.ExecutionID
	generation uint64
	event      *execution.Event
	prompt     action.Prompt
	result     action.Result
	status     execution.Status
	err        error
	submitted  bool
	done       bool
}

type actionUpdateMsg struct{ update actionUpdate }
type externalRun struct {
	runID      execution.ExecutionID
	generation uint64
	item       Action
	result     action.Result
	process    ExternalProcess
}
type externalFinishedMsg struct {
	runID      execution.ExecutionID
	generation uint64
	err        error
}

// Run starts the Bubble Tea v2 program. Bubble Tea owns raw mode, alternate
// screen, mouse reporting, panic cleanup, and restoration on all return paths.
func Run(ctx context.Context, deps Dependencies) error {
	if deps.Executor == nil || deps.Actor.Principal == "" {
		return errors.New("tui.executor-required")
	}
	if deps.Cockpit == nil {
		return errors.New("tui.cockpit-service-required")
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
	commands := []tea.Cmd{func() tea.Msg { return m.spinner.Tick() }, m.loadHistory()}
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
	case snapshotLoadedMsg:
		return m, m.acceptSnapshot(msg)
	case workLoadedMsg:
		m.acceptWork(msg)
		return m, nil
	case prsLoadedMsg:
		m.acceptPullRequests(msg)
		return m, nil
	case historyLoadedMsg:
		return m, m.acceptHistory(msg)
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
			current := m.active
			if current != nil {
				executor, actor, ctx := m.deps.Executor, m.deps.Actor, m.ctx
				generation := current.generation
				commands = append(commands, func() tea.Msg {
					err := executor.Respond(ctx, actor, effect.ExecutionID, effect.PromptID, effect.Response)
					if err == nil {
						return nil
					}
					return actionUpdateMsg{update: actionUpdate{runID: effect.ExecutionID, generation: generation, err: err, status: execution.StatusFailed, done: true}}
				})
			}
		case CancelExecutionEffect:
			executor, actor := m.deps.Executor, m.deps.Actor
			commands = append(commands, func() tea.Msg {
				_ = executor.Cancel(context.Background(), actor, effect.ExecutionID)
				return nil
			})
		}
	}
	return tea.Batch(commands...)
}

func (m *Model) loadHistory() tea.Cmd {
	executor, actor, root, ctx := m.deps.Executor, m.deps.Actor, m.deps.Root, m.ctx
	return func() tea.Msg {
		records, err := executor.List(ctx, actor, execution.ListFilter{Root: root, Limit: 500})
		if err != nil {
			return historyLoadedMsg{err: err}
		}
		items := make([]persistedHistory, 0, len(records))
		for _, listed := range records {
			record, getErr := executor.Get(ctx, actor, listed.ExecutionID)
			if getErr != nil {
				return historyLoadedMsg{err: getErr}
			}
			item := persistedHistory{record: record}
			if record.Status.Terminal() {
				subscription, subscribeErr := executor.Subscribe(ctx, actor, record.ExecutionID, 0)
				if subscribeErr != nil {
					return historyLoadedMsg{err: subscribeErr}
				}
				for event := range subscription.Events {
					item.events = append(item.events, event)
				}
				for streamErr := range subscription.Errors {
					if streamErr != nil {
						return historyLoadedMsg{err: streamErr}
					}
				}
			}
			items = append(items, item)
		}
		return historyLoadedMsg{items: items}
	}
}

func (m *Model) acceptHistory(msg historyLoadedMsg) tea.Cmd {
	if msg.err != nil {
		m.addMessage(m.errorText(msg.err))
		return nil
	}
	var resume *execution.Record
	for index := len(msg.items) - 1; index >= 0; index-- {
		item := msg.items[index]
		run := RunRecord{ID: item.record.ExecutionID, Label: string(item.record.ActionID), Status: item.record.Status}
		if item.record.TypedResult != nil {
			run.Lines = m.deps.ProjectResult(item.record.TypedResult)
		}
		if item.record.Failure != nil {
			run.Error = m.errorText(execution.NewFailureError(*item.record.Failure))
		}
		m.history.load(run)
		for eventIndex := range item.events {
			if recorded, err := m.projectExecutionEvent(item.events[eventIndex]); err == nil {
				m.history.appendEvent(item.record.ExecutionID, recorded)
			}
		}
		if !item.record.Status.Terminal() && (resume == nil || resumePriority(item.record.Status) > resumePriority(resume.Status)) {
			record := item.record
			resume = &record
		}
	}
	if resume == nil {
		return nil
	}
	return m.resumeActionRun(*resume)
}

func resumePriority(status execution.Status) int {
	switch status {
	case execution.StatusWaitingInput:
		return 4
	case execution.StatusRunning:
		return 3
	case execution.StatusCanceling:
		return 2
	case execution.StatusQueued:
		return 1
	default:
		return 0
	}
}

func (m *Model) startSnapshotLoad() tea.Cmd {
	if m.deps.Cockpit == nil || m.snapshotLoad.running {
		return nil
	}
	m.snapshotLoad.generation++
	generation := m.snapshotLoad.generation
	m.snapshotLoad.running, m.snapshotLoad.started, m.snapshotLoad.errorText = true, time.Now(), ""
	m.workLoad.generation++
	m.prLoad.generation++
	m.workLoad.running, m.prLoad.running = false, false
	root, service, ctx := m.snapshot.Root, m.deps.Cockpit, m.ctx
	if root == "" {
		root = m.deps.Root
	}
	m.addMessage(m.l10n.Text("tui.message.reload"))
	return func() tea.Msg {
		snapshot, err := service.Snapshot(ctx, root)
		return snapshotLoadedMsg{generation: generation, snapshot: snapshot, err: err}
	}
}

func (m *Model) startWorkLoad() tea.Cmd {
	if m.deps.Cockpit == nil || m.workLoad.running || m.snapshot.NeedsInit {
		return nil
	}
	m.workLoad.generation++
	generation := m.workLoad.generation
	m.workLoad.running, m.workLoad.started, m.workLoad.errorText = true, time.Now(), ""
	service, snapshot, ctx := m.deps.Cockpit, m.snapshot, m.ctx
	return func() tea.Msg {
		items, err := service.Work(ctx, snapshot)
		return workLoadedMsg{generation: generation, items: items, err: err}
	}
}

func (m *Model) startPRLoad() tea.Cmd {
	if m.deps.Cockpit == nil || m.prLoad.running || m.snapshot.NeedsInit {
		return nil
	}
	m.prLoad.generation++
	generation := m.prLoad.generation
	m.prLoad.running, m.prLoad.started, m.prLoad.errorText = true, time.Now(), ""
	service, snapshot, ctx := m.deps.Cockpit, m.snapshot, m.ctx
	return func() tea.Msg {
		items, err := service.PullRequests(ctx, snapshot)
		return prsLoadedMsg{generation: generation, items: items, err: err}
	}
}

func (m *Model) acceptSnapshot(msg snapshotLoadedMsg) tea.Cmd {
	if !m.snapshotLoad.running || msg.generation != m.snapshotLoad.generation {
		return nil
	}
	m.snapshotLoad.running = false
	if msg.err != nil {
		m.snapshotLoad.errorText = m.errorText(msg.err)
		m.addMessage(m.message("tui.message.load-failed", l10n.A("label", m.l10n.Text("tui.status.snapshot")), l10n.A("error", m.snapshotLoad.errorText)))
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
		m.workLoad.errorText = m.errorText(msg.err)
		m.addMessage(m.message("tui.message.load-failed", l10n.A("label", m.l10n.Text("tui.status.work")), l10n.A("error", m.workLoad.errorText)))
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
		m.prLoad.errorText = m.errorText(msg.err)
		m.addMessage(m.message("tui.message.load-failed", l10n.A("label", m.l10n.Text("tui.status.prs")), l10n.A("error", m.prLoad.errorText)))
		return
	}
	m.snapshot.PullRequests = msg.items
	m.clampSelections()
	m.addMessage(m.message("tui.message.loaded", l10n.A("label", m.l10n.Text("tui.status.prs")), l10n.A("count", len(msg.items))))
}

func (m *Model) startActionRun() tea.Cmd {
	if m.active == nil || m.deps.Executor == nil {
		return nil
	}
	active := *m.active
	updates := make(chan actionUpdate, 16)
	m.actionUpdates = updates
	executor, actor, ctx := m.deps.Executor, m.deps.Actor, m.ctx
	builder := m.deps.RequestBuilder
	root := m.snapshot.Root
	if root == "" {
		root = m.deps.Root
	}
	return func() tea.Msg {
		go func() {
			defer close(updates)
			request := active.action.Request
			var err error
			if builder != nil {
				request, err = builder(ctx, request)
				if err != nil {
					updates <- actionUpdate{generation: active.generation, err: err, status: execution.StatusFailed, done: true}
					return
				}
			}
			key, err := execution.NewIdempotencyKey()
			if err != nil {
				updates <- actionUpdate{generation: active.generation, err: err, status: execution.StatusFailed, done: true}
				return
			}
			id, err := executor.Submit(ctx, execution.Submission{Request: request, Root: root, Actor: actor, IdempotencyKey: key})
			if err != nil {
				updates <- actionUpdate{generation: active.generation, err: err, status: execution.StatusFailed, done: true}
				return
			}
			send := actionUpdateSender(ctx, updates)
			if !send(actionUpdate{runID: id, generation: active.generation, status: execution.StatusQueued, submitted: true}) {
				return
			}
			streamActionUpdates(ctx, executor, actor, id, active.generation, send)
		}()
		update, ok := <-updates
		if !ok {
			return actionUpdateMsg{update: actionUpdate{generation: active.generation, err: errors.New("tui.action-stream-closed"), status: execution.StatusFailed, done: true}}
		}
		return actionUpdateMsg{update: update}
	}
}

func (m *Model) resumeActionRun(record execution.Record) tea.Cmd {
	m.actionGeneration++
	generation := m.actionGeneration
	m.active = &activeAction{
		id: record.ExecutionID, action: Action{ID: record.ActionID, Label: string(record.ActionID)},
		generation: generation, started: record.CreatedAt,
	}
	updates := make(chan actionUpdate, 16)
	m.actionUpdates = updates
	executor, actor, ctx := m.deps.Executor, m.deps.Actor, m.ctx
	return func() tea.Msg {
		go func() {
			defer close(updates)
			streamActionUpdates(ctx, executor, actor, record.ExecutionID, generation, actionUpdateSender(ctx, updates))
		}()
		update, ok := <-updates
		if !ok {
			return actionUpdateMsg{update: actionUpdate{runID: record.ExecutionID, generation: generation, err: errors.New("tui.action-stream-closed"), status: execution.StatusFailed, done: true}}
		}
		return actionUpdateMsg{update: update}
	}
}

func actionUpdateSender(ctx context.Context, updates chan<- actionUpdate) func(actionUpdate) bool {
	return func(update actionUpdate) bool {
		select {
		case updates <- update:
			return true
		case <-ctx.Done():
			return false
		}
	}
}

func streamActionUpdates(ctx context.Context, executor execution.Executor, actor execution.Actor, id execution.ExecutionID, generation uint64, send func(actionUpdate) bool) {
	subscription, err := executor.Subscribe(ctx, actor, id, 0)
	if err != nil {
		send(actionUpdate{runID: id, generation: generation, err: err, status: execution.StatusFailed, done: true})
		return
	}
	for event := range subscription.Events {
		eventCopy := event
		if !send(actionUpdate{runID: id, generation: generation, event: &eventCopy}) {
			return
		}
		if event.Kind != execution.EventInputRequired {
			continue
		}
		record, getErr := executor.Get(ctx, actor, id)
		if getErr != nil {
			send(actionUpdate{runID: id, generation: generation, err: getErr, status: execution.StatusFailed, done: true})
			return
		}
		if record.PendingPrompt == nil {
			continue
		}
		prompt, decodeErr := execution.DecodePrompt(*record.PendingPrompt)
		if decodeErr != nil {
			send(actionUpdate{runID: id, generation: generation, err: decodeErr, status: execution.StatusFailed, done: true})
			return
		}
		if !send(actionUpdate{runID: id, generation: generation, prompt: prompt}) {
			return
		}
	}
	select {
	case streamErr, open := <-subscription.Errors:
		if open && streamErr != nil {
			send(actionUpdate{runID: id, generation: generation, err: streamErr, status: execution.StatusFailed, done: true})
			return
		}
	default:
	}
	record, waitErr := executor.Wait(ctx, actor, id)
	if waitErr == nil && record.Failure != nil {
		waitErr = execution.NewFailureError(*record.Failure)
	}
	if waitErr == nil && record.Status == execution.StatusCanceled {
		waitErr = context.Canceled
	}
	send(actionUpdate{runID: id, generation: generation, result: record.TypedResult, status: record.Status, err: waitErr, done: true})
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

func (m *Model) projectExecutionEvent(event execution.Event) (RecordedEvent, error) {
	message, err := execution.DecodeMessage(event.Message)
	if err != nil {
		return RecordedEvent{}, err
	}
	kind := action.EventProgress
	if event.Kind == execution.EventWarning {
		kind = action.EventWarning
	} else if event.Kind == execution.EventLog {
		kind = action.EventLog
	}
	raw := action.EventEnvelope{Action: event.ActionID, Kind: kind, Message: message, Data: event.TypedData}
	level, scope, text := InfoLevel, string(raw.Action), m.l10n.Render(raw.Message)
	if m.deps.ProjectEvent != nil && (event.Kind == execution.EventProgress || event.Kind == execution.EventWarning || event.Kind == execution.EventLog) {
		level, scope, text = m.deps.ProjectEvent(raw)
	}
	return RecordedEvent{At: event.At, Raw: raw, Level: level, Scope: scope, Text: text}, nil
}

func (m *Model) acceptActionUpdate(update actionUpdate) tea.Cmd {
	if m.active == nil || update.generation != m.active.generation {
		return nil
	}
	if update.submitted {
		m.active.id = update.runID
		m.history.start(update.runID, m.active.action.Label, update.status)
		if m.active.action.BlocksUntilDone {
			m.progressRun = update.runID
		}
		return waitForAction(m.actionUpdates)
	}
	if update.runID != m.active.id {
		return nil
	}
	if update.event != nil {
		recorded, err := m.projectExecutionEvent(*update.event)
		if err != nil {
			return m.finishActionFailure(update.runID, update.result, nil, execution.StatusFailed, err)
		}
		m.history.appendEvent(update.runID, recorded)
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
		var lines []string
		if update.result != nil {
			lines = m.deps.ProjectResult(update.result)
		}
		return m.finishActionFailure(update.runID, update.result, lines, update.status, update.err)
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

func (m *Model) openInputPrompt(runID execution.ExecutionID, prompt action.Prompt) {
	meta, choices, defaultOne, defaults := promptPresentation(prompt)
	labels := make([]string, 0, len(choices))
	for _, choice := range choices {
		labels = append(labels, m.l10n.Render(choice.Label))
	}
	label := m.l10n.Render(meta.Label)
	help := ""
	if meta.Help != nil {
		help = m.l10n.Render(*meta.Help)
	}
	selected := 0
	selectedMany := make([]bool, len(choices))
	for index, choice := range choices {
		if defaultOne != nil && choice.Value == *defaultOne {
			selected = index
		}
		for _, value := range defaults {
			if choice.Value == value {
				selectedMany[index] = true
				break
			}
		}
	}
	m.prompt = &inputPrompt{executionID: runID, prompt: prompt, label: label, help: help, choices: labels, selected: selected, selectedMany: selectedMany}
	m.addMessage(m.message("tui.message.input", l10n.A("label", label)))
}

func promptPresentation(prompt action.Prompt) (action.PromptMeta, []action.Choice, *action.ChoiceValue, []action.ChoiceValue) {
	switch typed := prompt.(type) {
	case action.TextPrompt:
		return typed.Meta, nil, nil, nil
	case action.SecretPrompt:
		return typed.Meta, nil, nil, nil
	case action.ConfirmPrompt:
		return typed.Meta, nil, nil, nil
	case action.SelectOnePrompt:
		return typed.Meta, typed.Choices, typed.Default, nil
	case action.SelectManyPrompt:
		return typed.Meta, typed.Choices, nil, typed.Defaults
	default:
		return action.PromptMeta{}, nil, nil, nil
	}
}

func (m *Model) finishActionFailure(runID execution.ExecutionID, result action.Result, lines []string, status execution.Status, err error) tea.Cmd {
	label := m.active.action.Label
	errorText := m.errorText(err)
	if !status.Terminal() {
		status = execution.StatusFailed
	}
	m.history.finish(runID, status, lines, errorText, nil)
	m.addMessage(m.message("tui.message.failed", l10n.A("label", label), l10n.A("error", errorText)))
	m.progressRun = execution.ExecutionID{}
	m.removeModal(progressModal)
	if result != nil && m.deps.ProjectState != nil {
		m.applyStateEffect(m.deps.ProjectState(result))
	}
	detailLines := append(append([]string(nil), lines...), errorText)
	m.detail = &detailState{title: label, lines: detailLines}
	m.active, m.actionUpdates = nil, nil
	m.pushModal(detailModal)
	return m.continueQueue()
}

func (m *Model) errorText(err error) string {
	return console.LocalizedErrorText(m.l10n, err)
}

func (m *Model) finishActionSuccess(runID execution.ExecutionID, result action.Result, lines []string, external *ExternalProcess) tea.Cmd {
	item := m.active.action
	m.history.finish(runID, execution.StatusSucceeded, lines, "", external)
	m.addMessage(m.message("tui.message.done", l10n.A("label", item.Label), l10n.A("status", m.l10n.Text("tui.status.ok"))))
	m.progressRun = execution.ExecutionID{}
	m.removeModal(progressModal)
	if m.deps.ProjectState != nil {
		m.applyStateEffect(m.deps.ProjectState(result))
	}
	resultLines := append([]string(nil), lines...)
	if len(resultLines) == 0 {
		if external != nil {
			resultLines = []string{m.l10n.Text("tui.result.external-complete")}
		} else {
			resultLines = []string{m.l10n.Text("tui.result.complete")}
		}
	}
	m.detail = &detailState{title: item.Label, lines: resultLines}
	m.pushModal(detailModal)
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
		return m.finishActionFailure(msg.runID, pending.result, nil, execution.StatusFailed, msg.err)
	}
	m.addMessage(m.message("tui.message.external-finished", l10n.A("label", pending.item.Label)))
	return m.finishActionSuccess(msg.runID, pending.result, nil, &pending.process)
}

func (m *Model) continueQueue() tea.Cmd {
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

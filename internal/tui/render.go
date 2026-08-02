package tui

import (
	"fmt"
	"image/color"
	"strings"
	"time"

	tea "charm.land/bubbletea/v2"
	"charm.land/lipgloss/v2"
	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/execution"
	"github.com/sachahjkl/dw/internal/l10n"
)

type palette struct {
	primary, accent, good, warn, bad, muted, text, surface, selected color.Color
}

var colors = palette{
	primary: lipgloss.Color("#7AA2F7"), accent: lipgloss.Color("#2AC3DE"), good: lipgloss.Color("#9ECE6A"), warn: lipgloss.Color("#E0AF68"),
	bad: lipgloss.Color("#F7768E"), muted: lipgloss.Color("#727B9A"), text: lipgloss.Color("#C0CAF5"), surface: lipgloss.Color("#1A1B26"), selected: lipgloss.Color("#283457"),
}

func (m *Model) View() tea.View {
	view := tea.NewView(m.render())
	view.AltScreen = true
	view.MouseMode = tea.MouseModeNone
	view.WindowTitle = m.l10n.Text(msgAppTitle)
	view.KeyboardEnhancements.ReportEventTypes = true
	view.KeyboardEnhancements.ReportAlternateKeys = true
	return view
}

func (m *Model) render() string {
	width, height := m.width, m.height
	if width <= 0 {
		width = 100
	}
	if height <= 0 {
		height = 30
	}
	if width < 50 || height < 14 {
		return lipgloss.NewStyle().Width(width).Height(height).Align(lipgloss.Center, lipgloss.Center).Foreground(colors.warn).Render(m.l10n.Text("tui.small"))
	}

	if m.snapshot.NeedsInit {
		return m.renderInit(width, height)
	}
	if m.prompt != nil {
		return m.renderInput(width, height)
	}
	if m.form != nil {
		return m.renderForm(*m.form, width, height, true)
	}
	if m.confirmation != nil {
		return m.renderConfirmation(width, height)
	}
	if modal, ok := m.topModal(); ok {
		return m.renderModal(modal, width, height)
	}

	header := m.renderHeader(width)
	footer := m.renderFooter(width)
	errorBanner := m.renderLoaderErrors(width)
	bodyHeight := max(1, height-lipgloss.Height(header)-lipgloss.Height(errorBanner)-lipgloss.Height(footer))
	body := lipgloss.NewStyle().Width(width).Height(bodyHeight).Render(m.renderBody(width, bodyHeight))
	parts := []string{header}
	if errorBanner != "" {
		parts = append(parts, errorBanner)
	}
	parts = append(parts, body, footer)
	return lipgloss.JoinVertical(lipgloss.Left, parts...)
}

func (m *Model) renderHeader(width int) string {
	renderTabs := func(compact bool) string {
		tabs := make([]string, 0, len(allViews))
		for i, view := range allViews {
			label := m.viewLabel(view)
			if compact && view != m.view {
				label = ""
			}
			text := fmt.Sprintf("%d", i+1)
			if label != "" {
				text += " " + label
			}
			style := lipgloss.NewStyle().Foreground(colors.muted).Padding(0, 1)
			if view == m.view {
				style = style.Foreground(colors.surface).Background(colors.primary).Bold(true)
			}
			tabs = append(tabs, style.Render(text))
		}
		return lipgloss.JoinHorizontal(lipgloss.Top, tabs...)
	}
	left := renderTabs(false)
	right := lipgloss.NewStyle().Foreground(colors.muted).Render(m.statusSummary())
	if lipgloss.Width(left)+1+lipgloss.Width(right) > width {
		left = renderTabs(true)
	}
	availableRight := max(0, width-lipgloss.Width(left)-1)
	right = truncate(right, availableRight)
	gap := max(1, width-lipgloss.Width(left)-lipgloss.Width(right))
	bar := truncate(left+strings.Repeat(" ", gap)+right, width)
	return lipgloss.NewStyle().Width(width).MaxHeight(1).Background(colors.surface).Render(bar)
}

func (m *Model) statusSummary() string {
	var parts []string
	if m.snapshotLoad.running {
		parts = append(parts, m.loadingText(m.l10n.Text("tui.status.snapshot"), m.snapshotLoad))
	}
	if m.workLoad.running {
		parts = append(parts, m.loadingText(m.l10n.Text("tui.status.work"), m.workLoad))
	}
	if m.prLoad.running {
		parts = append(parts, m.loadingText(m.l10n.Text("tui.status.prs"), m.prLoad))
	}
	if m.active != nil {
		text := m.active.action.Label
		if run := m.history.active(); run != nil && len(run.Events) != 0 {
			text = run.Events[len(run.Events)-1].Text
		}
		parts = append(parts, m.spinner.View()+" "+text)
	}
	parts = append(parts, m.snapshot.Root)
	return strings.Join(parts, "  ")
}

func (m *Model) loadingText(label string, state loaderState) string {
	elapsed := time.Since(state.started).Truncate(time.Second)
	return m.spinner.View() + " " + m.message("tui.status.elapsed", l10n.A("label", label), l10n.A("state", m.l10n.Text("tui.status.loading")), l10n.A("elapsed", elapsed))
}

func (m *Model) renderLoaderErrors(width int) string {
	var errors []string
	for _, item := range []struct {
		label string
		state loaderState
	}{
		{m.l10n.Text("tui.status.snapshot"), m.snapshotLoad},
		{m.l10n.Text("tui.status.work"), m.workLoad},
		{m.l10n.Text("tui.status.prs"), m.prLoad},
	} {
		if item.state.errorText != "" {
			errors = append(errors, item.label+": "+item.state.errorText)
		}
	}
	if len(errors) == 0 {
		return ""
	}
	text := m.l10n.Text("tui.load.error-prefix") + " " + strings.Join(errors, " · ") + "  " + m.l10n.Text("tui.load.retry")
	return lipgloss.NewStyle().Width(width).MaxHeight(1).Foreground(colors.bad).Background(colors.surface).Render(truncate(text, width))
}

func (m *Model) renderFooter(width int) string {
	legend := m.l10n.Text("tui.keys.global")
	switch m.view {
	case Dashboard:
		legend = m.l10n.Text("tui.keys.dashboard") + "  " + legend
	case Workspaces:
		legend = m.l10n.Text("tui.keys.workspaces") + "  " + legend
	case Work:
		legend = m.l10n.Text("tui.keys.work") + "  " + legend
	case PullRequests:
		legend = m.l10n.Text("tui.keys.prs") + "  " + legend
	case Data:
		legend = m.l10n.Text("tui.keys.data") + "  " + legend
	case Composer:
		if m.composer.Mode == ChooseTemplate {
			legend = m.l10n.Text("tui.keys.composer.choose") + "  " + legend
		} else {
			legend = m.l10n.Text("tui.keys.composer.edit") + "  " + legend
		}
	}
	return lipgloss.NewStyle().Width(width).Foreground(colors.text).Background(colors.surface).Padding(0, 1).Render(wrapLine(legend, width-2, 3))
}

func (m *Model) renderBody(width, height int) string {
	switch m.view {
	case Dashboard:
		return m.renderDashboard(width, height)
	case Workspaces:
		return m.renderWorkspaces(width, height)
	case Work:
		return m.renderWork(width, height)
	case PullRequests:
		return m.renderPRs(width, height)
	case Data:
		return m.renderData(width, height)
	case Composer:
		return m.renderForm(m.composer, width, height, false)
	}
	return ""
}

func (m *Model) renderDashboard(width, height int) string {
	metrics := [][]string{
		{m.l10n.Text("tui.label.projects"), fmt.Sprint(m.snapshot.ProjectCount)},
		{m.l10n.Text("tui.label.repositories"), fmt.Sprint(m.snapshot.RepositoryCount)},
		{m.l10n.Text("tui.label.workspaces"), fmt.Sprint(len(m.snapshot.Workspaces))},
		{m.l10n.Text("tui.label.work-items"), fmt.Sprint(m.workCount())},
		{m.l10n.Text("tui.label.pull-requests"), fmt.Sprint(len(m.snapshot.PullRequests))},
		{m.l10n.Text("tui.label.cleanup"), fmt.Sprint(m.snapshot.PruneCandidates)},
		{m.l10n.Text("tui.label.data-sources"), fmt.Sprint(len(m.snapshot.DataSources))},
		{m.l10n.Text("tui.label.agent"), m.snapshot.DefaultAgent},
	}
	columns := []tableColumn{
		{Header: "tui.column.section", MinWidth: 8, MaxWidth: 16, Priority: 3},
		{Header: "tui.column.subject", MinWidth: 12, MaxWidth: 28, Priority: 0},
		{Header: "tui.column.status", MinWidth: 7, MaxWidth: 14, Priority: 1},
		{Header: "tui.column.operation", MinWidth: 10, MaxWidth: 24, Priority: 2},
		{Header: "tui.column.context", MinWidth: 12, MaxWidth: 40, Priority: 4},
	}
	rows := make([][]string, 0, len(m.snapshot.Cockpit))
	for _, item := range m.snapshot.Cockpit {
		rows = append(rows, []string{item.Section, item.Title, m.semanticStatus(item.Status), item.Primary.Label, item.Subtitle})
	}
	if width < 100 {
		content := m.emptyOrTable(rows, columns, m.selectedCockpit, width-4, height-2, "tui.empty")
		return m.panel(m.l10n.Text("tui.panel.cockpit"), content, width, height)
	}
	leftWidth := max(24, width/3)
	rightWidth := width - leftWidth
	left := m.panel(m.l10n.Text("tui.panel.readiness"), m.renderPairs(metrics, leftWidth-4), leftWidth, height)
	right := m.panel(m.l10n.Text("tui.panel.cockpit"), m.emptyOrTable(rows, columns, m.selectedCockpit, rightWidth-4, height-2, "tui.empty"), rightWidth, height)
	return lipgloss.JoinHorizontal(lipgloss.Top, left, right)
}

func (m *Model) renderWorkspaces(width, height int) string {
	columns := []tableColumn{
		{Header: "tui.column.project", MinWidth: 10, MaxWidth: 20, Priority: 0},
		{Header: "tui.column.work-items", MinWidth: 10, MaxWidth: 24, Priority: 2},
		{Header: "tui.column.type", MinWidth: 8, MaxWidth: 14, Priority: 3},
		{Header: "tui.column.slug", MinWidth: 10, MaxWidth: 24, Priority: 1},
		{Header: "tui.column.repositories", MinWidth: 12, MaxWidth: 32, Priority: 4},
	}
	rows := make([][]string, 0, len(m.snapshot.Workspaces))
	for _, item := range m.snapshot.Workspaces {
		rows = append(rows, []string{item.Project, strings.Join(item.WorkItems, ", "), item.Type, item.Slug, strings.Join(item.Repositories, ", ")})
	}
	listHeight := max(5, height-7)
	content := m.emptyOrTable(rows, columns, m.selectedWorkspace, width-4, listHeight-2, "tui.empty.workspaces")
	list := m.panel(m.l10n.Text(msgWorkspaces), content, width, listHeight)
	detail := m.workspaceDetail(width, height-listHeight)
	return lipgloss.JoinVertical(lipgloss.Left, list, detail)
}

func (m *Model) workspaceDetail(width, height int) string {
	if m.selectedWorkspace >= len(m.snapshot.Workspaces) {
		return m.panel(m.l10n.Text("tui.panel.selection"), m.l10n.Text("tui.empty.workspaces"), width, height)
	}
	item := m.snapshot.Workspaces[m.selectedWorkspace]
	lines := [][]string{{m.l10n.Text("tui.column.project"), item.Project}, {m.l10n.Text("tui.column.work-items"), strings.Join(item.WorkItems, ", ")}, {m.l10n.Text("tui.column.branch"), item.Branch}, {m.l10n.Text("tui.column.repositories"), strings.Join(item.Repositories, ", ")}}
	return m.panel(m.l10n.Text("tui.panel.selection"), m.renderPairs(lines, width-4), width, height)
}

func (m *Model) renderWork(width, height int) string {
	tabs := m.panel(m.l10n.Text(msgWork), m.renderProjectTabs(width-4), width, 3)
	columns := []tableColumn{
		{Header: "tui.column.id", MinWidth: 8, MaxWidth: 18, Priority: 0},
		{Header: "tui.column.type", MinWidth: 8, MaxWidth: 14, Priority: 2},
		{Header: "tui.column.state", MinWidth: 8, MaxWidth: 14, Priority: 1},
		{Header: "tui.column.title", MinWidth: 16, MaxWidth: 64, Priority: 3},
	}
	rows := [][]string{}
	if m.selectedWorkProject < len(m.snapshot.WorkProjects) {
		for _, item := range m.snapshot.WorkProjects[m.selectedWorkProject].Items {
			rows = append(rows, []string{item.ID, item.Type, item.State, item.Title})
		}
	}
	content := m.emptyOrTable(rows, columns, m.selectedWorkItem, width-4, height-5, "tui.empty.work")
	return lipgloss.JoinVertical(lipgloss.Left, tabs, m.panel(m.l10n.Text("tui.status.work"), content, width, height-3))
}

func (m *Model) renderPRs(width, height int) string {
	columns := []tableColumn{
		{Header: "tui.column.project", MinWidth: 9, MaxWidth: 18, Priority: 1},
		{Header: "tui.column.repositories", MinWidth: 10, MaxWidth: 22, Priority: 2},
		{Header: "tui.column.id", MinWidth: 8, MaxWidth: 14, Priority: 0},
		{Header: "tui.column.status", MinWidth: 7, MaxWidth: 12, Priority: 0},
		{Header: "tui.column.work-items", MinWidth: 10, MaxWidth: 22, Priority: 4},
		{Header: "tui.column.workspace", MinWidth: 9, MaxWidth: 12, Priority: 5},
		{Header: "tui.column.branch", MinWidth: 10, MaxWidth: 28, Priority: 3},
		{Header: "tui.column.title", MinWidth: 14, MaxWidth: 48, Priority: 6},
	}
	rows := make([][]string, 0, len(m.snapshot.PullRequests))
	for _, item := range m.snapshot.PullRequests {
		state := m.l10n.Text("tui.status.ready")
		if item.Error != "" {
			state = m.l10n.Text("tui.status.error")
		} else if item.Draft {
			state = m.l10n.Text("tui.status.draft")
		}
		rows = append(rows, []string{item.Project, item.Repository, item.ID, state, strings.Join(item.WorkItems, ","), m.yesNo(item.Workspace != ""), item.Branch, item.Title})
	}
	content := m.emptyOrTable(rows, columns, m.selectedPR, width-4, height-2, "tui.empty.prs")
	return m.panel(m.l10n.Text(msgPRs), content, width, height)
}

func (m *Model) renderData(width, height int) string {
	columns := []tableColumn{
		{Header: "tui.column.scope", MinWidth: 10, MaxWidth: 20, Priority: 2},
		{Header: "tui.column.provider", MinWidth: 10, MaxWidth: 18, Priority: 1},
		{Header: "tui.column.data-source", MinWidth: 12, MaxWidth: 28, Priority: 0},
		{Header: "tui.column.operation", MinWidth: 12, MaxWidth: 32, Priority: 3},
	}
	rows := make([][]string, 0, len(m.snapshot.DataSources))
	for _, item := range m.snapshot.DataSources {
		scope := item.Project
		if scope == "" {
			scope = m.l10n.Text("tui.label.global")
		}
		operation := ""
		if actionItem, ok := findAction(item.Operations, DataCatalogSlot); ok {
			operation = actionItem.Label
		}
		rows = append(rows, []string{scope, item.Provider, item.Key, operation})
	}
	upper := max(6, height*3/5)
	listTitle := m.l10n.Text(msgData)
	operationsTitle := m.l10n.Text("tui.panel.operations")
	if m.dataFocus == 0 {
		listTitle += " · " + m.l10n.Text("tui.focus.active")
	} else {
		operationsTitle += " · " + m.l10n.Text("tui.focus.active")
	}
	listContent := m.emptyOrTable(rows, columns, m.selectedDataSource, width-4, upper-2, "tui.empty.data")
	list := m.panel(listTitle, listContent, width, upper)
	actions := m.renderActionList(operationsTitle, width, height-upper)
	return lipgloss.JoinVertical(lipgloss.Left, list, actions)
}

func (m *Model) renderActionList(title string, width, height int) string {
	items := m.visibleActions()
	columns := []tableColumn{
		{Header: "tui.column.operation", MinWidth: 12, MaxWidth: 28, Priority: 0},
		{Header: "tui.column.risk", MinWidth: 8, MaxWidth: 14, Priority: 1},
		{Header: "tui.column.description", MinWidth: 16, MaxWidth: 64, Priority: 2},
	}
	rows := make([][]string, 0, len(items))
	for _, item := range items {
		rows = append(rows, []string{item.Label, m.riskLabel(item.Risk), item.Description})
	}
	search := m.l10n.Text("tui.filter.empty")
	if m.filter != "" || m.filterActive {
		search = m.message("tui.filter", l10n.A("value", m.filter+map[bool]string{true: "_"}[m.filterActive]))
	}
	content := m.emptyOrTable(rows, columns, m.selectedAction, width-4, height-2, "tui.empty.actions")
	return m.panel(title+" · "+search, content, width, height)
}

func (m *Model) renderModal(modal modalKind, width, height int) string {
	switch modal {
	case menuModal:
		return m.renderMenu(width, height, false)
	case menuSectionModal:
		return m.renderMenu(width, height, true)
	case helpModal:
		return m.renderScrollable(m.l10n.Text(msgHelp), m.helpLines(), 0, m.l10n.Text("tui.keys.modal"), width, height, 78, 78)
	case stateModal:
		return m.renderScrollable(m.l10n.Text(msgState), m.stateLines(), m.stateScroll, m.l10n.Text("tui.keys.modal"), width, height, 82, 72)
	case journalModal:
		return m.renderScrollable(m.l10n.Text(msgJournal), m.journalLines(), m.history.Scroll, m.l10n.Text("tui.keys.journal"), width, height, map[bool]int{true: 100, false: 86}[m.history.Fullscreen], map[bool]int{true: 100, false: 78}[m.history.Fullscreen])
	case detailModal:
		if m.detail != nil {
			return m.renderScrollable(m.detail.title, m.detail.lines, m.detail.scroll, m.l10n.Text("tui.keys.modal"), width, height, 86, 78)
		}
	case progressModal:
		return m.renderScrollable(m.l10n.Text(msgProgress), m.progressLines(), 0, m.l10n.Text("tui.keys.progress"), width, height, 78, 55)
	}
	return ""
}

func (m *Model) renderMenu(width, height int, section bool) string {
	if !section {
		columns := []tableColumn{{Header: "tui.column.section", MinWidth: 20, MaxWidth: 52, Priority: 0}}
		labels := []string{m.l10n.Text("tui.menu.information"), m.l10n.Text("tui.menu.configuration"), m.l10n.Text("tui.menu.default-agent"), m.l10n.Text("tui.menu.terminal-color")}
		rows := make([][]string, 0, len(labels))
		for _, label := range labels {
			rows = append(rows, []string{label})
		}
		content := m.renderTable(columns, rows, m.selectedMenuSection, 54, 12) + "\n" + m.l10n.Text("tui.keys.modal")
		return m.centerPanel(m.l10n.Text(msgMenu), content, width, height, 60, 52)
	}
	columns := []tableColumn{
		{Header: "tui.column.key", MinWidth: 4, MaxWidth: 8, Priority: 0},
		{Header: "tui.column.operation", MinWidth: 14, MaxWidth: 28, Priority: 0},
		{Header: "tui.column.status", MinWidth: 7, MaxWidth: 12, Priority: 1},
		{Header: "tui.column.description", MinWidth: 18, MaxWidth: 48, Priority: 2},
	}
	items := m.menuItems()
	rows := make([][]string, 0, len(items))
	for _, item := range items {
		active := ""
		if item.action != nil && item.action.Active {
			active = m.l10n.Text("tui.status.ready")
		}
		rows = append(rows, []string{item.key, item.label, active, item.description})
	}
	tableWidth := max(40, width*7/10)
	content := m.emptyOrTable(rows, columns, m.selectedMenuItem, tableWidth, max(8, height/2), "tui.empty.actions") + "\n" + m.l10n.Text("tui.keys.modal")
	return m.centerPanel(m.l10n.Text(msgMenu), content, width, height, 76, 65)
}

func (m *Model) renderConfirmation(width, height int) string {
	item := m.confirmation
	title := m.l10n.Text(msgConfirm)
	explanation := m.l10n.Text("tui.confirm.safe")
	if item.Risk == External {
		title = m.l10n.Text(msgExternalConfirm)
		explanation = m.l10n.Text("tui.confirm.external")
	}
	if item.Risk == Destructive {
		title = m.l10n.Text(msgDestructiveConfirm)
		explanation = m.l10n.Text("tui.confirm.destructive")
	}
	body := m.renderPairs([][]string{{m.l10n.Text("tui.label.operation"), item.Label}, {m.l10n.Text("tui.label.effect"), item.Description}, {m.l10n.Text("tui.label.risk"), m.riskLabel(item.Risk)}}, max(40, width*2/3)) + "\n\n" + explanation + "\n\n" + m.l10n.Text("tui.keys.confirm")
	return m.centerPanel(title, body, width, height, 72, 38)
}

func (m *Model) renderInput(width, height int) string {
	prompt := m.prompt
	body := m.renderPairs([][]string{{m.l10n.Text("tui.label.prompt"), prompt.label}, {m.l10n.Text("tui.column.help"), prompt.help}}, max(40, width*2/3)) + "\n\n"
	switch prompt.prompt.PromptKind() {
	case action.PromptConfirm:
		defaultLabel := m.l10n.Text("tui.label.no")
		if typed, ok := prompt.prompt.(action.ConfirmPrompt); ok && typed.Default {
			defaultLabel = m.l10n.Text("tui.label.yes")
		}
		body += m.l10n.Text("tui.label.default") + ": " + defaultLabel
	case action.PromptSelectOne, action.PromptSelectMany:
		body += m.renderPromptChoices(max(1, height*55/100-9))
	case action.PromptText:
		body += m.l10n.Text("tui.column.value") + ": " + renderCursorValue(prompt.value, prompt.cursor, false)
	case action.PromptSecret:
		body += m.l10n.Text("tui.column.value") + ": " + renderCursorValue(prompt.value, prompt.cursor, true)
	}
	body += "\n\n" + m.l10n.Text("tui.keys.input")
	return m.centerPanel(m.l10n.Text(msgActionInput), body, width, height, 74, 55)
}

func (m *Model) renderInit(width, height int) string {
	body := m.l10n.Text("tui.init.body") + "\n\n" + m.message("tui.status.root", l10n.A("root", m.snapshot.Root)) + "\n\n" + m.l10n.Text("tui.init.keys")
	return m.centerPanel(m.l10n.Text("tui.init.title"), body, width, height, 64, 42)
}

func (m *Model) renderForm(form FormState, width, height int, modal bool) string {
	if form.Mode == ChooseTemplate {
		columns := []tableColumn{
			{Header: "tui.column.operation", MinWidth: 16, MaxWidth: 30, Priority: 0},
			{Header: "tui.column.description", MinWidth: 20, MaxWidth: 64, Priority: 1},
		}
		rows := make([][]string, 0, len(formTemplates))
		for _, template := range formTemplates {
			rows = append(rows, []string{m.l10n.Text(template.Label), m.l10n.Text(template.Description)})
		}
		content := m.renderTable(columns, rows, form.TemplateIndex, max(40, width-4), max(6, height-3)) + "\n" + m.l10n.Text("tui.keys.composer.choose")
		if modal {
			return m.centerPanel(m.l10n.Text(msgForms), content, width, height, 82, 82)
		}
		return m.panel(m.l10n.Text(msgForms), content, width, height)
	}
	columns := []tableColumn{
		{Header: "tui.column.field", MinWidth: 14, MaxWidth: 28, Priority: 0},
		{Header: "tui.column.value", MinWidth: 16, MaxWidth: 44, Priority: 0},
		{Header: "tui.column.help", MinWidth: 18, MaxWidth: 64, Priority: 1},
	}
	rows := make([][]string, 0, len(form.Fields))
	for i, field := range form.Fields {
		value := field.Value
		if field.Kind == ToggleField {
			value = m.yesNo(field.enabled())
		} else if i == form.SelectedField {
			value = renderCursorValue(field.Value, field.Cursor, false)
		}
		rows = append(rows, []string{m.l10n.Text(field.Label), value, m.l10n.Text(field.Help)})
	}
	template := form.template()
	actionItem, valid := form.action(m.l10n)
	preview := m.l10n.Text("tui.form.incomplete")
	if valid {
		preview = actionItem.Label + " · " + m.message("tui.form.risk", l10n.A("risk", m.riskLabel(actionItem.Risk)))
	}
	issues := form.validation(m.l10n)
	if len(issues) != 0 {
		preview += "\n" + strings.Join(issues, "\n")
	}
	table := m.renderTable(columns, rows, form.SelectedField, max(40, width-4), max(5, height-7))
	content := table + "\n" + m.panel(m.l10n.Text("tui.panel.preview"), preview, max(20, width-4), min(5, height/3)) + "\n" + m.l10n.Text("tui.keys.composer.edit")
	title := m.l10n.Text(template.Label)
	if modal {
		return m.centerPanel(title, content, width, height, 86, 86)
	}
	return m.panel(title, content, width, height)
}

func (m *Model) renderScrollable(title string, lines []string, scroll int, legend string, width, height, percentWidth, percentHeight int) string {
	_, _, viewportWidth, viewportHeight := scrollDimensions(width, height, percentWidth, percentHeight)
	visual := visualLines(lines, viewportWidth)
	m.viewport.SetWidth(viewportWidth)
	m.viewport.SetHeight(viewportHeight)
	m.viewport.SetContent(strings.Join(visual, "\n"))
	scroll = min(max(0, scroll), max(0, len(visual)-viewportHeight))
	m.viewport.SetYOffset(scroll)
	content := m.viewport.View() + "\n" + lipgloss.NewStyle().Foreground(colors.muted).Render(legend)
	return m.centerPanel(title, content, width, height, percentWidth, percentHeight)
}

func (m *Model) helpLines() []string {
	return []string{m.l10n.Text("tui.help.navigation"), m.l10n.Text("tui.help.nav-lines"), "", m.l10n.Text("tui.help.actions"), m.l10n.Text("tui.help.action-lines"), "", m.l10n.Text("tui.help.accessibility"), m.l10n.Text("tui.help.accessibility-lines")}
}

func (m *Model) stateLines() []string {
	lines := []string{m.l10n.Text("tui.panel.loads")}
	lines = append(lines, m.loaderLine(m.l10n.Text("tui.status.snapshot"), m.snapshotLoad), m.loaderLine(m.l10n.Text("tui.status.work"), m.workLoad), m.loaderLine(m.l10n.Text("tui.status.prs"), m.prLoad))
	if m.active != nil {
		lines = append(lines, "", m.l10n.Text("tui.status.action"), m.active.action.Label)
	}
	lines = append(lines, "", m.l10n.Text("tui.panel.messages"))
	lines = append(lines, m.messages...)
	return lines
}

func (m *Model) loaderLine(label string, state loaderState) string {
	status := m.l10n.Text("tui.status.ready")
	if state.running {
		status = m.l10n.Text("tui.status.loading")
	}
	if state.errorText != "" {
		status = m.l10n.Text("tui.status.error") + ": " + state.errorText
	}
	return label + ": " + status
}

func (m *Model) journalLines() []string {
	run := m.history.selected()
	if run == nil {
		return []string{m.l10n.Text("tui.history.empty")}
	}
	status := m.l10n.Text("tui.status.running")
	switch run.Status {
	case execution.StatusSucceeded:
		status = m.l10n.Text("tui.status.ok")
	case execution.StatusFailed, execution.StatusCanceled, execution.StatusInterrupted:
		status = m.l10n.Text("tui.status.error")
	}
	lines := []string{m.message("tui.history.run", l10n.A("current", m.history.Selected+1), l10n.A("total", len(m.history.Runs)), l10n.A("label", run.Label), l10n.A("status", status)), m.message("tui.history.levels", l10n.A("levels", m.levelLabels())), ""}
	for _, event := range m.history.visibleEvents(run) {
		lines = append(lines, fmt.Sprintf("%s | %s | %s", event.At.Format("2006-01-02 15:04:05Z"), event.Scope, event.Text))
	}
	if !run.Status.Terminal() && len(run.Events) == 0 {
		lines = append(lines, m.l10n.Text("tui.history.waiting"))
	}
	if len(run.Lines) != 0 {
		if len(lines) > 3 {
			lines = append(lines, "")
		}
		lines = append(lines, run.Lines...)
	}
	if run.Error != "" {
		lines = append(lines, run.Error)
	}
	return lines
}

func (m *Model) levelLabels() string {
	labels := []string{"error", "warn", "info", "debug", "other"}
	for i := range labels {
		if m.history.Levels[i] {
			labels[i] = "[x] " + labels[i]
		} else {
			labels[i] = "[ ] " + labels[i]
		}
	}
	return strings.Join(labels, "  ")
}

func (m *Model) progressLines() []string {
	run := m.history.active()
	if run == nil {
		return []string{m.l10n.Text("tui.progress.waiting")}
	}
	lines := []string{m.l10n.Text("tui.label.operation") + ": " + run.Label, "", m.l10n.Text("tui.progress.live")}
	start := max(0, len(run.Events)-12)
	for _, event := range run.Events[start:] {
		lines = append(lines, "• "+event.Text)
	}
	if len(run.Events) == 0 {
		lines = append(lines, m.l10n.Text("tui.progress.waiting"))
	}
	return lines
}

func (m *Model) panel(title, content string, width, height int) string {
	style := lipgloss.NewStyle().Border(lipgloss.RoundedBorder()).BorderForeground(colors.primary).Foreground(colors.text).Width(max(1, width)).Height(max(1, height)).MaxWidth(max(1, width)).MaxHeight(max(1, height)).Padding(0, 1)
	return style.Render(lipgloss.NewStyle().Foreground(colors.accent).Bold(true).Render(title) + "\n" + content)
}

func (m *Model) centerPanel(title, content string, width, height, percentWidth, percentHeight int) string {
	panelWidth, panelHeight := max(30, width*percentWidth/100), max(8, height*percentHeight/100)
	panelWidth, panelHeight = min(width, panelWidth), min(height, panelHeight)
	contentPanel := m.panel(title, content, panelWidth, panelHeight)
	return lipgloss.Place(width, height, lipgloss.Center, lipgloss.Center, contentPanel, lipgloss.WithWhitespaceStyle(lipgloss.NewStyle().Background(colors.surface)))
}

type tableColumn struct {
	Header   l10n.ID
	MinWidth int
	MaxWidth int
	Priority int
}

func (m *Model) renderTable(columns []tableColumn, rows [][]string, selected, width, height int) string {
	if width <= 0 || height <= 0 || len(columns) == 0 || len(rows) == 0 {
		return ""
	}
	selected = clampIndex(selected, len(rows))
	if width < 56 {
		return m.renderCards(columns, rows, selected, width, height)
	}
	headers := make([]string, len(columns))
	widths := make([]int, len(columns))
	visible := make([]bool, len(columns))
	for index, column := range columns {
		headers[index] = m.l10n.Text(column.Header)
		maxWidth := column.MaxWidth
		if maxWidth <= 0 {
			maxWidth = 48
		}
		widths[index] = min(maxWidth, max(column.MinWidth, lipgloss.Width(headers[index])))
		visible[index] = true
	}
	for _, row := range rows {
		for index := range columns {
			if index < len(row) {
				maxWidth := columns[index].MaxWidth
				if maxWidth <= 0 {
					maxWidth = 48
				}
				widths[index] = min(maxWidth, max(widths[index], lipgloss.Width(singleLine(row[index]))))
			}
		}
	}
	totalWidth := func() int {
		total, count := 2, 0
		for index := range columns {
			if visible[index] {
				total += widths[index]
				count++
			}
		}
		return total + max(0, count-1)*2
	}
	for totalWidth() > width {
		candidate := -1
		for index, column := range columns {
			if visible[index] && widths[index] > max(1, column.MinWidth) && (candidate == -1 || column.Priority > columns[candidate].Priority) {
				candidate = index
			}
		}
		if candidate >= 0 {
			widths[candidate]--
			continue
		}
		visibleCount := 0
		for _, shown := range visible {
			if shown {
				visibleCount++
			}
		}
		if visibleCount <= 1 {
			break
		}
		for index, column := range columns {
			if visible[index] && (candidate == -1 || column.Priority > columns[candidate].Priority) {
				candidate = index
			}
		}
		visible[candidate] = false
	}
	line := func(row []string, header bool, marked bool) string {
		cells := make([]string, 0, len(columns))
		for index := range columns {
			if !visible[index] {
				continue
			}
			value := headers[index]
			if !header {
				value = ""
				if index < len(row) {
					value = singleLine(row[index])
				}
			}
			value = truncate(value, widths[index])
			value += strings.Repeat(" ", max(0, widths[index]-lipgloss.Width(value)))
			cells = append(cells, value)
		}
		prefix := "  "
		if marked {
			prefix = "› "
		}
		return truncate(prefix+strings.Join(cells, "  "), width)
	}
	dataHeight := max(1, height-2)
	start := 0
	if selected >= dataHeight {
		start = selected - dataHeight + 1
	}
	end := min(len(rows), start+dataHeight)
	lines := []string{
		lipgloss.NewStyle().Foreground(colors.muted).Bold(true).Render(line(nil, true, false)),
		lipgloss.NewStyle().Foreground(colors.muted).Render(strings.Repeat("─", min(width, totalWidth()))),
	}
	for index := start; index < end; index++ {
		value := line(rows[index], false, index == selected)
		style := lipgloss.NewStyle().Foreground(colors.text)
		if index == selected {
			style = style.Background(colors.selected).Foreground(colors.accent).Bold(true).Width(width)
		}
		lines = append(lines, style.Render(value))
	}
	return strings.Join(lines, "\n")
}

func (m *Model) renderCards(columns []tableColumn, rows [][]string, selected, width, height int) string {
	cards := make([][]string, len(rows))
	for rowIndex, row := range rows {
		for columnIndex, column := range columns {
			if columnIndex >= len(row) || strings.TrimSpace(row[columnIndex]) == "" {
				continue
			}
			prefix := "  "
			if len(cards[rowIndex]) == 0 && rowIndex == selected {
				prefix = "› "
			}
			label := m.l10n.Text(column.Header)
			cards[rowIndex] = append(cards[rowIndex], truncate(prefix+label+": "+singleLine(row[columnIndex]), width))
		}
		if len(cards[rowIndex]) == 0 {
			cards[rowIndex] = []string{"  —"}
		}
		cards[rowIndex] = append(cards[rowIndex], "")
	}
	start := 0
	used := 0
	for index := selected; index >= 0; index-- {
		if used+len(cards[index]) > height {
			start = index + 1
			break
		}
		used += len(cards[index])
		start = index
	}
	var lines []string
	for index := start; index < len(cards) && len(lines) < height; index++ {
		card := cards[index]
		if len(lines)+len(card) > height {
			card = card[:height-len(lines)]
		}
		style := lipgloss.NewStyle().Foreground(colors.text)
		if index == selected {
			style = style.Background(colors.selected).Foreground(colors.accent).Bold(true).Width(width)
		}
		for _, line := range card {
			lines = append(lines, style.Render(line))
		}
	}
	return strings.Join(lines, "\n")
}

func singleLine(value string) string {
	return strings.Join(strings.Fields(value), " ")
}

func (m *Model) emptyOrTable(rows [][]string, columns []tableColumn, selected, width, height int, emptyMessage l10n.ID) string {
	if len(rows) == 0 {
		return m.l10n.Text(emptyMessage)
	}
	return m.renderTable(columns, rows, selected, width, height)
}

func (m *Model) renderProjectTabs(width int) string {
	if len(m.snapshot.WorkProjects) == 0 {
		return m.l10n.Text("tui.empty.work")
	}
	labels := make([]string, 0, len(m.snapshot.WorkProjects))
	for index, project := range m.snapshot.WorkProjects {
		label := project.Key
		if project.Provider != "" {
			label += " · " + project.Provider
		}
		label = fmt.Sprintf("%s (%d)", label, len(project.Items))
		if index != m.selectedWorkProject {
			label = fmt.Sprintf("%d:%s", index+1, project.Key)
		}
		style := lipgloss.NewStyle().Foreground(colors.muted).Padding(0, 1)
		if index == m.selectedWorkProject {
			style = style.Foreground(colors.surface).Background(colors.accent).Bold(true)
		}
		labels = append(labels, style.Render(truncate(label, max(4, width/2))))
	}
	return truncate(strings.Join(labels, " "), width)
}

func (m *Model) renderPromptChoices(height int) string {
	prompt := m.prompt
	if len(prompt.choices) == 0 {
		return m.l10n.Text("tui.empty")
	}
	height = max(1, height)
	start := max(0, prompt.selected-height+1)
	end := min(len(prompt.choices), start+height)
	lines := make([]string, 0, end-start)
	for index := start; index < end; index++ {
		checked := " "
		if prompt.prompt.PromptKind() == action.PromptSelectMany && prompt.selectedMany[index] {
			checked = "x"
		}
		lines = append(lines, fmt.Sprintf("%s [%s] %s", marker(index == prompt.selected), checked, prompt.choices[index]))
	}
	return strings.Join(lines, "\n")
}

func renderCursorValue(value string, cursor int, secret bool) string {
	runes := []rune(value)
	cursor = min(max(0, cursor), len(runes))
	if secret {
		for index := range runes {
			runes[index] = '•'
		}
	}
	return string(runes[:cursor]) + "_" + string(runes[cursor:])
}

func scrollDimensions(width, height, percentWidth, percentHeight int) (panelWidth, panelHeight, viewportWidth, viewportHeight int) {
	panelWidth = min(width, max(30, width*percentWidth/100))
	panelHeight = min(height, max(8, height*percentHeight/100))
	viewportWidth = max(1, panelWidth-4)
	viewportHeight = max(1, panelHeight-6)
	return
}

func visualLines(lines []string, width int) []string {
	visual := make([]string, 0, len(lines))
	for _, line := range lines {
		if line == "" {
			visual = append(visual, "")
			continue
		}
		runes := []rune(line)
		for len(runes) > 0 {
			end := 0
			for end < len(runes) && lipgloss.Width(string(runes[:end+1])) <= width {
				end++
			}
			if end == 0 {
				end = 1
			}
			visual = append(visual, string(runes[:end]))
			runes = runes[end:]
		}
	}
	return visual
}

func maxScrollableOffset(lines []string, width, height, percentWidth, percentHeight int) int {
	_, _, viewportWidth, viewportHeight := scrollDimensions(width, height, percentWidth, percentHeight)
	return max(0, len(visualLines(lines, viewportWidth))-viewportHeight)
}

func (m *Model) maxStateScroll() int {
	return maxScrollableOffset(m.stateLines(), m.width, m.height, 82, 72)
}

func (m *Model) maxDetailScroll() int {
	if m.detail == nil {
		return 0
	}
	return maxScrollableOffset(m.detail.lines, m.width, m.height, 86, 78)
}

func (m *Model) maxJournalScroll() int {
	percentWidth := map[bool]int{true: 100, false: 86}[m.history.Fullscreen]
	percentHeight := map[bool]int{true: 100, false: 78}[m.history.Fullscreen]
	return maxScrollableOffset(m.journalLines(), m.width, m.height, percentWidth, percentHeight)
}

func (m *Model) renderPairs(rows [][]string, width int) string {
	labelWidth := 0
	for _, row := range rows {
		if len(row) >= 2 {
			labelWidth = max(labelWidth, lipgloss.Width(singleLine(row[0])))
		}
	}
	labelWidth = min(labelWidth, max(8, width/2))
	var lines []string
	for _, row := range rows {
		if len(row) < 2 {
			continue
		}
		label := lipgloss.NewStyle().Foreground(colors.muted).Width(labelWidth).MaxHeight(1).Render(truncate(singleLine(row[0]), labelWidth))
		lines = append(lines, label+" "+truncate(singleLine(row[1]), max(1, width-labelWidth-1)))
	}
	return strings.Join(lines, "\n")
}

func (m *Model) viewLabel(view View) string {
	switch view {
	case Dashboard:
		return m.l10n.Text(msgDashboard)
	case Workspaces:
		return m.l10n.Text(msgWorkspaces)
	case Work:
		return m.l10n.Text(msgWork)
	case PullRequests:
		return m.l10n.Text(msgPRs)
	case Data:
		return m.l10n.Text(msgData)
	case Composer:
		return m.l10n.Text(msgComposer)
	}
	return ""
}
func (m *Model) riskLabel(risk Risk) string {
	switch risk {
	case Safe:
		return m.l10n.Text("tui.risk.safe")
	case External:
		return m.l10n.Text("tui.risk.external")
	case Preview:
		return m.l10n.Text("tui.risk.preview")
	case Destructive:
		return m.l10n.Text("tui.risk.destructive")
	}
	return ""
}
func (m *Model) yesNo(value bool) string {
	if value {
		return m.l10n.Text("tui.label.yes")
	}
	return m.l10n.Text("tui.label.no")
}

func (m *Model) semanticStatus(value string) string {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "true":
		return m.yesNo(true)
	case "false":
		return m.yesNo(false)
	default:
		return value
	}
}

func (m *Model) workCount() int {
	count := 0
	for _, project := range m.snapshot.WorkProjects {
		count += len(project.Items)
	}
	return count
}
func marker(selected bool) string {
	if selected {
		return "›"
	}
	return " "
}
func truncate(value string, width int) string {
	if width <= 0 {
		return ""
	}
	if lipgloss.Width(value) <= width {
		return value
	}
	runes := []rune(value)
	for len(runes) > 0 && lipgloss.Width(string(runes))+1 > width {
		runes = runes[:len(runes)-1]
	}
	return string(runes) + "…"
}
func wrapLine(value string, width, maxLines int) string {
	if width <= 0 {
		return ""
	}
	words := strings.Fields(value)
	var lines []string
	current := ""
	for _, word := range words {
		candidate := word
		if current != "" {
			candidate = current + " " + word
		}
		if lipgloss.Width(candidate) > width && current != "" {
			lines = append(lines, current)
			current = word
			if len(lines) == maxLines {
				break
			}
		} else {
			current = candidate
		}
	}
	if current != "" && len(lines) < maxLines {
		lines = append(lines, current)
	}
	return strings.Join(lines, "\n")
}

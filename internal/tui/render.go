package tui

import (
	"fmt"
	"image/color"
	"strings"
	"time"

	tea "charm.land/bubbletea/v2"
	"charm.land/lipgloss/v2"
	"github.com/sachahjkl/dw/internal/action"
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
	view.MouseMode = tea.MouseModeCellMotion
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
	if width < 60 || height < 16 {
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
	if modal, ok := m.topModal(); ok {
		return m.renderModal(modal, width, height)
	}
	if m.confirmation != nil {
		return m.renderConfirmation(width, height)
	}

	header := m.renderHeader(width)
	footer := m.renderFooter(width)
	bodyHeight := max(1, height-lipgloss.Height(header)-lipgloss.Height(footer))
	body := lipgloss.NewStyle().Width(width).Height(bodyHeight).Render(m.renderBody(width, bodyHeight))
	return lipgloss.JoinVertical(lipgloss.Left, header, body, footer)
}

func (m *Model) renderHeader(width int) string {
	var tabs []string
	for i, view := range allViews {
		label := m.viewLabel(view)
		style := lipgloss.NewStyle().Foreground(colors.muted).Padding(0, 1)
		if view == m.view {
			style = style.Foreground(colors.surface).Background(colors.primary).Bold(true)
		}
		tabs = append(tabs, style.Render(fmt.Sprintf("%d %s", i+1, label)))
	}
	left := lipgloss.JoinHorizontal(lipgloss.Top, tabs...)
	status := m.statusSummary()
	right := lipgloss.NewStyle().Foreground(colors.muted).Render(status)
	gap := max(1, width-lipgloss.Width(left)-lipgloss.Width(right))
	bar := left + strings.Repeat(" ", gap) + right
	return lipgloss.NewStyle().Width(width).Background(colors.surface).Render(bar)
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
	if len(m.queue) != 0 {
		parts = append(parts, m.message("tui.status.queue", l10n.A("count", len(m.queue))))
	}
	parts = append(parts, m.snapshot.Root)
	return strings.Join(parts, "  ")
}

func (m *Model) loadingText(label string, state loaderState) string {
	elapsed := time.Since(state.started).Truncate(time.Second)
	return m.spinner.View() + " " + m.message("tui.status.elapsed", l10n.A("label", label), l10n.A("state", m.l10n.Text("tui.status.loading")), l10n.A("elapsed", elapsed))
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
		legend = m.l10n.Text("tui.keys.composer") + "  " + legend
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
	left := m.panel(m.l10n.Text("tui.panel.readiness"), m.renderPairs(metrics, max(20, width/3-4)), max(24, width/3), height)
	rightRows := make([][]string, 0, len(m.snapshot.Cockpit))
	for i, item := range m.snapshot.Cockpit {
		rightRows = append(rightRows, []string{marker(i == m.selectedCockpit), item.Section, item.Title, item.Status, item.Primary.Label, item.Subtitle})
	}
	if len(rightRows) == 0 {
		rightRows = [][]string{{"", m.l10n.Text("tui.empty"), "", "", "", ""}}
	}
	rightWidth := width - max(24, width/3)
	right := m.panel(m.l10n.Text("tui.panel.cockpit"), m.renderRows(rightRows, rightWidth-4, height-2), rightWidth, height)
	if width < 100 {
		return m.panel(m.l10n.Text("tui.panel.cockpit"), m.renderRows(rightRows, width-4, height-2), width, height)
	}
	return lipgloss.JoinHorizontal(lipgloss.Top, left, right)
}

func (m *Model) renderWorkspaces(width, height int) string {
	rows := make([][]string, 0, len(m.snapshot.Workspaces))
	for i, item := range m.snapshot.Workspaces {
		rows = append(rows, []string{marker(i == m.selectedWorkspace), item.Project, strings.Join(item.WorkItems, ", "), item.Type, item.Slug, strings.Join(item.Repositories, ", ")})
	}
	if len(rows) == 0 {
		rows = [][]string{{"", m.l10n.Text("tui.empty.workspaces")}}
	}
	listHeight := max(5, height-7)
	list := m.panel(m.l10n.Text(msgWorkspaces), m.renderRows(rows, width-4, listHeight-2), width, listHeight)
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
	projectTabs := make([]string, 0, len(m.snapshot.WorkProjects))
	for i, project := range m.snapshot.WorkProjects {
		style := lipgloss.NewStyle().Foreground(colors.muted).Padding(0, 1)
		if i == m.selectedWorkProject {
			style = style.Foreground(colors.surface).Background(colors.accent).Bold(true)
		}
		label := project.Key
		if project.Provider != "" {
			label += " · " + project.Provider
		}
		projectTabs = append(projectTabs, style.Render(fmt.Sprintf("%s (%d)", label, len(project.Items))))
	}
	if len(projectTabs) == 0 {
		projectTabs = []string{m.l10n.Text("tui.empty.work")}
	}
	tabs := m.panel(m.l10n.Text(msgWork), strings.Join(projectTabs, " "), width, 3)
	rows := [][]string{}
	if m.selectedWorkProject < len(m.snapshot.WorkProjects) {
		for i, item := range m.snapshot.WorkProjects[m.selectedWorkProject].Items {
			rows = append(rows, []string{marker(i == m.selectedWorkItem), item.ID, item.Type, item.State, item.Title})
		}
	}
	if len(rows) == 0 {
		rows = [][]string{{"", m.l10n.Text("tui.empty.work")}}
	}
	return lipgloss.JoinVertical(lipgloss.Left, tabs, m.panel(m.l10n.Text("tui.status.work"), m.renderRows(rows, width-4, height-5), width, height-3))
}

func (m *Model) renderPRs(width, height int) string {
	rows := make([][]string, 0, len(m.snapshot.PullRequests))
	for i, item := range m.snapshot.PullRequests {
		state := m.l10n.Text("tui.status.ready")
		if item.Error != "" {
			state = m.l10n.Text("tui.status.error")
		} else if item.Draft {
			state = "draft"
		}
		rows = append(rows, []string{marker(i == m.selectedPR), item.Project, item.Repository, item.ID, state, strings.Join(item.WorkItems, ","), present(item.Workspace), item.Branch, item.Title})
	}
	if len(rows) == 0 {
		rows = [][]string{{"", m.l10n.Text("tui.empty.prs")}}
	}
	return m.panel(m.l10n.Text(msgPRs), m.renderRows(rows, width-4, height-2), width, height)
}

func (m *Model) renderData(width, height int) string {
	rows := make([][]string, 0, len(m.snapshot.DataSources))
	for i, item := range m.snapshot.DataSources {
		scope := item.Project
		if scope == "" {
			scope = m.l10n.Text("tui.label.global")
		}
		operation := ""
		if actionItem, ok := findAction(item.Actions, DataCatalogSlot); ok {
			operation = actionItem.Label
		}
		rows = append(rows, []string{marker(i == m.selectedDataSource), scope, item.Provider, item.Key, operation})
	}
	if len(rows) == 0 {
		rows = [][]string{{"", m.l10n.Text("tui.empty.data")}}
	}
	upper := max(6, height*3/5)
	list := m.panel(m.l10n.Text(msgData), m.renderRows(rows, width-4, upper-2), width, upper)
	actions := m.renderActionList(width, height-upper)
	return lipgloss.JoinVertical(lipgloss.Left, list, actions)
}

func (m *Model) renderActionList(width, height int) string {
	items := m.visibleActions()
	rows := make([][]string, 0, len(items))
	for i, item := range items {
		rows = append(rows, []string{marker(i == m.selectedAction), item.Label, m.riskLabel(item.Risk), item.Description})
	}
	if len(rows) == 0 {
		rows = [][]string{{"", m.l10n.Text("tui.empty.actions")}}
	}
	search := m.l10n.Text("tui.filter.empty")
	if m.filter != "" || m.filterActive {
		search = m.message("tui.filter", l10n.A("value", m.filter+map[bool]string{true: "_"}[m.filterActive]))
	}
	return m.panel(m.l10n.Text("tui.panel.operations")+" · "+search, m.renderRows(rows, width-4, height-2), width, height)
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
		labels := []string{m.l10n.Text("tui.menu.information"), m.l10n.Text("tui.menu.configuration"), m.l10n.Text("tui.menu.default-agent"), m.l10n.Text("tui.menu.terminal-color")}
		rows := make([][]string, 0, len(labels))
		for i, label := range labels {
			rows = append(rows, []string{marker(i == m.selectedMenuSection), label})
		}
		return m.centerPanel(m.l10n.Text(msgMenu), m.renderRows(rows, 54, 12)+"\n"+m.l10n.Text("tui.keys.modal"), width, height, 60, 52)
	}
	items := m.menuItems()
	rows := make([][]string, 0, len(items))
	for i, item := range items {
		active := ""
		if item.action != nil && item.action.Active {
			active = m.l10n.Text("tui.status.ready")
		}
		rows = append(rows, []string{marker(i == m.selectedMenuItem), item.key, item.label, active, item.description})
	}
	return m.centerPanel(m.l10n.Text(msgMenu), m.renderRows(rows, max(40, width*7/10), max(8, height/2))+"\n"+m.l10n.Text("tui.keys.modal"), width, height, 76, 65)
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
	switch prompt.prompt.Kind {
	case action.PromptConfirm:
		defaultLabel := m.l10n.Text("tui.label.no")
		if prompt.prompt.Default != nil && string(*prompt.prompt.Default) == "true" {
			defaultLabel = m.l10n.Text("tui.label.yes")
		}
		body += m.l10n.Text("tui.label.default") + ": " + defaultLabel
	case action.PromptSelectOne, action.PromptSelectMany:
		for i, label := range prompt.choices {
			checked := " "
			if prompt.prompt.Kind == action.PromptSelectMany && prompt.selectedMany[i] {
				checked = "x"
			}
			body += fmt.Sprintf("%s [%s] %s\n", marker(i == prompt.selected), checked, label)
		}
	case action.PromptText:
		body += m.l10n.Text("tui.column.value") + ": " + prompt.value + "_"
	case action.PromptSecret:
		body += m.l10n.Text("tui.column.value") + ": " + strings.Repeat("•", len([]rune(prompt.value))) + "_"
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
		rows := make([][]string, 0, len(formTemplates))
		for i, template := range formTemplates {
			rows = append(rows, []string{marker(i == form.TemplateIndex), m.l10n.Text(template.Label), m.l10n.Text(template.Description)})
		}
		content := m.renderRows(rows, max(40, width-4), max(6, height-3)) + "\n" + m.l10n.Text("tui.keys.composer")
		if modal {
			return m.centerPanel(m.l10n.Text(msgForms), content, width, height, 82, 82)
		}
		return m.panel(m.l10n.Text(msgForms), content, width, height)
	}
	rows := make([][]string, 0, len(form.Fields))
	for i, field := range form.Fields {
		value := field.Value
		if field.Kind == ToggleField {
			if field.enabled() {
				value = m.l10n.Text("tui.label.yes")
			} else {
				value = m.l10n.Text("tui.label.no")
			}
		}
		if i == form.SelectedField && field.Kind == TextField {
			value += "_"
		}
		rows = append(rows, []string{marker(i == form.SelectedField), m.l10n.Text(field.Label), value, m.l10n.Text(field.Help)})
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
	content := m.renderRows(rows, max(40, width-4), max(5, height-7)) + "\n" + m.panel(m.l10n.Text("tui.panel.preview"), preview, max(20, width-4), min(5, height/3)) + "\n" + m.l10n.Text("tui.keys.composer")
	title := m.l10n.Text(template.Label)
	if modal {
		return m.centerPanel(title, content, width, height, 86, 86)
	}
	return m.panel(title, content, width, height)
}

func (m *Model) renderScrollable(title string, lines []string, scroll int, legend string, width, height, percentWidth, percentHeight int) string {
	panelWidth := max(30, width*percentWidth/100)
	panelHeight := max(8, height*percentHeight/100)
	innerWidth, innerHeight := max(1, panelWidth-4), max(1, panelHeight-4)
	m.viewport.SetWidth(innerWidth)
	m.viewport.SetHeight(max(1, innerHeight-2))
	m.viewport.SetContent(strings.Join(lines, "\n"))
	maxScroll := max(0, len(lines)-m.viewport.Height())
	if scroll > maxScroll {
		scroll = maxScroll
	}
	m.viewport.SetYOffset(max(0, scroll))
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
	if len(m.queue) != 0 {
		lines = append(lines, "", m.l10n.Text("tui.panel.queue"))
		for _, item := range m.queue {
			lines = append(lines, item.action.Label)
		}
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
	if run.Status == RunSucceeded {
		status = m.l10n.Text("tui.status.ok")
	}
	if run.Status == RunFailed {
		status = m.l10n.Text("tui.status.error")
	}
	lines := []string{m.message("tui.history.run", l10n.A("current", m.history.Selected+1), l10n.A("total", len(m.history.Runs)), l10n.A("label", run.Label), l10n.A("status", status)), m.message("tui.history.levels", l10n.A("levels", m.levelLabels())), ""}
	for _, event := range m.history.visibleEvents(run) {
		lines = append(lines, fmt.Sprintf("%s | %s | %s", event.At.Format("2006-01-02 15:04:05Z"), event.Scope, event.Text))
	}
	if run.Status == RunRunning && len(run.Events) == 0 {
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
	if len(lines) > maxHistoryEvents {
		hidden := len(lines) - maxHistoryEvents
		lines = append([]string{m.message("tui.history.hidden", l10n.A("count", hidden))}, lines[hidden:]...)
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
	style := lipgloss.NewStyle().Border(lipgloss.RoundedBorder()).BorderForeground(colors.primary).Foreground(colors.text).Width(max(1, width-2)).Height(max(1, height-2)).Padding(0, 1)
	return style.Render(lipgloss.NewStyle().Foreground(colors.accent).Bold(true).Render(title) + "\n" + content)
}

func (m *Model) centerPanel(title, content string, width, height, percentWidth, percentHeight int) string {
	panelWidth, panelHeight := max(30, width*percentWidth/100), max(8, height*percentHeight/100)
	panelWidth, panelHeight = min(width, panelWidth), min(height, panelHeight)
	contentPanel := m.panel(title, content, panelWidth, panelHeight)
	return lipgloss.Place(width, height, lipgloss.Center, lipgloss.Center, contentPanel, lipgloss.WithWhitespaceStyle(lipgloss.NewStyle().Background(colors.surface)))
}

func (m *Model) renderRows(rows [][]string, width, height int) string {
	if height <= 0 {
		return ""
	}
	start := 0
	selected := 0
	for i, row := range rows {
		if len(row) != 0 && row[0] == "›" {
			selected = i
			break
		}
	}
	if selected >= height {
		start = selected - height + 1
	}
	end := min(len(rows), start+height)
	var lines []string
	for _, row := range rows[start:end] {
		line := strings.Join(row, "  ")
		line = truncate(line, width)
		style := lipgloss.NewStyle().Foreground(colors.text)
		if len(row) != 0 && row[0] == "›" {
			style = style.Background(colors.selected).Foreground(colors.accent).Bold(true).Width(width)
		}
		lines = append(lines, style.Render(line))
	}
	return strings.Join(lines, "\n")
}

func (m *Model) renderPairs(rows [][]string, width int) string {
	var lines []string
	for _, row := range rows {
		if len(row) < 2 {
			continue
		}
		label := lipgloss.NewStyle().Foreground(colors.muted).Width(min(18, max(10, width/4))).Render(row[0])
		lines = append(lines, label+" "+truncate(row[1], max(1, width-lipgloss.Width(label)-1)))
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
func present(value string) string {
	if value == "" {
		return "—"
	}
	return "✓"
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

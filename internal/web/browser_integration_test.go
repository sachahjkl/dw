package web

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
	"time"

	"github.com/chromedp/cdproto/emulation"
	cdpruntime "github.com/chromedp/cdproto/runtime"
	"github.com/chromedp/chromedp"
	"github.com/sachahjkl/dw/internal/webservice"
)

func TestBrowserLiveModeEndToEnd(t *testing.T) {
	chromium := os.Getenv("DW_CHROMIUM")
	if chromium == "" {
		t.Skip("DW_CHROMIUM is not set; Nix runs this test with Chromium")
	}
	repository, err := filepath.Abs("../..")
	if err != nil {
		t.Fatal(err)
	}
	temporary := t.TempDir()
	binaryName := "dw"
	if runtime.GOOS == "windows" {
		binaryName += ".exe"
	}
	binary := filepath.Join(temporary, binaryName)
	build := exec.Command("go", "build", "-o", binary, "./cmd/dw")
	build.Dir = repository
	build.Env = append(os.Environ(), "CGO_ENABLED=0")
	if output, buildErr := build.CombinedOutput(); buildErr != nil {
		t.Fatalf("build dw: %v\n%s", buildErr, output)
	}

	runtimeDirectory := filepath.Join(temporary, "run")
	if err = os.MkdirAll(runtimeDirectory, 0o700); err != nil {
		t.Fatal(err)
	}
	environment := append(os.Environ(),
		"HOME="+temporary,
		"USERPROFILE="+temporary,
		"APPDATA="+filepath.Join(temporary, "appdata"),
		"LOCALAPPDATA="+filepath.Join(temporary, "localappdata"),
		"XDG_CONFIG_HOME="+filepath.Join(temporary, "config"),
		"XDG_STATE_HOME="+filepath.Join(temporary, "state"),
		"XDG_RUNTIME_DIR="+runtimeDirectory,
	)
	startWebTestService(t, repository, binary, environment, "--port", "0")
	serviceRunning := true
	defer func() {
		if serviceRunning {
			stopWebTestService(t, repository, binary, environment)
		}
	}()

	status := webTestStatus(t, repository, binary, environment)
	location := webTestTicketURL(t, temporary, status.Address)
	startWebTestService(t, repository, binary, environment, "--port", "0")
	idempotent := webTestStatus(t, repository, binary, environment)
	if idempotent.PID != status.PID || idempotent.Address != status.Address {
		t.Fatalf("idempotent start changed service: first=%#v second=%#v", status, idempotent)
	}

	chromeHome := filepath.Join(temporary, "chrome")
	if err = os.MkdirAll(chromeHome, 0o700); err != nil {
		t.Fatal(err)
	}
	var chromeOutput bytes.Buffer
	allocatorOptions := append(chromedp.DefaultExecAllocatorOptions[:],
		chromedp.ExecPath(chromium), chromedp.Headless, chromedp.NoSandbox,
		chromedp.DisableGPU, chromedp.UserDataDir(filepath.Join(chromeHome, "profile")),
		chromedp.Env("HOME="+chromeHome, "XDG_CONFIG_HOME="+filepath.Join(chromeHome, "config"), "XDG_CACHE_HOME="+filepath.Join(chromeHome, "cache")),
		chromedp.Flag("disable-dev-shm-usage", true), chromedp.Flag("disable-crash-reporter", true),
		chromedp.CombinedOutput(&chromeOutput),
	)
	allocatorContext, allocatorCancel := chromedp.NewExecAllocator(context.Background(), allocatorOptions...)
	defer allocatorCancel()
	browserContext, browserCancel := chromedp.NewContext(allocatorContext)
	defer browserCancel()
	browserContext, timeoutCancel := context.WithTimeout(browserContext, 45*time.Second)
	defer timeoutCancel()

	var browserErrors []string
	chromedp.ListenTarget(browserContext, func(event any) {
		if exception, ok := event.(*cdpruntime.EventExceptionThrown); ok {
			browserErrors = append(browserErrors, exception.ExceptionDetails.Error())
		}
	})
	var title string
	if err = chromedp.Run(browserContext,
		chromedp.Navigate(location),
		chromedp.WaitVisible("h1", chromedp.ByQuery),
		chromedp.Title(&title),
		chromedp.Poll(`(() => { const tabs=[...document.querySelectorAll('[role="tablist"] [role="tab"]')]; const panels=[...document.querySelectorAll('[role="tabpanel"]')]; const selected=tabs.filter(tab => tab.getAttribute('aria-selected') === 'true'); const actions=document.querySelector('#tab-actions'); return tabs.length === 6 && panels.length === 7 && selected.length === 1 && selected[0].id === 'tab-overview' && actions?.getAttribute('aria-controls') === 'actions' && tabs.every(tab => document.getElementById(tab.getAttribute('aria-controls'))?.getAttribute('aria-labelledby') === tab.id); })()`, nil),
	); err != nil {
		t.Fatalf("verify tab semantics: %v\n%s", err, chromeOutput.String())
	}
	if err = chromedp.Run(browserContext,
		chromedp.Focus(`#tab-overview`, chromedp.ByQuery),
		chromedp.Evaluate(`document.activeElement.dispatchEvent(new KeyboardEvent('keydown', {key:'ArrowRight', bubbles:true}))`, nil),
		chromedp.Poll(`document.activeElement?.id === 'tab-work' && document.querySelector('#tab-work')?.getAttribute('aria-selected') === 'true'`, nil),
		chromedp.WaitVisible(`#work`, chromedp.ByQuery),
	); err != nil {
		t.Fatalf("use tab keyboard navigation: %v\n%s", err, chromeOutput.String())
	}
	var resourceDriven bool
	if err = chromedp.Run(browserContext,
		chromedp.Evaluate(`document.querySelector('#tab-commands, #commands, [data-command-key], [data-command-target]') === null`, &resourceDriven),
		chromedp.Click(`#tab-overview`, chromedp.ByQuery),
		chromedp.WaitVisible(`#overview`, chromedp.ByQuery),
		chromedp.Click(`#overview form[data-operation-relation="doctor"] button[type="submit"]`, chromedp.ByQuery),
		chromedp.Poll(`(() => { const form=document.querySelector('#overview form[data-operation-relation="doctor"]'); const button=form?.querySelector('.operation-button-text'); return !!form?.dataset.operationState && button?.textContent !== 'Doctor'; })()`, nil),
	); err != nil {
		t.Fatalf("submit contextual doctor operation: %v\nbrowser events: %v\n%s", err, browserErrors, chromeOutput.String())
	}
	var closeButtonFits bool
	if err = chromedp.Run(browserContext,
		chromedp.Poll(`document.querySelector('#actions article[data-relation="doctor"]:is([data-status="succeeded"],[data-status="failed"],[data-status="canceled"],[data-status="interrupted"])') !== null`, nil, chromedp.WithPollingMutation()),
		chromedp.Poll(`document.querySelector('#action-results dialog.result-dialog[open]') !== null`, nil, chromedp.WithPollingMutation()),
		chromedp.Evaluate(`(() => { const dialog=document.querySelector('#action-results dialog.result-dialog[open]'); const form=dialog?.querySelector('.dialog-close'); const button=form?.querySelector('button'); if (!dialog || !form || !button) return false; const d=dialog.getBoundingClientRect(); const b=button.getBoundingClientRect(); return button.scrollWidth <= button.clientWidth && b.left >= d.left && b.right <= d.right; })()`, &closeButtonFits),
		chromedp.Evaluate(`document.querySelector('#action-results dialog.result-dialog[open]').close()`, nil),
		chromedp.Click(`#tab-actions`, chromedp.ByQuery),
		chromedp.WaitVisible(`#actions article[data-relation="doctor"]`, chromedp.ByQuery),
		chromedp.Evaluate(`document.querySelector('#actions article[data-relation="doctor"] .activity').open = true`, nil),
		chromedp.Poll(`(() => { const times=[...document.querySelectorAll('#actions time')]; return times.length > 0 && times.every(item => item.textContent !== item.dateTime); })()`, nil),
	); err != nil {
		t.Fatalf("run contextual doctor operation: %v\nbrowser events: %v\n%s", err, browserErrors, chromeOutput.String())
	}
	if !closeButtonFits {
		t.Fatal("result dialog close button overflows its container")
	}
	if !resourceDriven {
		t.Fatal("web UI still exposes command catalogue elements")
	}
	if title != "DevWorkflow" {
		t.Fatalf("title = %q", title)
	}
	var executionText string
	if err = chromedp.Run(browserContext, chromedp.Text("#actions", &executionText, chromedp.ByQuery)); err != nil {
		t.Fatal(err)
	}
	for _, marker := range []string{"Doctor", "Completed", "Action queued.", "Action started.", "Action completed."} {
		if !strings.Contains(executionText, marker) {
			t.Errorf("execution view does not contain %q: %s", marker, executionText)
		}
	}
	var compactMobileChecks []bool
	if err = chromedp.Run(browserContext,
		emulation.SetDeviceMetricsOverride(390, 844, 1, false),
		chromedp.Evaluate(`(() => { document.documentElement.style.scrollBehavior = 'auto'; document.querySelector('#tab-overview').click(); window.scrollTo(0, 300); })()`, nil),
		chromedp.Poll(`Math.abs(document.querySelector('.navbar').getBoundingClientRect().top) <= 1`, nil),
		chromedp.Evaluate(`(() => {
			const header = document.querySelector('.app-header');
			const nav = document.querySelector('.navbar');
			const tabs = [...document.querySelectorAll('.nav-tabs .nav-tab')];
			const stack = document.createElement('aside');
			stack.className = 'toast-stack';
			stack.innerHTML = '<button class="toast"><strong>Input required</strong><span>Confirm operation</span><small>Open</small></button>';
			nav.after(stack);
			const navRect = nav.getBoundingClientRect();
			return [
				header.getBoundingClientRect().height <= 52,
				header.querySelector('.eyebrow, .root-path') === null,
				header.querySelector('#tab-actions') !== null,
				getComputedStyle(header.querySelector('h1')).whiteSpace === 'nowrap',
				getComputedStyle(nav).position === 'sticky',
				Math.abs(navRect.top) <= 1,
				Math.abs(navRect.left) <= 1,
				Math.abs(navRect.right - innerWidth) <= 1,
				new Set(tabs.map(tab => Math.round(tab.getBoundingClientRect().top))).size === 1,
				getComputedStyle(stack).position === 'static',
			];
		})()`, &compactMobileChecks),
	); err != nil {
		t.Fatal(err)
	}
	for index, passed := range compactMobileChecks {
		if !passed {
			t.Fatalf("mobile layout check %d failed: %v", index, compactMobileChecks)
		}
	}
	var noHorizontalOverflow bool
	if err = chromedp.Run(browserContext,
		emulation.SetDeviceMetricsOverride(390, 844, 1, false),
		chromedp.Evaluate(`document.documentElement.scrollWidth <= document.documentElement.clientWidth`, &noHorizontalOverflow),
	); err != nil {
		t.Fatal(err)
	}
	if !noHorizontalOverflow {
		t.Fatal("mobile layout has horizontal overflow")
	}
	if len(browserErrors) != 0 {
		t.Fatalf("browser JavaScript errors: %v", browserErrors)
	}

	stopWebTestService(t, repository, binary, environment)
	serviceRunning = false
	statePath := filepath.Join(runtimeDirectory, "devworkflow", "web", "state.json")
	if runtime.GOOS == "windows" {
		statePath = filepath.Join(temporary, "localappdata", "DevWorkflow", "web", "state.json")
	}
	if _, statErr := os.Stat(statePath); !os.IsNotExist(statErr) {
		t.Fatalf("runtime state remains after stop: %v", statErr)
	}
	startWebTestService(t, repository, binary, environment)
	serviceRunning = true
	restarted := webTestStatus(t, repository, binary, environment)
	restartedLocation := webTestTicketURL(t, temporary, restarted.Address)
	if err = chromedp.Run(browserContext,
		emulation.SetDeviceMetricsOverride(1280, 800, 1, false),
		chromedp.Navigate(restartedLocation),
		chromedp.Poll(`document.querySelector('#actions .status.succeeded') !== null`, nil, chromedp.WithPollingMutation()),
		chromedp.Click(`#tab-actions`, chromedp.ByQuery),
		chromedp.WaitVisible(`#actions .status.succeeded`, chromedp.ByQuery),
	); err != nil {
		t.Fatal(err)
	}
}

func startWebTestService(t *testing.T, directory, binary string, environment []string, arguments ...string) {
	t.Helper()
	commandArguments := append([]string{"web", "start"}, arguments...)
	command := exec.Command(binary, commandArguments...)

	command.Dir = directory
	command.Env = environment
	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("start web service: %v\n%s", err, output)
	}
}

func stopWebTestService(t *testing.T, directory, binary string, environment []string) {
	t.Helper()
	command := exec.Command(binary, "web", "stop")
	command.Dir = directory
	command.Env = environment
	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("stop web service: %v\n%s", err, output)
	}
}

func webTestStatus(t *testing.T, directory, binary string, environment []string) webservice.StatusV1 {
	t.Helper()
	command := exec.Command(binary, "web", "status", "--json")
	command.Dir = directory
	command.Env = environment
	output, err := command.Output()
	if err != nil {
		t.Fatal(err)
	}
	var status webservice.StatusV1
	if err = json.Unmarshal(output, &status); err != nil {
		t.Fatalf("decode status: %v\n%s", err, output)
	}
	if !status.Running || status.Address == "" {
		t.Fatalf("service is not running: %#v", status)
	}
	return status
}

func webTestTicketURL(t *testing.T, temporary, address string) string {
	t.Helper()
	configPath := filepath.Join(temporary, "config", "DevWorkflow", "web.json")
	if runtime.GOOS == "windows" {
		configPath = filepath.Join(temporary, "localappdata", "DevWorkflow", "web.json")
	}
	configContent, err := os.ReadFile(configPath)
	if err != nil {
		t.Fatal(err)
	}
	var configValue webservice.WebConfigV1
	if err = json.Unmarshal(configContent, &configValue); err != nil {
		t.Fatal(err)
	}
	request, err := http.NewRequest(http.MethodPost, "http://"+address+"/admin/tickets", bytes.NewReader([]byte(`{"schema":1,"noExpiry":false}`)))
	if err != nil {
		t.Fatal(err)
	}
	request.Header.Set("Authorization", "Bearer "+configValue.ServiceSecret.String())
	response, err := http.DefaultClient.Do(request)
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusCreated {
		t.Fatalf("ticket status = %d", response.StatusCode)
	}
	var ticket TicketV1
	if err = json.NewDecoder(response.Body).Decode(&ticket); err != nil {
		t.Fatal(err)
	}
	return fmt.Sprintf("http://%s/?ticket=%s", address, ticket.Ticket)
}

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
	if runtime.GOOS != "linux" {
		t.Skip("the Nix Chromium integration runs on Linux")
	}

	repository, err := filepath.Abs("../..")
	if err != nil {
		t.Fatal(err)
	}
	temporary := t.TempDir()
	binary := filepath.Join(temporary, "dw")
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
		chromedp.Evaluate(`(() => { const node = [...document.querySelectorAll('details')].find(item => item.querySelector('code')?.textContent === 'doctor'); if (!node) throw new Error('doctor command missing'); node.open = true; })()`, nil),
		chromedp.Click(`details[open] button[type="submit"]`, chromedp.ByQuery),
		chromedp.WaitVisible(`#executions .status.succeeded`, chromedp.ByQuery),
		chromedp.Poll(`document.querySelector('#executions')?.innerText.includes('#3 succeeded')`, nil, chromedp.WithPollingMutation()),
	); err != nil {
		t.Fatalf("%v\n%s", err, chromeOutput.String())
	}
	if title != "DevWorkflow" {
		t.Fatalf("title = %q", title)
	}
	var executionText string
	if err = chromedp.Run(browserContext, chromedp.Text("#executions", &executionText, chromedp.ByQuery)); err != nil {
		t.Fatal(err)
	}
	for _, marker := range []string{"doctor", "succeeded", "#1 queued", "#2 started", "#3 succeeded"} {
		if !strings.Contains(executionText, marker) {
			t.Errorf("execution view does not contain %q: %s", marker, executionText)
		}
	}
	if err = chromedp.Run(browserContext,
		chromedp.Evaluate(`(() => { const node = [...document.querySelectorAll('details')].find(item => item.querySelector('code')?.textContent === 'secret.delete'); if (!node) throw new Error('secret delete command missing'); node.open = true; const input = node.querySelector('input[type="text"]'); input.value = 'dw-browser-confirm'; input.dispatchEvent(new Event('input', {bubbles:true})); node.querySelector('button[type="submit"]').click(); })()`, nil),
		chromedp.Poll(`[...document.querySelectorAll('#executions article.execution')].some(item => item.querySelector('h3')?.textContent === 'secret.delete' && item.querySelector('.status')?.textContent === 'waiting-input' && item.querySelector('.prompt input[type="checkbox"]'))`, nil, chromedp.WithPollingMutation()),
		chromedp.Evaluate(`(() => { const item = [...document.querySelectorAll('#executions article.execution')].find(node => node.querySelector('h3')?.textContent === 'secret.delete' && node.querySelector('.status')?.textContent === 'waiting-input'); item.querySelector('.prompt input[type="checkbox"]').click(); item.querySelector('.prompt button[type="submit"]').click(); })()`, nil),
		chromedp.Poll(`[...document.querySelectorAll('#executions article.execution')].some(item => item.querySelector('h3')?.textContent === 'secret.delete' && ['failed','succeeded'].includes(item.querySelector('.status')?.textContent) && !item.querySelector('.prompt'))`, nil, chromedp.WithPollingMutation()),
		chromedp.Evaluate(`(() => { const node = [...document.querySelectorAll('details')].find(item => item.querySelector('code')?.textContent === 'secret.delete'); const input = node.querySelector('input[type="text"]'); input.value = 'dw-browser-cancel'; input.dispatchEvent(new Event('input', {bubbles:true})); node.querySelector('button[type="submit"]').click(); })()`, nil),
		chromedp.Poll(`[...document.querySelectorAll('#executions article.execution')].some(item => item.querySelector('h3')?.textContent === 'secret.delete' && item.querySelector('.status')?.textContent === 'waiting-input')`, nil, chromedp.WithPollingMutation()),
		chromedp.Evaluate(`(() => { const item = [...document.querySelectorAll('#executions article.execution')].find(node => node.querySelector('h3')?.textContent === 'secret.delete' && node.querySelector('.status')?.textContent === 'waiting-input'); item.querySelector('button[aria-label^="Cancel execution"]').click(); })()`, nil),
		chromedp.Poll(`[...document.querySelectorAll('#executions article.execution')].some(item => item.querySelector('h3')?.textContent === 'secret.delete' && item.querySelector('.status')?.textContent === 'canceled')`, nil, chromedp.WithPollingMutation()),
	); err != nil {
		t.Fatalf("prompt and cancellation flow: %v\n%s", err, chromeOutput.String())
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
		chromedp.WaitVisible(`#executions .status.succeeded`, chromedp.ByQuery),
	); err != nil {
		t.Fatal(err)
	}
}

func startWebTestService(t *testing.T, directory, binary string, environment []string, arguments ...string) {
	t.Helper()
	commandArguments := append([]string{"web", "start", "--no-open"}, arguments...)
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
	configContent, err := os.ReadFile(filepath.Join(temporary, "config", "DevWorkflow", "web.json"))
	if err != nil {
		t.Fatal(err)
	}
	var configValue webservice.WebConfigV1
	if err = json.Unmarshal(configContent, &configValue); err != nil {
		t.Fatal(err)
	}
	request, err := http.NewRequest(http.MethodPost, "http://"+address+"/admin/tickets", bytes.NewReader(nil))
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

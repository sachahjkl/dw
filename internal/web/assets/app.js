globalThis.dwDateTimeFormatter = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
  timeStyle: "short",
});
globalThis.dwTimeFormatter = new Intl.DateTimeFormat(undefined, {
  timeStyle: "short",
});

document.addEventListener("keydown", (event) => {
  if (!(event.target instanceof Element)) return;
  const current = event.target.closest('[role="tab"]');
  const tablist = current?.closest('[role="tablist"]');
  if (!(current instanceof HTMLButtonElement) || tablist === null) return;

  const tabs = [...tablist.querySelectorAll('[role="tab"]')];
  const index = tabs.indexOf(current);
  let next = index;
  if (event.key === "ArrowRight") next = (index + 1) % tabs.length;
  else if (event.key === "ArrowLeft") next = (index - 1 + tabs.length) % tabs.length;
  else if (event.key === "Home") next = 0;
  else if (event.key === "End") next = tabs.length - 1;
  else return;

  event.preventDefault();
  tabs[next].focus();
  tabs[next].click();
});

const pendingOperations = new Map();
const observedActiveOperations = new Set();
const settlingOperations = new Set();
let autoOpenOperation = "";

function operationForms(key) {
  return [...document.querySelectorAll("form.operation")].filter((form) => form.dataset.operationKey === key);
}

function operationExecution(key) {
  return [...document.querySelectorAll("#actions .execution")].find((item) => item.dataset.operationKey === key);
}

function setOperationState(key, state, label) {
  for (const form of operationForms(key)) {
    const button = form.querySelector(".operation-button");
    const text = button?.querySelector(".operation-button-text");
    if (!(button instanceof HTMLButtonElement) || !(text instanceof HTMLElement)) continue;
    form.dataset.operationState = state;
    button.disabled = button.dataset.operationDisabled === "true" || ["starting", "queued", "running", "waiting-input", "canceling"].includes(state);
    if (text.textContent !== label) text.textContent = label;
  }
}

function resetOperation(key) {
  for (const form of operationForms(key)) {
    const button = form.querySelector(".operation-button");
    const text = button?.querySelector(".operation-button-text");
    if (!(button instanceof HTMLButtonElement) || !(text instanceof HTMLElement)) continue;
    delete form.dataset.operationState;
    button.disabled = button.dataset.operationDisabled === "true";
    const label = button.dataset.operationLabel || text.textContent;
    if (text.textContent !== label) text.textContent = label;
  }
}

function openCompletedResult(key) {
  autoOpenOperation = key;
  const dialog = [...document.querySelectorAll("dialog.result-dialog")].find((item) => item.dataset.operationKey === key);
  if (!(dialog instanceof HTMLDialogElement) || dialog.open) return;
  dialog.showModal();
}

function syncOperations() {
  const activeNow = new Set();
  for (const execution of document.querySelectorAll("#actions .execution")) {
    const key = execution.dataset.operationKey;
    const status = execution.dataset.status || "";
    if (!key || !["queued", "running", "waiting-input", "canceling"].includes(status)) continue;
    activeNow.add(key);
    const progress = execution.dataset.progress || execution.dataset.statusLabel || "Running";
    setOperationState(key, status, progress);
  }
  for (const key of observedActiveOperations) {
    if (activeNow.has(key) || pendingOperations.has(key) || settlingOperations.has(key)) continue;
    const execution = operationExecution(key);
    if (execution && ["succeeded", "failed", "canceled", "interrupted"].includes(execution.dataset.status || "")) {
      openCompletedResult(key);
    }
    resetOperation(key);
  }
  observedActiveOperations.clear();
  for (const key of activeNow) observedActiveOperations.add(key);

  for (const [key, pending] of pendingOperations) {
    const execution = operationExecution(key);
    if (!(execution instanceof HTMLElement)) {
      setOperationState(key, "starting", "Starting…");
      continue;
    }
    const executionID = execution.dataset.executionId || "";
    if (executionID === pending.previousExecution && !pending.started) continue;
    pending.started = true;
    clearTimeout(pending.timeout);
    const status = execution.dataset.status || "running";
    const progress = execution.dataset.progress || execution.dataset.statusLabel || "Running";
    const terminal = ["succeeded", "failed", "canceled", "interrupted"].includes(status);
    setOperationState(key, status, terminal ? (execution.dataset.statusLabel || progress) : progress);
    if (!terminal) continue;
    openCompletedResult(key);
    pendingOperations.delete(key);
    settlingOperations.add(key);
    setTimeout(() => {
      settlingOperations.delete(key);
      resetOperation(key);
    }, 1800);
  }
  for (const next of document.querySelectorAll("[data-next-operation]")) {
    if (!(next instanceof HTMLButtonElement)) continue;
    const form = [...document.querySelectorAll("form.operation")].find((item) =>
      item.dataset.operationRelation === next.dataset.nextOperation && item.dataset.operationSubject === next.dataset.operationSubject
    );
    next.disabled = !(form instanceof HTMLFormElement);
    const label = form?.querySelector(".operation-button")?.dataset.operationLabel;
    if (label && next.textContent !== label) next.textContent = label;
  }
  if (autoOpenOperation) openCompletedResult(autoOpenOperation);
}

document.addEventListener("submit", (event) => {
  if (!(event.target instanceof HTMLFormElement) || !event.target.matches("form.operation")) return;
  const key = event.target.dataset.operationKey;
  if (!key) return;
  clearTimeout(pendingOperations.get(key)?.timeout);
  const current = operationExecution(key);
  const timeout = setTimeout(() => globalThis.dwOperationRejected(key), 15000);
  pendingOperations.set(key, {previousExecution: current?.dataset.executionId || "", started: false, timeout});
  setOperationState(key, "starting", "Starting…");
});

globalThis.dwOperationRejected = (key) => {
  clearTimeout(pendingOperations.get(key)?.timeout);
  pendingOperations.delete(key);
  settlingOperations.add(key);
  setOperationState(key, "failed", "Could not start");
  setTimeout(() => {
    settlingOperations.delete(key);
    resetOperation(key);
  }, 1800);
};

document.addEventListener("click", (event) => {
  if (!(event.target instanceof Element)) return;
  const next = event.target.closest("[data-next-operation]");
  if (!(next instanceof HTMLButtonElement)) return;
  const relation = next.dataset.nextOperation;
  const subject = next.dataset.operationSubject;
  const form = [...document.querySelectorAll("form.operation")].find((item) =>
    item.dataset.operationRelation === relation && item.dataset.operationSubject === subject
  );
  if (!(form instanceof HTMLFormElement)) return;
  next.closest("dialog")?.close();
  form.requestSubmit();
});

document.addEventListener("close", (event) => {
  if (event.target instanceof HTMLDialogElement && event.target.dataset.operationKey === autoOpenOperation) {
    autoOpenOperation = "";
  }
}, true);

new MutationObserver(syncOperations).observe(document.body, {childList: true, subtree: true});
syncOperations();

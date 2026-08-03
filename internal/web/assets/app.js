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
const pendingResults = new Set();
const dismissedInteractions = new Set();
let activeDialogID = "";
const promptSubmissionTimers = new WeakMap();
const toastExpiryTimers = new WeakMap();

function actionResponseTimeout() {
  const value = Number(document.body.dataset.actionResponseTimeout);
  return Number.isFinite(value) && value > 0 ? value : 15000;
}

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

function openOperationDialog(kind, executionID) {
  if (kind === "result") pendingResults.add(executionID);
  openNextDialog();
}

function showActionDialog(dialog) {
  activeDialogID = dialog.id;
  dialog.showModal();
}

function openNextDialog() {
  const dialogs = [...document.querySelectorAll("dialog.interaction-dialog")];
  const liveTokens = new Set(dialogs.map((dialog) => dialog.dataset.dialogToken || ""));
  for (const token of dismissedInteractions) {
    if (!liveTokens.has(token)) dismissedInteractions.delete(token);
  }
  const openDialog = document.querySelector("dialog.action-dialog[open]");
  if (openDialog instanceof HTMLDialogElement && openDialog.matches(":modal")) return;
  if (openDialog instanceof HTMLDialogElement) openDialog.open = false;
  if (activeDialogID) {
    const activeDialog = document.getElementById(activeDialogID);
    if (activeDialog instanceof HTMLDialogElement) {
      showActionDialog(activeDialog);
      queueMicrotask(() => activeDialog.querySelector('.prompt input:not([type="hidden"]), .prompt select, .auth-action')?.focus());
      return;
    }
    activeDialogID = "";
  }
  for (const execution of document.querySelectorAll("#actions .execution[data-interaction-label]")) {
    if (!execution.dataset.interactionLabel) continue;
    const executionID = execution.dataset.executionId || "";
    const dialog = dialogs.find((item) => item.dataset.executionId === executionID);
    if (!dialog || dismissedInteractions.has(dialog.dataset.dialogToken || "")) continue;
    showActionDialog(dialog);
    queueMicrotask(() => dialog.querySelector('.prompt input:not([type="hidden"]), .prompt select, .auth-action')?.focus());
    return;
  }
  for (const executionID of pendingResults) {
    const dialog = [...document.querySelectorAll("dialog.result-dialog")].find((item) => item.dataset.executionId === executionID);
    if (!(dialog instanceof HTMLDialogElement)) {
      const execution = [...document.querySelectorAll("#actions .execution")].find((item) => item.dataset.executionId === executionID);
      if (!execution) pendingResults.delete(executionID);
      continue;
    }
    pendingResults.delete(executionID);
    showActionDialog(dialog);
    return;
  }
}

globalThis.dwOpenActionDialog = (kind, executionID) => {
  if (kind === "interaction") {
    const dialog = [...document.querySelectorAll("dialog.interaction-dialog")].find((item) => item.dataset.executionId === executionID);
    if (dialog instanceof HTMLDialogElement) dismissedInteractions.delete(dialog.dataset.dialogToken || "");
  }
  openOperationDialog(kind, executionID);
};

function syncOperations() {
  const activeNow = new Set();
  for (const execution of document.querySelectorAll("#actions .execution")) {
    const key = execution.dataset.operationKey;
    const status = execution.dataset.status || "";
    if (!key || !["queued", "running", "waiting-input", "canceling"].includes(status)) continue;
    if (activeNow.has(key)) continue;
    activeNow.add(key);
    const interaction = execution.dataset.interactionLabel || "";
    const progress = interaction || execution.dataset.progress || execution.dataset.statusLabel || "Running";
    setOperationState(key, status, progress);
  }
  for (const key of observedActiveOperations) {
    if (activeNow.has(key) || pendingOperations.has(key) || settlingOperations.has(key)) continue;
    const execution = operationExecution(key);
    if (execution && ["succeeded", "failed", "canceled", "interrupted"].includes(execution.dataset.status || "")) {
      openOperationDialog("result", execution.dataset.executionId || "");
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
    const final = ["succeeded", "failed", "canceled", "interrupted"].includes(status);
    setOperationState(key, status, final ? (execution.dataset.statusLabel || progress) : progress);
    if (!final) continue;
    openOperationDialog("result", executionID);
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
  scheduleToastExpiry();
  openNextDialog();
}

document.addEventListener("submit", (event) => {
  if (!(event.target instanceof HTMLFormElement)) return;
  if (event.target.matches("form.prompt")) {
    const button = event.target.querySelector('button[type="submit"]');
    if (button instanceof HTMLButtonElement) {
      button.disabled = true;
      button.textContent = "Submitting…";
      const feedback = event.target.querySelector(".prompt-feedback");
      if (feedback instanceof HTMLElement) feedback.textContent = "Submitting response…";
      clearTimeout(promptSubmissionTimers.get(event.target));
      promptSubmissionTimers.set(event.target, setTimeout(() => globalThis.dwPromptRejected(event.target), actionResponseTimeout()));
    }
    return;
  }
  if (!event.target.matches("form.operation")) return;
  const key = event.target.dataset.operationKey;
  if (!key) return;
  clearTimeout(pendingOperations.get(key)?.timeout);
  const current = operationExecution(key);
  const timeout = setTimeout(() => globalThis.dwOperationRejected(key), actionResponseTimeout());
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

globalThis.dwPromptAccepted = (form) => {
  clearTimeout(promptSubmissionTimers.get(form));
  promptSubmissionTimers.delete(form);
  const feedback = form?.querySelector(".prompt-feedback");
  if (feedback instanceof HTMLElement) feedback.textContent = "Response accepted.";
};

globalThis.dwPromptRejected = (form) => {
  clearTimeout(promptSubmissionTimers.get(form));
  promptSubmissionTimers.delete(form);
  const button = form?.querySelector('button[type="submit"]');
  if (button instanceof HTMLButtonElement) {
    button.disabled = false;
    button.textContent = "Submit response";
  }
  const feedback = form?.querySelector(".prompt-feedback");
  if (feedback instanceof HTMLElement) feedback.textContent = "Response was not accepted. Try again.";
};

function scheduleToastExpiry() {
  for (const toast of document.querySelectorAll(".toast[data-expires-at]")) {
    if (!(toast instanceof HTMLElement) || toastExpiryTimers.has(toast)) continue;
    const expiresAt = Number(toast.dataset.expiresAt);
    if (!Number.isFinite(expiresAt) || expiresAt <= 0) continue;
    const timeout = setTimeout(() => toast.remove(), Math.max(0, expiresAt - Date.now()));
    toastExpiryTimers.set(toast, timeout);
  }
}

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
  if (event.target instanceof HTMLDialogElement && event.target.classList.contains("interaction-dialog")) {
    dismissedInteractions.add(event.target.dataset.dialogToken || "");
  }
  if (event.target instanceof HTMLDialogElement && event.target.id === activeDialogID) activeDialogID = "";
  queueMicrotask(syncOperations);
}, true);

new MutationObserver(syncOperations).observe(document.body, {childList: true, subtree: true});
syncOperations();

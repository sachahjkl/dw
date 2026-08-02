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


let actionFeedbackTimer;
document.addEventListener("submit", (event) => {
  if (!(event.target instanceof HTMLFormElement)) return;
  if (event.target.closest(".operation") === null) return;

  const feedback = document.querySelector("#action-feedback");
  if (!(feedback instanceof HTMLElement)) return;

  feedback.hidden = false;
  clearTimeout(actionFeedbackTimer);
  actionFeedbackTimer = setTimeout(() => {
    feedback.hidden = true;
  }, 2500);
});

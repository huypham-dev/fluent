const api = window.textActions;
const $ = (id) => document.getElementById(id);
const result = $('result');
const input = $('input');

let view = null;
let lastPhase = null;

const isGenerating = () => view?.state?.phase === 'generating';
const canCopy = () => view?.kind === 'session' && !isGenerating() && result.value.trim() !== '';

function copy() {
  if (canCopy()) api.copy(result.value);
}

function renderSession() {
  const { state, title, editable, inputLabel } = view;
  const generating = state.phase === 'generating';
  $('title').textContent = title;
  input.placeholder = inputLabel;
  result.readOnly = generating || !editable;

  // Edits live in the text area, so only generation updates may replace its content.
  if (generating || lastPhase !== state.phase) {
    result.value = state.result;
    if (generating) result.scrollTop = result.scrollHeight;
  }

  $('error-row').hidden = state.phase !== 'failed';
  $('error').textContent = state.error?.message ?? '';
  const needsSettings = state.error?.kind === 'notFound' || state.error?.kind === 'unsupported';
  $('retry').hidden = needsSettings;
  $('error-settings').hidden = !needsSettings;

  $('copy').disabled = !canCopy();
  $('hint').textContent = generating
    ? 'Writing…'
    : editable
      ? '⏎ Copy · ⇧⏎ New line · esc Close'
      : 'esc Close';

  if (lastPhase === 'generating' && state.phase === 'ready') {
    (editable ? result : input).focus();
  }
  lastPhase = state.phase;
}

function renderMessage() {
  $('message-title').textContent = view.title;
  $('message-detail').textContent = view.detail;
  const button = $('message-button');
  button.hidden = !view.button;
  button.textContent = view.button === 'accessibility' ? 'Open System Settings' : 'Open Settings';
}

api.onView((next) => {
  view = next;
  const isSession = view?.kind === 'session';
  $('session').hidden = !isSession;
  $('message').hidden = view?.kind !== 'message';
  if (isSession) renderSession();
  else if (view) renderMessage();
});

result.addEventListener('input', () => {
  api.edit(result.value);
  $('copy').disabled = !canCopy();
});

result.addEventListener('keydown', (event) => {
  if (event.key !== 'Enter' || event.isComposing || event.shiftKey) return;
  if (result.readOnly) return;
  event.preventDefault();
  copy();
});

input.addEventListener('keydown', (event) => {
  if (event.key !== 'Enter' || event.isComposing) return;
  event.preventDefault();
  const text = input.value.trim();
  if (!text) return;
  input.value = '';
  api.submit(text);
});

window.addEventListener('keydown', (event) => {
  if (event.key === 'Escape' && !event.isComposing) api.close();
});

$('copy').addEventListener('click', copy);
$('retry').addEventListener('click', () => api.retry());
$('error-settings').addEventListener('click', () => api.openSettings());
$('message-button').addEventListener('click', () => {
  if (view?.button === 'accessibility') api.openAccessibility();
  else api.openSettings();
});

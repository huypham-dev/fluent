const api = window.fluent;
const $ = (id) => document.getElementById(id);
const result = $('result');
const input = $('input');

let view = null;
let lastPhase = null;
let copied = false;

const isGenerating = () => view?.state?.phase === 'generating';
const canCopy = () => view?.kind === 'session' && !isGenerating() && result.value.trim() !== '';

function copy() {
  if (copied || !canCopy()) return;
  copied = true;
  api.copy(result.value);
  renderCopyState();
}

function renderHint(keys) {
  $('hint').replaceChildren(
    ...keys.map(([key, label]) => {
      const item = document.createElement('span');
      const kbd = document.createElement('kbd');
      kbd.textContent = key;
      item.append(kbd, label);
      return item;
    }),
  );
}

function renderStatus() {
  const status = $('status');
  const generating = isGenerating();
  status.hidden = !generating && !copied;
  status.classList.toggle('copied', copied);
  status.replaceChildren();
  if (copied) {
    status.textContent = 'Copied to clipboard';
  } else if (generating) {
    const dot = document.createElement('span');
    dot.className = 'dot';
    status.append(dot, 'Writing…');
  }
}

function renderCopyState() {
  const button = $('copy');
  button.disabled = !copied && !canCopy();
  button.classList.toggle('copied', copied);
  $('copy-icon').setAttribute('href', copied ? '#i-check' : '#i-copy');
  $('copy-label').textContent = copied ? 'Copied' : 'Copy';
  renderStatus();
}

function renderSession() {
  const { state, title, editable, inputLabel, action } = view;
  const generating = state.phase === 'generating';
  if (generating) copied = false;
  $('title').textContent = title;
  $('title-icon').setAttribute('href', `#i-${action}`);
  input.placeholder = inputLabel;
  result.placeholder = state.phase === 'failed' ? 'No result yet.' : '';
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

  renderCopyState();
  renderHint(
    editable && canCopy()
      ? [['⏎', 'Copy'], ['⇧⏎', 'New line'], ['esc', 'Close']]
      : [['esc', 'Close']],
  );

  if (lastPhase === 'generating' && state.phase === 'ready') {
    (editable ? result : input).focus();
  }
  lastPhase = state.phase;
}

function renderMessage() {
  $('message-title').textContent = view.title;
  $('message-detail').textContent = view.detail;
  $('message-icon').setAttribute('href', view.button === 'accessibility' ? '#i-lock' : '#i-alert');
  const button = $('message-button');
  button.hidden = !view.button;
  button.textContent = view.button === 'accessibility' ? 'Open System Settings' : 'Open Settings';
}

// Registered before onView, which tells the main process the page is listening.
api.onPlacement((placement) => {
  document.body.classList.toggle('below', placement?.side === 'below');
  document.body.classList.toggle('above', placement?.side === 'above');
  if (placement) document.body.style.setProperty('--arrow-x', `${placement.arrowX}px`);
});

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
  renderCopyState();
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
$('close').addEventListener('click', () => api.close());
$('message-close').addEventListener('click', () => api.close());
$('retry').addEventListener('click', () => api.retry());
$('error-settings').addEventListener('click', () => api.openSettings());
$('message-button').addEventListener('click', () => {
  if (view?.button === 'accessibility') api.openAccessibility();
  else api.openSettings();
});

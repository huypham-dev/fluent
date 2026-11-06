const api = window.fluent;
const $ = (id) => document.getElementById(id);

const REQUIRED_FIELDS = {
  primaryLanguage: 'Primary language',
  secondaryLanguage: 'Secondary language',
  cliPath: 'Claude CLI path',
};
const MODEL_LABELS = { sonnet: 'Sonnet: better writing', haiku: 'Haiku: faster' };
const isMac = navigator.userAgent.includes('Macintosh');

let saved = null;
let saving = false;

function hotkeyLabel(hotkey) {
  return hotkey.replaceAll('+', ' + ').replace('Alt', isMac ? 'Option' : 'Alt');
}

function fillSelect(select, options, labelFor) {
  select.replaceChildren(
    ...options.map((option) => {
      const element = document.createElement('option');
      element.value = option;
      element.textContent = labelFor(option);
      return element;
    }),
  );
}

function render(settings) {
  saved = settings;
  $('hotkey').value = settings.hotkey;
  $('model').value = settings.model;
  $('autoBar').checked = settings.autoBar;
  for (const field of Object.keys(REQUIRED_FIELDS)) $(field).value = settings[field];
}

function readForm() {
  const next = {
    hotkey: $('hotkey').value,
    model: $('model').value,
    autoBar: $('autoBar').checked,
  };
  for (const field of Object.keys(REQUIRED_FIELDS)) next[field] = $(field).value.trim();
  return next;
}

function setStatus(text, kind = '') {
  const status = $('status');
  status.className = kind;
  status.replaceChildren();
  if (!text) return;
  if (kind) {
    const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    svg.setAttribute('class', 'icon');
    svg.setAttribute('aria-hidden', 'true');
    const use = document.createElementNS('http://www.w3.org/2000/svg', 'use');
    use.setAttribute('href', kind === 'saved' ? '#i-check' : '#i-alert');
    svg.append(use);
    status.append(svg);
  }
  status.append(text);
}

/** Refreshes the validation state and the Save button after any change. */
function refresh() {
  if (!saved) return;
  const next = readForm();
  const missing = Object.keys(REQUIRED_FIELDS).filter((field) => !next[field]);
  for (const field of Object.keys(REQUIRED_FIELDS)) {
    $(field).setAttribute('aria-invalid', String(missing.includes(field)));
  }
  const changed = Object.keys(next).some((key) => next[key] !== saved[key]);
  $('save').disabled = saving || !changed || missing.length > 0;
  if (missing.length > 0) setStatus(`${REQUIRED_FIELDS[missing[0]]} is required.`, 'error');
  else setStatus('');
}

api.getSettings().then(({ settings, hotkeys, models, hookRunning }) => {
  fillSelect($('hotkey'), hotkeys, hotkeyLabel);
  fillSelect($('model'), models, (model) => MODEL_LABELS[model] ?? model);
  render(settings);
  $('hook-notice').hidden = hookRunning;
  refresh();
});

$('form').addEventListener('input', refresh);

$('form').addEventListener('submit', async (event) => {
  event.preventDefault();
  if ($('save').disabled) return;
  saving = true;
  $('save').disabled = true;
  try {
    const stored = await api.saveSettings(readForm());
    saving = false;
    render(stored);
    refresh();
    setStatus('Saved', 'saved');
  } catch (error) {
    saving = false;
    refresh();
    // Electron prefixes errors from the main process with the channel name.
    const reason = error.message.replace(/^Error invoking remote method '[^']+': (Error: )?/, '');
    setStatus(`Could not save: ${reason}`, 'error');
  }
});

$('editStyle').addEventListener('click', () => api.editStyle());

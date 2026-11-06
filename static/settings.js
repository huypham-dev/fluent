const api = window.fluent;
const $ = (id) => document.getElementById(id);
const TEXT_FIELDS = ['cliPath', 'primaryLanguage', 'secondaryLanguage'];

function fillSelect(select, options, value) {
  select.replaceChildren(
    ...options.map((option) => {
      const element = document.createElement('option');
      element.value = option;
      element.textContent = option;
      return element;
    }),
  );
  select.value = value;
}

function render(settings) {
  $('hotkey').value = settings.hotkey;
  $('model').value = settings.model;
  $('autoBar').checked = settings.autoBar;
  for (const field of TEXT_FIELDS) $(field).value = settings[field];
}

api.getSettings().then(({ settings, hotkeys, models, hookRunning }) => {
  fillSelect($('hotkey'), hotkeys, settings.hotkey);
  fillSelect($('model'), models, settings.model);
  render(settings);
  if (!hookRunning) $('status').textContent = 'Mouse monitoring is not running.';
});

$('save').addEventListener('click', async () => {
  const next = {
    hotkey: $('hotkey').value,
    model: $('model').value,
    autoBar: $('autoBar').checked,
  };
  for (const field of TEXT_FIELDS) next[field] = $(field).value;
  render(await api.saveSettings(next));
  $('status').textContent = 'Saved';
});

$('editStyle').addEventListener('click', () => api.editStyle());

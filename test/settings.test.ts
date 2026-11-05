import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { HOTKEYS, loadSettings, parseSettings, saveSettings, type Settings } from '../src/settings';
import { StyleStore } from '../src/styleStore';

const defaults: Settings = {
  hotkey: 'Control+Alt+R',
  model: 'sonnet',
  cliPath: '/default/claude',
  primaryLanguage: 'Vietnamese',
  secondaryLanguage: 'English',
  autoBar: true,
  styleTemplateInstalled: false,
};

const tempPath = (name: string) => join(mkdtempSync(join(tmpdir(), 'textactions-test-')), name);

describe('settings', () => {
  it('offers the hotkeys from the spec', () => {
    expect(HOTKEYS).toEqual(['Control+Alt+R', 'Control+Alt+D', 'Control+Shift+Space']);
  });

  it('keeps valid values', () => {
    const parsed = parseSettings({ ...defaults, model: 'haiku', autoBar: false, primaryLanguage: 'Japanese' }, defaults);
    expect(parsed).toMatchObject({ model: 'haiku', autoBar: false, primaryLanguage: 'Japanese' });
  });

  it.each([
    [{ hotkey: 'Control+Q' }],
    [{ model: 'gpt' }],
    [{ cliPath: 5 }],
    [{ cliPath: '  ' }],
    [{ primaryLanguage: '' }],
    [{ secondaryLanguage: null }],
    [{ autoBar: 'yes' }],
  ])('falls back to the default for %j', (raw) => {
    expect(parseSettings(raw, defaults)).toEqual(defaults);
  });

  it.each([[null], ['text'], [[]]])('returns defaults for %j', (raw) => {
    expect(parseSettings(raw, defaults)).toEqual(defaults);
  });

  it('returns defaults for a missing or corrupt file', () => {
    const path = tempPath('settings.json');
    expect(loadSettings(path, defaults)).toEqual(defaults);
    writeFileSync(path, '{not json');
    expect(loadSettings(path, defaults)).toEqual(defaults);
  });

  it('saves and loads', () => {
    const path = tempPath('settings.json');
    saveSettings(path, { ...defaults, model: 'haiku' });
    expect(loadSettings(path, defaults).model).toBe('haiku');
    expect(JSON.parse(readFileSync(path, 'utf8')).model).toBe('haiku');
  });
});

describe('StyleStore', () => {
  it('creates the template and folders', () => {
    const store = new StyleStore(join(tempPath('x'), 'nested', 'style.md'));
    store.installTemplate();
    expect(store.load()).toBeTruthy();
  });

  it('never overwrites', () => {
    const path = tempPath('style.md');
    const store = new StyleStore(path);
    store.installTemplate();
    writeFileSync(path, 'mine');
    store.installTemplate();
    expect(store.load()).toBe('mine');
  });

  it('returns null when the file is missing', () => {
    expect(new StyleStore(tempPath('style.md')).load()).toBeNull();
  });
});

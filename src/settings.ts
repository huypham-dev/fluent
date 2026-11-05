import { mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import { dirname } from 'node:path';

export const HOTKEYS = ['Control+Alt+R', 'Control+Alt+D', 'Control+Shift+Space'] as const;
export const MODELS = ['sonnet', 'haiku'] as const;

export type Settings = {
  hotkey: (typeof HOTKEYS)[number];
  model: (typeof MODELS)[number];
  cliPath: string;
  primaryLanguage: string;
  secondaryLanguage: string;
  autoBar: boolean;
  styleTemplateInstalled: boolean;
};

const nonEmpty = (value: unknown): value is string => typeof value === 'string' && value.trim() !== '';
const oneOf = <T extends string>(options: readonly T[], value: unknown): value is T =>
  options.includes(value as T);

/** Keeps each valid value and uses the default for anything else. */
export function parseSettings(raw: unknown, defaults: Settings): Settings {
  const input = (raw !== null && typeof raw === 'object' ? raw : {}) as Record<string, unknown>;
  return {
    hotkey: oneOf(HOTKEYS, input.hotkey) ? input.hotkey : defaults.hotkey,
    model: oneOf(MODELS, input.model) ? input.model : defaults.model,
    cliPath: nonEmpty(input.cliPath) ? input.cliPath : defaults.cliPath,
    primaryLanguage: nonEmpty(input.primaryLanguage) ? input.primaryLanguage : defaults.primaryLanguage,
    secondaryLanguage: nonEmpty(input.secondaryLanguage) ? input.secondaryLanguage : defaults.secondaryLanguage,
    autoBar: typeof input.autoBar === 'boolean' ? input.autoBar : defaults.autoBar,
    styleTemplateInstalled:
      typeof input.styleTemplateInstalled === 'boolean' ? input.styleTemplateInstalled : defaults.styleTemplateInstalled,
  };
}

export function loadSettings(path: string, defaults: Settings): Settings {
  try {
    return parseSettings(JSON.parse(readFileSync(path, 'utf8')), defaults);
  } catch {
    return defaults;
  }
}

export function saveSettings(path: string, settings: Settings): void {
  mkdirSync(dirname(path), { recursive: true });
  const temporary = `${path}.tmp`;
  writeFileSync(temporary, JSON.stringify(settings, null, 2));
  renameSync(temporary, path);
}

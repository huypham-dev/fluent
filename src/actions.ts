import type { Settings } from './settings';

export type ActionId = 'draft' | 'explain' | 'translate';

export const ACTIONS: Record<ActionId, { title: string; editable: boolean; inputLabel: string }> = {
  draft: { title: 'Draft reply', editable: true, inputLabel: 'Rewrite: shorter, decline politely…' },
  explain: { title: 'Explain', editable: false, inputLabel: 'Ask a follow-up question…' },
  translate: { title: 'Translate', editable: true, inputLabel: 'Rewrite: more casual, shorter…' },
};

/**
 * Explain favors speed. Translate stays on the chosen model because Haiku often picks
 * the wrong target language when it must choose between the primary and secondary one.
 */
export function modelFor(action: ActionId, chosenModel: Settings['model']): Settings['model'] {
  return action === 'explain' ? 'haiku' : chosenModel;
}

export function isActionId(value: unknown): value is ActionId {
  return typeof value === 'string' && Object.hasOwn(ACTIONS, value);
}

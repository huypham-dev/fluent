export type ActionId = 'draft' | 'explain' | 'translate';

export const ACTIONS: Record<ActionId, { title: string; editable: boolean; inputLabel: string }> = {
  draft: { title: 'Draft reply', editable: true, inputLabel: 'Rewrite: shorter, decline politely…' },
  explain: { title: 'Explain', editable: false, inputLabel: 'Ask a follow-up question…' },
  translate: { title: 'Translate', editable: true, inputLabel: 'Rewrite: more casual, shorter…' },
};

export function isActionId(value: unknown): value is ActionId {
  return typeof value === 'string' && Object.hasOwn(ACTIONS, value);
}

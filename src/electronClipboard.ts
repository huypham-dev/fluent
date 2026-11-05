import { clipboard, ClipboardItem } from 'electron';
import type { ClipboardPort } from './captureSelection';
import { logEvent } from './diagnostics';

export type ClipboardSnapshot = { items: ClipboardItem[]; text: string };

/**
 * Copies each item's data out so the snapshot stays valid after the clipboard changes.
 * The plain text is kept separately as a fallback if a format cannot be written back.
 */
export const electronClipboard: ClipboardPort<ClipboardSnapshot> = {
  save: async () => {
    const [items, text] = await Promise.all([clipboard.read(), clipboard.readText()]);
    const copies = await Promise.all(
      items.map(async (item) => {
        const data: ConstructorParameters<typeof ClipboardItem>[0] = {};
        for (const type of item.types) {
          data[type] = await item.getType(type);
        }
        return new ClipboardItem(data);
      }),
    );
    return { items: copies, text };
  },
  restore: async ({ items, text }) => {
    clipboard.clear();
    if (items.length === 0) return;
    try {
      await clipboard.write(items);
    } catch (error) {
      logEvent('clipboard-restore-failed', { error: String(error) });
      if (text) await clipboard.writeText(text);
    }
  },
  writeText: (text) => clipboard.writeText(text),
  readText: () => clipboard.readText(),
};

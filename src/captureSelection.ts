type MaybePromise<T> = T | Promise<T>;

export type ClipboardPort<S> = {
  save(): MaybePromise<S>;
  restore(snapshot: S): MaybePromise<void>;
  writeText(text: string): MaybePromise<void>;
  readText(): MaybePromise<string>;
};

export type CaptureResult =
  | { kind: 'text'; text: string }
  | { kind: 'noSelection' }
  | { kind: 'noText' };

/**
 * Reads the selection by triggering a copy. Electron has no clipboard change counter,
 * so a unique marker shows whether the copy replaced the clipboard's content.
 */
export async function captureSelection<S>(deps: {
  clipboard: ClipboardPort<S>;
  sendCopy: () => void | Promise<void>;
  marker: string;
  timeoutMs?: number;
  pollMs?: number;
  /** Sends copy again at this interval while the marker stays. Off by default. */
  resendMs?: number;
}): Promise<CaptureResult> {
  const { clipboard, marker, timeoutMs = 500, pollMs = 10, resendMs } = deps;
  const snapshot = await clipboard.save();
  try {
    await clipboard.writeText(marker);
    await deps.sendCopy();

    const deadline = Date.now() + timeoutMs;
    let nextSend = resendMs === undefined ? Infinity : Date.now() + resendMs;
    let text = await clipboard.readText();
    // Some apps clear the clipboard before writing, so empty text may be temporary.
    while ((text === marker || text === '') && Date.now() < deadline) {
      await new Promise((resolve) => setTimeout(resolve, pollMs));
      text = await clipboard.readText();
      if (text === marker && resendMs !== undefined && Date.now() >= nextSend) {
        await deps.sendCopy();
        nextSend = Date.now() + resendMs;
      }
    }

    if (text === marker) return { kind: 'noSelection' };
    if (text === '') return { kind: 'noText' };
    return text.trim() === '' ? { kind: 'noSelection' } : { kind: 'text', text };
  } finally {
    await clipboard.restore(snapshot);
  }
}

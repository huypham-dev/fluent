import { describe, expect, it } from 'vitest';
import { captureSelection, type ClipboardPort } from '../src/captureSelection';

const MARKER = 'marker-123';

function fakeClipboard(initial: string) {
  let text = initial;
  const clipboard: ClipboardPort<string> = {
    save: () => text,
    restore: (snapshot) => {
      text = snapshot;
    },
    writeText: (value) => {
      text = value;
    },
    readText: () => text,
  };
  return {
    clipboard,
    set: (value: string) => {
      text = value;
    },
    get: () => text,
  };
}

const options = { marker: MARKER, timeoutMs: 60, pollMs: 2 };

describe('captureSelection', () => {
  it('keeps waiting when an app clears the clipboard before writing to it', async () => {
    const fake = fakeClipboard('old');
    const result = await captureSelection({
      ...options,
      clipboard: fake.clipboard,
      sendCopy: () => {
        fake.set('');
        setTimeout(() => fake.set('selected'), 15);
      },
    });
    expect(result).toEqual({ kind: 'text', text: 'selected' });
  });

  it('works with an asynchronous clipboard', async () => {
    let text = 'old';
    const later = async <T>(value: () => T): Promise<T> => {
      await new Promise((resolve) => setTimeout(resolve, 1));
      return value();
    };
    const result = await captureSelection({
      ...options,
      clipboard: {
        save: () => later(() => text),
        restore: (snapshot) =>
          later(() => {
            text = snapshot;
          }),
        writeText: (value) =>
          later(() => {
            text = value;
          }),
        readText: () => later(() => text),
      },
      sendCopy: () => {
        text = 'selected';
      },
    });
    expect(result).toEqual({ kind: 'text', text: 'selected' });
    expect(text).toBe('old');
  });

  it('returns copied text and restores the clipboard', async () => {
    const fake = fakeClipboard('old');
    const result = await captureSelection({
      ...options,
      clipboard: fake.clipboard,
      sendCopy: () => fake.set('selected'),
    });
    expect(result).toEqual({ kind: 'text', text: 'selected' });
    expect(fake.get()).toBe('old');
  });

  it('writes the marker before sending copy', async () => {
    const fake = fakeClipboard('old');
    let seen = '';
    await captureSelection({
      ...options,
      clipboard: fake.clipboard,
      sendCopy: () => {
        seen = fake.get();
      },
    });
    expect(seen).toBe(MARKER);
  });

  it('sees a copy that arrives after a delay', async () => {
    const fake = fakeClipboard('old');
    const result = await captureSelection({
      ...options,
      clipboard: fake.clipboard,
      sendCopy: () => {
        setTimeout(() => fake.set('late'), 15);
      },
    });
    expect(result).toEqual({ kind: 'text', text: 'late' });
  });

  it('reports no selection when the marker stays and restores', async () => {
    const fake = fakeClipboard('old');
    const result = await captureSelection({ ...options, clipboard: fake.clipboard, sendCopy: () => {} });
    expect(result).toEqual({ kind: 'noSelection' });
    expect(fake.get()).toBe('old');
  });

  it('treats whitespace as no selection and restores', async () => {
    const fake = fakeClipboard('old');
    const result = await captureSelection({
      ...options,
      clipboard: fake.clipboard,
      sendCopy: () => fake.set(' \n '),
    });
    expect(result).toEqual({ kind: 'noSelection' });
    expect(fake.get()).toBe('old');
  });

  it('reports no text when non-text content replaces the marker', async () => {
    const fake = fakeClipboard('old');
    const result = await captureSelection({
      ...options,
      clipboard: fake.clipboard,
      sendCopy: () => fake.set(''),
    });
    expect(result).toEqual({ kind: 'noText' });
    expect(fake.get()).toBe('old');
  });

  it('restores the clipboard when sending copy throws', async () => {
    const fake = fakeClipboard('old');
    await expect(
      captureSelection({
        ...options,
        clipboard: fake.clipboard,
        sendCopy: () => {
          throw new Error('hook failed');
        },
      }),
    ).rejects.toThrow('hook failed');
    expect(fake.get()).toBe('old');
  });
});

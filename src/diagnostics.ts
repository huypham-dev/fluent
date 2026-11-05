import { appendFileSync } from 'node:fs';
import { join } from 'node:path';
import { app } from 'electron';

export function logPath(): string {
  return join(app.getPath('userData'), 'diagnostics.log');
}

/** Records event names and text lengths for support. Never pass selected text or results. */
export function logEvent(event: string, details: Record<string, string | number | boolean> = {}): void {
  const line = JSON.stringify({ time: new Date().toISOString(), event, ...details });
  try {
    appendFileSync(logPath(), `${line}\n`);
  } catch {
    // Diagnostics must never break the app.
  }
}

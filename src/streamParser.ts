export type StreamEvent =
  | { type: 'text'; text: string }
  | { type: 'completed' }
  | { type: 'failed'; message: string }
  | { type: 'ignored' };

/** Parses one line of the Claude CLI's `stream-json` output. */
export function parseStreamLine(line: string): StreamEvent {
  let data: any;
  try {
    data = JSON.parse(line);
  } catch {
    return { type: 'ignored' };
  }
  if (data?.type === 'result') {
    if (data.is_error !== true) return { type: 'completed' };
    const message = typeof data.result === 'string' && data.result ? data.result : 'The Claude CLI reported an error.';
    return { type: 'failed', message };
  }
  if (data?.type !== 'stream_event') return { type: 'ignored' };
  if (data.event?.type === 'message_stop') return { type: 'completed' };
  const delta = data.event?.type === 'content_block_delta' ? data.event.delta : null;
  return delta?.type === 'text_delta' && typeof delta.text === 'string'
    ? { type: 'text', text: delta.text }
    : { type: 'ignored' };
}

/** Decodes only complete lines, so a multibyte character split across reads stays intact. */
export class LineBuffer {
  private pending = Buffer.alloc(0);

  push(chunk: Buffer): string[] {
    this.pending = Buffer.concat([this.pending, chunk]);
    const lines: string[] = [];
    let newline: number;
    while ((newline = this.pending.indexOf(0x0a)) !== -1) {
      lines.push(this.pending.subarray(0, newline).toString('utf8').replace(/\r$/, ''));
      this.pending = this.pending.subarray(newline + 1);
    }
    return lines;
  }
}

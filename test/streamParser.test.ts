import { describe, expect, it } from 'vitest';
import { LineBuffer, parseStreamLine } from '../src/streamParser';

describe('parseStreamLine', () => {
  it('reads a text delta', () => {
    const line = JSON.stringify({
      type: 'stream_event',
      event: { type: 'content_block_delta', delta: { type: 'text_delta', text: 'Hi' } },
    });
    expect(parseStreamLine(line)).toEqual({ type: 'text', text: 'Hi' });
  });

  it('treats message_stop as completion', () => {
    const line = JSON.stringify({ type: 'stream_event', event: { type: 'message_stop' } });
    expect(parseStreamLine(line)).toEqual({ type: 'completed' });
  });

  it('reads an error result', () => {
    const line = JSON.stringify({ type: 'result', is_error: true, result: 'Not logged in' });
    expect(parseStreamLine(line)).toEqual({ type: 'failed', message: 'Not logged in' });
  });

  it('uses a fallback for an error result with no message', () => {
    expect(parseStreamLine(JSON.stringify({ type: 'result', is_error: true }))).toEqual({
      type: 'failed',
      message: 'The Claude CLI reported an error.',
    });
  });

  it('treats a successful result as completion', () => {
    expect(parseStreamLine(JSON.stringify({ type: 'result', is_error: false }))).toEqual({ type: 'completed' });
  });

  it.each(['', 'not json', 'null', '{"type":"system"}'])('ignores %j', (line) => {
    expect(parseStreamLine(line)).toEqual({ type: 'ignored' });
  });
});

describe('LineBuffer', () => {
  it('returns complete lines and keeps the rest', () => {
    const buffer = new LineBuffer();
    expect(buffer.push(Buffer.from('a\nb\nc'))).toEqual(['a', 'b']);
    expect(buffer.push(Buffer.from('\n'))).toEqual(['c']);
  });

  it('joins a multibyte character split across chunks', () => {
    const buffer = new LineBuffer();
    const bytes = Buffer.from('sử dụng\n');
    expect(buffer.push(bytes.subarray(0, 2))).toEqual([]);
    expect(buffer.push(bytes.subarray(2))).toEqual(['sử dụng']);
  });

  it('strips the carriage return from Windows line endings', () => {
    expect(new LineBuffer().push(Buffer.from('a\r\n'))).toEqual(['a']);
  });
});

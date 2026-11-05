import { existsSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { CliError, defaultCliPath, runCli } from '../src/cliGenerator';

const delta = (text: string) =>
  JSON.stringify({ type: 'stream_event', event: { type: 'content_block_delta', delta: { type: 'text_delta', text } } });
const STOP = JSON.stringify({ type: 'stream_event', event: { type: 'message_stop' } });

/** Writes a fake CLI as a Node script, so these tests run on macOS and Windows. */
function fakeCli(body: string) {
  const dir = mkdtempSync(join(tmpdir(), 'textactions-test-'));
  const script = join(dir, 'fake.js');
  writeFileSync(script, `const fs = require('fs'); const dir = ${JSON.stringify(dir)};\n${body}`);
  return { dir, options: { executable: process.execPath, prefixArgs: [script], model: 'sonnet', cwd: dir } };
}

const READ_STDIN = `process.stdin.resume(); process.stdin.on('end', main); process.stdin.on('error', () => {});`;

async function collect(options: Parameters<typeof runCli>[0]) {
  let text = '';
  let error: unknown = null;
  try {
    await runCli(options, (piece) => {
      text += piece;
    }).done;
  } catch (caught) {
    error = caught;
  }
  return { text, error };
}

describe('runCli', () => {
  it('streams text in order, including Vietnamese', async () => {
    const cli = fakeCli(`${READ_STDIN}
      function main() { console.log(${JSON.stringify(delta('Xin '))}); console.log(${JSON.stringify(delta('chào ử'))}); console.log(${JSON.stringify(STOP)}); }`);
    const result = await collect({ ...cli.options, system: 's', prompt: 'p' });
    expect(result).toEqual({ text: 'Xin chào ử', error: null });
  });

  it('finishes on message_stop without waiting for the process to exit', async () => {
    const cli = fakeCli(`${READ_STDIN}
      function main() { console.log(${JSON.stringify(STOP)}); setTimeout(() => {}, 20000); }`);
    const start = Date.now();
    const result = await collect({ ...cli.options, system: 's', prompt: 'p' });
    expect(result.error).toBeNull();
    expect(Date.now() - start).toBeLessThan(5000);
  });

  it('sends the prompt over stdin and the expected flags', async () => {
    const cli = fakeCli(`let input = ''; process.stdin.on('data', (d) => { input += d; }); process.stdin.on('end', () => {
        const args = process.argv.slice(2);
        const system = fs.readFileSync(args[args.indexOf('--system-prompt-file') + 1], 'utf8');
        fs.writeFileSync(dir + '/seen.json', JSON.stringify({ input, args, system }));
        console.log(${JSON.stringify(STOP)});
      });`);
    await collect({ ...cli.options, system: 'SYSTEM ử', prompt: 'the prompt' });
    const seen = JSON.parse(readFileSync(join(cli.dir, 'seen.json'), 'utf8'));
    expect(seen.input).toBe('the prompt');
    expect(seen.system).toBe('SYSTEM ử');
    expect(seen.args).not.toContain('the prompt');
    expect(seen.args).not.toContain('--bare');
    for (const flag of ['-p', '--strict-mcp-config', '--no-session-persistence', '--verbose', '--include-partial-messages']) {
      expect(seen.args).toContain(flag);
    }
    const value = (flag: string) => seen.args[seen.args.indexOf(flag) + 1];
    expect(value('--tools')).toBe('');
    expect(value('--setting-sources')).toBe('');
    expect(value('--model')).toBe('sonnet');
    expect(value('--output-format')).toBe('stream-json');
  });

  it.each([
    ['success', `console.log(${JSON.stringify(STOP)});`],
    ['failure', `process.exit(1);`],
  ])('deletes the system prompt file after %s', async (_name, finish) => {
    const cli = fakeCli(`${READ_STDIN}
      function main() { const a = process.argv.slice(2); fs.writeFileSync(dir + '/path.txt', a[a.indexOf('--system-prompt-file') + 1]); ${finish} }`);
    await collect({ ...cli.options, system: 's', prompt: 'p' });
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(existsSync(readFileSync(join(cli.dir, 'path.txt'), 'utf8'))).toBe(false);
  });

  it('deletes the system prompt file after cancel', async () => {
    const cli = fakeCli(`${READ_STDIN}
      function main() { const a = process.argv.slice(2); fs.writeFileSync(dir + '/path.txt', a[a.indexOf('--system-prompt-file') + 1]); console.log(${JSON.stringify(delta('x'))}); setTimeout(() => {}, 20000); }`);
    let cancel = () => {};
    const done = new Promise<void>((resolve, reject) => {
      const run = runCli({ ...cli.options, system: 's', prompt: 'p' }, () => run.cancel());
      cancel = run.cancel;
      run.done.then(resolve, reject);
    });
    await done;
    cancel();
    await new Promise((resolve) => setTimeout(resolve, 100));
    expect(existsSync(readFileSync(join(cli.dir, 'path.txt'), 'utf8'))).toBe(false);
  });

  it('reports a missing executable', async () => {
    const result = await collect({ executable: '/nonexistent/claude', model: 's', cwd: tmpdir(), system: 's', prompt: 'p' });
    expect(result.error).toBeInstanceOf(CliError);
    expect((result.error as CliError).kind).toBe('notFound');
  });

  it.each(['C:\\tools\\claude.cmd', 'C:\\tools\\CLAUDE.BAT'])('rejects %s as unsupported', async (executable) => {
    const result = await collect({ executable, model: 's', cwd: tmpdir(), system: 's', prompt: 'p' });
    expect((result.error as CliError).kind).toBe('unsupported');
  });

  it('reports a non-zero exit with stderr', async () => {
    const cli = fakeCli(`${READ_STDIN} function main() { console.error('boom happened'); process.exit(3); }`);
    const result = await collect({ ...cli.options, system: 's', prompt: 'p' });
    expect((result.error as CliError).kind).toBe('failed');
    expect((result.error as CliError).message).toContain('boom happened');
  });

  it('reports an error result with its message and keeps earlier text', async () => {
    const failure = JSON.stringify({ type: 'result', is_error: true, result: 'Not logged in' });
    const cli = fakeCli(`${READ_STDIN} function main() { console.log(${JSON.stringify(delta('partial'))}); console.log(${JSON.stringify(failure)}); }`);
    const result = await collect({ ...cli.options, system: 's', prompt: 'p' });
    expect(result.text).toBe('partial');
    expect((result.error as CliError).message).toBe('Not logged in');
  });

  it('treats an exit without completion as a failure', async () => {
    const cli = fakeCli(`${READ_STDIN} function main() { console.log(${JSON.stringify(delta('partial'))}); }`);
    const result = await collect({ ...cli.options, system: 's', prompt: 'p' });
    expect((result.error as CliError).kind).toBe('failed');
  });

  it('times out on silence', async () => {
    const cli = fakeCli(`setTimeout(() => {}, 20000);`);
    const result = await collect({ ...cli.options, system: 's', prompt: 'p', inactivityMs: 300 });
    expect((result.error as CliError).kind).toBe('timeout');
  });

  it('resets the timeout whenever output arrives', async () => {
    const cli = fakeCli(`${READ_STDIN}
      function main() { let n = 0; const t = setInterval(() => { console.log(${JSON.stringify(delta('a'))}); if (++n === 8) { clearInterval(t); console.log(${JSON.stringify(STOP)}); } }, 200); }`);
    const result = await collect({ ...cli.options, system: 's', prompt: 'p', inactivityMs: 1000 });
    expect(result).toEqual({ text: 'aaaaaaaa', error: null });
  }, 15000);

  it('terminates the process on cancel and resolves', async () => {
    const cli = fakeCli(`${READ_STDIN}
      function main() { fs.writeFileSync(dir + '/pid.txt', String(process.pid)); console.log(${JSON.stringify(delta('x'))}); setTimeout(() => {}, 20000); }`);
    await new Promise<void>((resolve, reject) => {
      const run = runCli({ ...cli.options, system: 's', prompt: 'p' }, () => run.cancel());
      run.done.then(resolve, reject);
    });
    const pid = Number(readFileSync(join(cli.dir, 'pid.txt'), 'utf8'));
    let alive = true;
    for (let i = 0; i < 100 && alive; i += 1) {
      try {
        process.kill(pid, 0);
        await new Promise((resolve) => setTimeout(resolve, 20));
      } catch {
        alive = false;
      }
    }
    expect(alive).toBe(false);
  });

  it('survives a process that exits before reading a large prompt', async () => {
    const cli = fakeCli(`process.exit(1);`);
    const result = await collect({ ...cli.options, system: 's', prompt: 'x'.repeat(2_000_000) });
    expect((result.error as CliError).kind).toBe('failed');
  });
});

describe('defaultCliPath', () => {
  it('uses the native install location on each platform', () => {
    expect(defaultCliPath('darwin', '/Users/a')).toBe('/Users/a/.local/bin/claude');
    expect(defaultCliPath('win32', 'C:\\Users\\a')).toBe('C:\\Users\\a\\.local\\bin\\claude.exe');
  });
});

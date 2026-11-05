import { spawn } from 'node:child_process';
import { accessSync, constants, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { delimiter, join, posix, win32 } from 'node:path';
import { LineBuffer, parseStreamLine } from './streamParser';

export type CliErrorKind = 'notFound' | 'unsupported' | 'failed' | 'timeout';

export class CliError extends Error {
  constructor(
    readonly kind: CliErrorKind,
    message: string,
  ) {
    super(message);
  }
}

export type CliOptions = {
  executable: string;
  /** Arguments placed before the CLI flags. Tests use this to run a script. */
  prefixArgs?: string[];
  model: string;
  system: string;
  prompt: string;
  cwd: string;
  inactivityMs?: number;
};

export type CliRun = { done: Promise<void>; cancel(): void };

const EXIT_GRACE_MS = 300;
const STDERR_LIMIT = 8192;

export function defaultCliPath(platform: NodeJS.Platform, home: string): string {
  return platform === 'win32'
    ? win32.join(home, '.local', 'bin', 'claude.exe')
    : posix.join(home, '.local', 'bin', 'claude');
}

function expandHome(path: string): string {
  return path === '~' || path.startsWith('~/') || path.startsWith('~\\') ? join(homedir(), path.slice(1)) : path;
}

/** Apps started from Finder or at login do not inherit the shell's `PATH`. */
function environment(): NodeJS.ProcessEnv {
  const extra = process.platform === 'win32'
    ? [join(homedir(), '.local', 'bin')]
    : [join(homedir(), '.local', 'bin'), '/opt/homebrew/bin', '/usr/local/bin'];
  return { ...process.env, PATH: [...extra, process.env.PATH ?? ''].join(delimiter) };
}

function validateExecutable(executable: string): void {
  if (/\.(cmd|bat)$/i.test(executable)) {
    throw new CliError('unsupported', `${executable} is a script launcher, which is not supported. Use claude.exe.`);
  }
  try {
    accessSync(executable, constants.X_OK);
  } catch {
    throw new CliError('notFound', `Claude CLI not found at ${executable}.`);
  }
}

/** Runs the CLI once without a shell, so text can never be interpreted as a command. */
export function runCli(options: CliOptions, onText: (text: string) => void): CliRun {
  let cancel = () => {};
  const done = new Promise<void>((resolve, reject) => {
    const executable = expandHome(options.executable);
    validateExecutable(executable);

    const promptDir = mkdtempSync(join(tmpdir(), 'textactions-'));
    const systemFile = join(promptDir, 'system.txt');
    writeFileSync(systemFile, options.system, { mode: 0o600 });

    const child = spawn(
      executable,
      [
        ...(options.prefixArgs ?? []),
        '-p',
        '--system-prompt-file', systemFile,
        '--tools', '',
        '--setting-sources', '',
        '--strict-mcp-config',
        '--no-session-persistence',
        '--model', options.model,
        '--output-format', 'stream-json',
        '--verbose',
        '--include-partial-messages',
      ],
      { cwd: options.cwd, env: environment(), shell: false, windowsHide: true },
    );

    const inactivityMs = options.inactivityMs ?? 30_000;
    const lines = new LineBuffer();
    let stderr = '';
    let finished = false;
    let timer: NodeJS.Timeout | undefined;

    const armTimer = () => {
      clearTimeout(timer);
      timer = setTimeout(
        () => finish(new CliError('timeout', `No response for ${Math.round(inactivityMs / 1000)} seconds.`)),
        inactivityMs,
      );
    };

    const finish = (error: CliError | null) => {
      if (finished) return;
      finished = true;
      clearTimeout(timer);
      child.kill();
      rmSync(promptDir, { recursive: true, force: true });
      if (error) reject(error);
      else resolve();
    };

    cancel = () => finish(null);

    child.on('error', (error) => finish(new CliError('failed', error.message)));
    child.stderr.on('data', (chunk: Buffer) => {
      stderr = (stderr + chunk.toString('utf8')).slice(-STDERR_LIMIT);
    });
    child.stdout.on('data', (chunk: Buffer) => {
      if (finished) return;
      armTimer();
      for (const line of lines.push(chunk)) {
        const event = parseStreamLine(line);
        if (event.type === 'text') onText(event.text);
        if (event.type === 'completed') return finish(null);
        if (event.type === 'failed') return finish(new CliError('failed', event.message));
        if (finished) return;
      }
    });
    child.on('exit', (code) => {
      // Give the pipes a moment to deliver the last output.
      setTimeout(() => {
        const message = stderr.trim()
          || (code ? `The Claude CLI exited with status ${code}.` : 'The Claude CLI stopped before finishing.');
        finish(new CliError('failed', message));
      }, EXIT_GRACE_MS);
    });

    // A CLI that exits early closes stdin. That surfaces through the exit handler.
    child.stdin.on('error', () => {});
    child.stdin.end(options.prompt);
    armTimer();
  });
  return { done, cancel: () => cancel() };
}

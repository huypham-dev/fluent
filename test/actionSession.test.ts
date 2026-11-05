import { describe, expect, it } from 'vitest';
import { ActionSession, type GenerateRequest } from '../src/actionSession';
import { CliError } from '../src/cliGenerator';

type Script = { pieces?: string[]; error?: CliError; hang?: boolean };

function setup(scripts: Script[], style: () => string | null = () => null, action: 'draft' | 'explain' = 'draft') {
  const requests: GenerateRequest[] = [];
  let cancels = 0;
  const session = new ActionSession({
    action,
    selectedText: 'Review my PR?',
    languages: { primaryLanguage: 'Vietnamese', secondaryLanguage: 'English' },
    loadStyle: style,
    generate: (request, onText) => {
      requests.push(request);
      const script = scripts.shift() ?? {};
      for (const piece of script.pieces ?? []) onText(piece);
      return {
        done: script.hang
          ? new Promise<void>(() => {})
          : script.error
            ? Promise.reject(script.error)
            : Promise.resolve(),
        cancel: () => {
          cancels += 1;
        },
      };
    },
  });
  return { session, requests, cancels: () => cancels };
}

const settled = () => new Promise((resolve) => setTimeout(resolve, 5));

describe('ActionSession', () => {
  it('accumulates text and becomes ready', async () => {
    const { session } = setup([{ pieces: ['Sure', ', today'] }]);
    session.start();
    expect(session.state.phase).toBe('generating');
    await settled();
    expect(session.state).toMatchObject({ phase: 'ready', result: 'Sure, today' });
  });

  it('notifies on change', async () => {
    const { session } = setup([{ pieces: ['a'] }]);
    const phases: string[] = [];
    session.onChange = (state) => phases.push(state.phase);
    session.start();
    await settled();
    expect(phases.at(-1)).toBe('ready');
    expect(phases.length).toBeGreaterThan(1);
  });

  it('keeps partial text on failure', async () => {
    const error = new CliError('failed', 'usage limit');
    const { session } = setup([{ pieces: ['Sure'], error }]);
    session.start();
    await settled();
    expect(session.state).toMatchObject({
      phase: 'failed',
      result: 'Sure',
      error: { kind: 'failed', message: 'usage limit' },
    });
  });

  it('sends the edited result in a follow-up', async () => {
    const { session, requests } = setup([{ pieces: ['First'] }, { pieces: ['Short'] }]);
    session.start();
    await settled();
    session.setResult('My edit');
    session.submitInput('shorter');
    await settled();
    expect(requests[1]?.prompt).toContain('My edit');
    expect(requests[1]?.prompt).toContain('shorter');
    expect(requests[1]?.prompt).toContain('Review my PR?');
    expect(session.state.result).toBe('Short');
  });

  it('restores the edited result when a follow-up fails with no output', async () => {
    const { session } = setup([{ pieces: ['First'] }, { error: new CliError('timeout', 'slow') }]);
    session.start();
    await settled();
    session.setResult('My edit');
    session.submitInput('shorter');
    await settled();
    expect(session.state).toMatchObject({ phase: 'failed', result: 'My edit' });
  });

  it('retries the last request', async () => {
    const { session, requests } = setup([
      { pieces: ['First'] },
      { error: new CliError('timeout', 'slow') },
      { pieces: ['Short'] },
    ]);
    session.start();
    await settled();
    session.setResult('My edit');
    session.submitInput('shorter');
    await settled();
    session.retry();
    await settled();
    expect(requests[2]?.prompt).toBe(requests[1]?.prompt);
    expect(session.state).toMatchObject({ phase: 'ready', result: 'Short' });
  });

  it('cancel stops the running generation', async () => {
    const { session, cancels } = setup([{ pieces: ['x'], hang: true }]);
    session.start();
    session.cancel();
    expect(cancels()).toBe(1);
  });

  it('a follow-up during generation stops the running one', async () => {
    const { session, cancels } = setup([{ pieces: ['Old'], hang: true }, { pieces: ['New'] }]);
    session.start();
    session.submitInput('shorter');
    await settled();
    expect(cancels()).toBe(1);
    expect(session.state).toMatchObject({ phase: 'ready', result: 'New' });
  });

  it('ignores blank input', () => {
    const { session, requests } = setup([{ pieces: ['x'] }]);
    session.start();
    session.submitInput('   ');
    expect(requests).toHaveLength(1);
  });

  it('ignores edits while generating', () => {
    const { session } = setup([{ pieces: ['x'], hang: true }]);
    session.start();
    session.setResult('mine');
    expect(session.state.result).toBe('x');
  });

  it('reads the style for each generation', async () => {
    let style = 'first style';
    const { session, requests } = setup([{}, {}], () => style);
    session.start();
    await settled();
    style = 'second style';
    session.submitInput('again');
    await settled();
    expect(requests[0]?.system).toContain('first style');
    expect(requests[1]?.system).toContain('second style');
  });

  it('turns an unexpected error into a failed state', async () => {
    const { session } = setup([{ error: new Error('odd') as CliError }]);
    session.start();
    await settled();
    expect(session.state).toMatchObject({ phase: 'failed', error: { kind: 'failed', message: 'odd' } });
  });
});

import type { ActionId } from './actions';
import { CliError, type CliErrorKind, type CliRun } from './cliGenerator';
import { systemPrompt, userPrompt, type Languages } from './prompt';

export type GenerateRequest = { system: string; prompt: string };
export type Generate = (request: GenerateRequest, onText: (text: string) => void) => CliRun;

export type SessionState = {
  phase: 'generating' | 'ready' | 'failed';
  result: string;
  error: { kind: CliErrorKind; message: string } | null;
};

/** The state of one action, from the first generation through any follow-ups. */
export class ActionSession {
  state: SessionState = { phase: 'generating', result: '', error: null };
  onChange: ((state: SessionState) => void) | null = null;

  private active: CliRun | null = null;
  private runId = 0;
  private lastPrompt: string;

  constructor(
    private readonly deps: {
      action: ActionId;
      selectedText: string;
      languages: Languages;
      loadStyle: () => string | null;
      generate: Generate;
    },
  ) {
    this.lastPrompt = userPrompt({ selectedText: deps.selectedText });
  }

  start(): void {
    this.run(this.lastPrompt);
  }

  retry(): void {
    this.run(this.lastPrompt);
  }

  submitInput(text: string): void {
    const instruction = text.trim();
    if (!instruction) return;
    this.run(
      userPrompt({
        selectedText: this.deps.selectedText,
        followUp: { currentResult: this.state.result, instruction },
      }),
    );
  }

  setResult(text: string): void {
    if (this.state.phase === 'generating') return;
    this.state = { ...this.state, result: text };
  }

  cancel(): void {
    this.runId += 1;
    this.active?.cancel();
    this.active = null;
  }

  private run(prompt: string): void {
    this.cancel();
    const id = this.runId;
    const previousResult = this.state.result;
    let receivedText = false;
    this.lastPrompt = prompt;
    this.update({ phase: 'generating', result: '', error: null });

    const run = this.deps.generate(
      {
        system: systemPrompt(this.deps.action, { ...this.deps.languages, style: this.deps.loadStyle() }),
        prompt,
      },
      (text) => {
        if (id !== this.runId) return;
        receivedText = true;
        this.update({ ...this.state, result: this.state.result + text });
      },
    );
    this.active = run;

    run.done.then(
      () => {
        if (id === this.runId) this.update({ ...this.state, phase: 'ready' });
      },
      (error: unknown) => {
        if (id !== this.runId) return;
        this.update({
          phase: 'failed',
          result: receivedText ? this.state.result : previousResult,
          error: {
            kind: error instanceof CliError ? error.kind : 'failed',
            message: error instanceof Error ? error.message : String(error),
          },
        });
      },
    );
  }

  private update(state: SessionState): void {
    this.state = state;
    this.onChange?.(state);
  }
}

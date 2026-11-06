import { tmpdir } from 'node:os';
import { homedir } from 'node:os';
import { describe, expect, it } from 'vitest';
import type { ActionId } from '../src/actions';
import { defaultCliPath, runCli } from '../src/cliGenerator';
import { systemPrompt, userPrompt } from '../src/prompt';

/** Calls the real Claude CLI and uses real usage. Run with `FLUENT_LIVE=1 npx vitest run test/live`. */
describe.runIf(process.env.FLUENT_LIVE === '1')('live CLI', () => {
  it.each<[ActionId, string]>([
    ['draft', 'Can you review my PR today?'],
    ['explain', 'The deployment failed because of a race condition in the migration.'],
    ['translate', 'Please let me know when the deployment is finished.'],
    ['translate', 'Bạn gửi link PR giúp mình nhé.'],
  ])('%s: %s', async (action, selectedText) => {
    const start = Date.now();
    let text = '';
    await runCli(
      {
        executable: defaultCliPath(process.platform, homedir()),
        model: 'sonnet',
        cwd: tmpdir(),
        system: systemPrompt(action, { style: null, primaryLanguage: 'Vietnamese', secondaryLanguage: 'English' }),
        prompt: userPrompt({ selectedText }),
      },
      (piece) => {
        text += piece;
      },
    ).done;
    console.log(`LIVE ${action} ${Date.now() - start}ms: ${text}`);
    expect(text.trim()).not.toBe('');
  }, 60000);
});

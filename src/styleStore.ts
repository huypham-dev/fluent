import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname } from 'node:path';

const TEMPLATE = `# My reply style

Describe how you write. This file is read for every Draft reply, so edits apply immediately.

## About me
- Role: (for example, backend engineer)

## Voice
- Direct and friendly, with no filler.
- Simple, common words, like everyday chat.
- Short messages, usually one to three sentences.

## Examples of how I write
- "Sure, I'll take a look this afternoon."
- "Good catch, I'll push a fix today."
`;

/** The user's editable style guide for drafts. */
export class StyleStore {
  constructor(readonly path: string) {}

  installTemplate(): void {
    if (existsSync(this.path)) return;
    mkdirSync(dirname(this.path), { recursive: true });
    writeFileSync(this.path, TEMPLATE);
  }

  load(): string | null {
    try {
      return readFileSync(this.path, 'utf8');
    } catch {
      return null;
    }
  }
}

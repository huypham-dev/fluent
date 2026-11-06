import { constants, copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';

/** The app was called TextActions, and its data folder was named after it. */
export const LEGACY_DATA_FOLDER = 'TextActions';

const USER_FILES = ['settings.json', 'style.md'];

/**
 * Copies the user's own files from the old data folder, leaving the originals in
 * place. A file already in the new folder wins, so this is safe on every launch.
 * Reports failures rather than throwing, because the app can start without them.
 */
export function migrateLegacyData(legacyDir: string, dataDir: string): { copied: string[]; failed: string[] } {
  const copied: string[] = [];
  const failed: string[] = [];
  for (const file of USER_FILES) {
    const source = join(legacyDir, file);
    const target = join(dataDir, file);
    if (!existsSync(source) || existsSync(target)) continue;
    try {
      mkdirSync(dataDir, { recursive: true });
      copyFileSync(source, target, constants.COPYFILE_EXCL);
      copied.push(file);
    } catch {
      failed.push(file);
    }
  }
  return { copied, failed };
}

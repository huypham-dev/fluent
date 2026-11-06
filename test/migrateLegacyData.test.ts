import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { migrateLegacyData } from '../src/migrateLegacyData';

function dirs() {
  const root = mkdtempSync(join(tmpdir(), 'fluent-test-'));
  const legacyDir = join(root, 'legacy');
  const dataDir = join(root, 'data');
  mkdirSync(legacyDir);
  return { legacyDir, dataDir };
}

describe('migrateLegacyData', () => {
  it('copies settings and style into a data folder that does not exist yet', () => {
    const { legacyDir, dataDir } = dirs();
    writeFileSync(join(legacyDir, 'settings.json'), 'old settings');
    writeFileSync(join(legacyDir, 'style.md'), 'old style');

    const result = migrateLegacyData(legacyDir, dataDir);

    expect(readFileSync(join(dataDir, 'settings.json'), 'utf8')).toBe('old settings');
    expect(readFileSync(join(dataDir, 'style.md'), 'utf8')).toBe('old style');
    expect(result).toEqual({ copied: ['settings.json', 'style.md'], failed: [] });
  });

  it('keeps the legacy files in place', () => {
    const { legacyDir, dataDir } = dirs();
    writeFileSync(join(legacyDir, 'style.md'), 'old style');

    migrateLegacyData(legacyDir, dataDir);

    expect(existsSync(join(legacyDir, 'style.md'))).toBe(true);
  });

  it('does not overwrite files that already exist in the data folder', () => {
    const { legacyDir, dataDir } = dirs();
    mkdirSync(dataDir);
    writeFileSync(join(legacyDir, 'style.md'), 'old style');
    writeFileSync(join(dataDir, 'style.md'), 'new style');

    const result = migrateLegacyData(legacyDir, dataDir);

    expect(readFileSync(join(dataDir, 'style.md'), 'utf8')).toBe('new style');
    expect(result).toEqual({ copied: [], failed: [] });
  });

  it('does nothing when there is no legacy folder', () => {
    const { legacyDir, dataDir } = dirs();

    const result = migrateLegacyData(join(legacyDir, 'missing'), dataDir);

    expect(result).toEqual({ copied: [], failed: [] });
  });

  it('copies only settings and style', () => {
    const { legacyDir, dataDir } = dirs();
    writeFileSync(join(legacyDir, 'diagnostics.log'), 'old log');

    migrateLegacyData(legacyDir, dataDir);

    expect(existsSync(join(dataDir, 'diagnostics.log'))).toBe(false);
  });

  it('reports a failed copy instead of throwing, so the app can still start', () => {
    const { legacyDir, dataDir } = dirs();
    writeFileSync(dataDir, 'a file where the data folder should be');
    writeFileSync(join(legacyDir, 'style.md'), 'old style');

    const result = migrateLegacyData(legacyDir, dataDir);

    expect(result.copied).toEqual([]);
    expect(result.failed).toEqual(['style.md']);
  });
});

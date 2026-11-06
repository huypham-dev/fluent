import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import packageJson from '../package.json';

function pngInfo(path: string) {
  const file = readFileSync(path);
  return { width: file.readUInt32BE(16), height: file.readUInt32BE(20), colorType: file[25] };
}

function icoSizes(path: string) {
  const file = readFileSync(path);
  const count = file.readUInt16LE(4);
  return Array.from({ length: count }, (_, index) => file[6 + index * 16] || 256);
}

const RGBA = 6;

describe('tray icons', () => {
  it('has transparent macOS icons at 1x and 2x', () => {
    expect(pngInfo('static/tray.png')).toEqual({ width: 18, height: 18, colorType: RGBA });
    expect(pngInfo('static/tray@2x.png')).toEqual({ width: 36, height: 36, colorType: RGBA });
  });

  it('has a Windows icon for each common display scale', () => {
    expect(icoSizes('static/tray.ico')).toEqual([16, 20, 24, 32, 40, 48]);
  });

  it('ships with the app', () => {
    expect(packageJson.build.files).toContain('static/**');
  });
});

describe('app icons', () => {
  it('has a macOS icon where the build expects it', () => {
    expect(readFileSync(packageJson.build.mac.icon).subarray(0, 4).toString()).toBe('icns');
  });

  it('has a Windows icon up to 256 pixels where the build expects it', () => {
    expect(icoSizes(packageJson.build.win.icon)).toContain(256);
  });
});

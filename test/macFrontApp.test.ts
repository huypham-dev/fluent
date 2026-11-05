import { describe, expect, it } from 'vitest';
import { parseBundleId, shouldRefocus } from '../src/macFrontApp';

describe('parseBundleId', () => {
  it('reads the bundle ID from lsappinfo output', () => {
    expect(parseBundleId('"CFBundleIdentifier"="com.tinyspeck.slackmacgap"\n')).toBe('com.tinyspeck.slackmacgap');
  });

  it.each(['', 'kCFBundleIdentifier=NULL', '"CFBundleIdentifier"=[ NULL ]'])(
    'returns null for %j',
    (output) => {
      expect(parseBundleId(output)).toBeNull();
    },
  );
});

describe('shouldRefocus', () => {
  const own = 'local.textactions.app';

  it('refocuses when this app took focus from another app', () => {
    expect(shouldRefocus({ source: 'com.google.Chrome', current: own, own })).toBe(true);
  });

  it('does nothing when the source app still has focus', () => {
    expect(shouldRefocus({ source: 'com.google.Chrome', current: 'com.google.Chrome', own })).toBe(false);
  });

  it('does nothing when the source is unknown', () => {
    expect(shouldRefocus({ source: null, current: own, own })).toBe(false);
  });

  it('does nothing when the source is this app', () => {
    expect(shouldRefocus({ source: own, current: own, own })).toBe(false);
  });

  it('does not steal focus when the user moved to a third app', () => {
    expect(shouldRefocus({ source: 'com.google.Chrome', current: 'com.apple.TextEdit', own })).toBe(false);
  });
});

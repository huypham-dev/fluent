import { execFile, execFileSync } from 'node:child_process';

export function parseBundleId(lsappinfoOutput: string): string | null {
  return /"CFBundleIdentifier"="([^"]+)"/.exec(lsappinfoOutput)?.[1] ?? null;
}

/** True only when this app took focus from the app holding the selection. */
export function shouldRefocus(apps: { source: string | null; current: string | null; own: string }): boolean {
  return apps.source !== null && apps.source !== apps.own && apps.current === apps.own;
}

export function frontmostBundleId(): string | null {
  try {
    const asn = execFileSync('lsappinfo', ['front'], { encoding: 'utf8' }).trim();
    return parseBundleId(execFileSync('lsappinfo', ['info', '-only', 'bundleid', asn], { encoding: 'utf8' }));
  } catch {
    return null;
  }
}

/** Brings an app to the front and waits until macOS reports it there. */
export async function activateApp(bundleId: string, timeoutMs = 500, pollMs = 20): Promise<boolean> {
  execFile('open', ['-b', bundleId], () => {
    // The polling below decides success, so the exit status adds nothing.
  });
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (frontmostBundleId() === bundleId) return true;
    await new Promise((resolve) => setTimeout(resolve, pollMs));
  }
  return false;
}

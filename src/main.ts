import { randomUUID } from 'node:crypto';
import { homedir } from 'node:os';
import { join } from 'node:path';
import {
  app,
  BrowserWindow,
  clipboard,
  globalShortcut,
  ipcMain,
  Menu,
  nativeImage,
  nativeTheme,
  screen,
  shell,
  systemPreferences,
  Tray,
} from 'electron';
import { uIOhook, UiohookKey } from 'uiohook-napi';
import { ActionSession } from './actionSession';
import { ACTIONS, isActionId, type ActionId } from './actions';
import { barPosition } from './barPosition';
import { captureSelection } from './captureSelection';
import { defaultCliPath, runCli } from './cliGenerator';
import { logEvent, logPath } from './diagnostics';
import { electronClipboard } from './electronClipboard';
import { activateApp, frontmostBundleId, shouldRefocus } from './macFrontApp';
import { createSelectionDetector, type MouseInfo, type Point } from './selectionDetector';
import { HOTKEYS, loadSettings, MODELS, parseSettings, saveSettings, type Settings } from './settings';
import { StyleStore } from './styleStore';
import { HII_PATTERN, renderPattern } from './trayIcon';

const BAR_SIZE = { width: 132, height: 40 };
const BAR_TIMEOUT_MS = 4000;
const HOTKEY_KEY_GRACE_MS = 600;
const PERMISSION_POLL_MS = 2000;
const REFOCUS_SETTLE_MS = 80;
const COPY_RESEND_MS = 150;
const REFOCUSED_COPY_TIMEOUT_MS = 1000;
// An unpackaged run is the stock Electron app, so it has Electron's bundle id.
const ownBundleId = () => (app.isPackaged ? 'local.textactions.app' : 'com.github.Electron');
const isMac = process.platform === 'darwin';

type PopupView =
  | {
      kind: 'session';
      title: string;
      editable: boolean;
      inputLabel: string;
      state: ActionSession['state'];
    }
  | { kind: 'message'; title: string; detail: string; button?: 'accessibility' | 'settings' };

const settingsPath = () => join(app.getPath('userData'), 'settings.json');
const style = () => new StyleStore(join(app.getPath('userData'), 'style.md'));

const defaults: Settings = {
  hotkey: 'Control+Alt+R',
  model: 'sonnet',
  cliPath: defaultCliPath(process.platform, homedir()),
  primaryLanguage: 'Vietnamese',
  secondaryLanguage: 'English',
  autoBar: true,
  styleTemplateInstalled: false,
};

let settings = defaults;
let tray: Tray | null = null;
let bar: BrowserWindow | null = null;
let popup: BrowserWindow | null = null;
let popupReady = false;
let settingsWindow: BrowserWindow | null = null;
let session: ActionSession | null = null;
let view: PopupView | null = null;
let hideTimer: NodeJS.Timeout | null = null;
let sourceApp: string | null = null;
let ignoreKeysUntil = 0;
let hookRunning = false;
let capturing = false;

const webPreferences = {
  preload: join(__dirname, 'preload.js'),
  contextIsolation: true,
  sandbox: true,
  nodeIntegration: false,
};

function staticFile(name: string): string {
  return join(__dirname, '..', 'static', name);
}

function hasPermission(): boolean {
  return !isMac || systemPreferences.isTrustedAccessibilityClient(false);
}

/** Windows that show local pages must never navigate or open other content. */
function lockDown(window: BrowserWindow): void {
  window.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
  window.webContents.on('will-navigate', (event) => event.preventDefault());
}

// MARK: Icon bar

function isInside(window: BrowserWindow | null, point: Point): boolean {
  if (!window || window.isDestroyed() || !window.isVisible()) return false;
  const { x, y, width, height } = window.getBounds();
  return point.x >= x && point.x < x + width && point.y >= y && point.y < y + height;
}

const detector = createSelectionDetector({
  isOwnWindow: (point) => isInside(bar, point) || isInside(popup, point) || isInside(settingsWindow, point),
});

/** Hook positions are physical pixels on Windows and points on macOS. */
function toDip(point: Point): Point {
  return isMac ? point : screen.screenToDipPoint(point);
}

function mouseInfo(event: { x: number; y: number; button: unknown; clicks: number }): MouseInfo {
  return { ...toDip(event), button: Number(event.button), clicks: event.clicks };
}

function createBar(): BrowserWindow {
  const window = new BrowserWindow({
    ...BAR_SIZE,
    show: false,
    frame: false,
    resizable: false,
    movable: false,
    minimizable: false,
    maximizable: false,
    fullscreenable: false,
    focusable: false,
    skipTaskbar: true,
    alwaysOnTop: true,
    hasShadow: true,
    ...(isMac ? { type: 'panel' as const } : {}),
    webPreferences,
  });
  lockDown(window);
  window.setAlwaysOnTop(true, 'screen-saver');
  if (isMac) window.setVisibleOnAllWorkspaces(true, { visibleOnFullScreen: true });
  void window.loadFile(staticFile('bar.html'));
  return window;
}

function showBar(pointer: Point): void {
  if (!bar) return;
  sourceApp = isMac ? frontmostBundleId() : null;
  const { workArea } = screen.getDisplayNearestPoint(pointer);
  const { x, y } = barPosition(pointer, BAR_SIZE, workArea);
  bar.setPosition(Math.round(x), Math.round(y));
  bar.showInactive();
  logEvent('bar-shown');
  if (hideTimer) clearTimeout(hideTimer);
  hideTimer = setTimeout(hideBar, BAR_TIMEOUT_MS);
}

function hideBar(): void {
  if (hideTimer) clearTimeout(hideTimer);
  hideTimer = null;
  if (bar?.isVisible()) bar.hide();
}

function startHook(): void {
  uIOhook.on('mousedown', (event) => {
    const info = mouseInfo(event);
    detector.mouseDown(info);
    if (!isInside(bar, info)) hideBar();
  });
  uIOhook.on('mouseup', (event) => {
    const info = mouseInfo(event);
    if (detector.mouseUp(info) && settings.autoBar && !capturing) showBar(info);
  });
  uIOhook.on('keydown', () => {
    if (Date.now() >= ignoreKeysUntil) hideBar();
  });
  try {
    uIOhook.start();
    hookRunning = true;
    logEvent('hook-started');
  } catch (error) {
    logEvent('hook-failed', { error: String(error) });
    showMessage({
      kind: 'message',
      title: 'The icon bar is turned off',
      detail: 'TextActions could not monitor the mouse, which security software can block. You can still select text and press the hotkey.',
      button: 'settings',
    });
  }
}

function waitForPermissionThenStartHook(): void {
  if (hasPermission()) {
    startHook();
    return;
  }
  logEvent('permission-missing');
  systemPreferences.isTrustedAccessibilityClient(true);
  const timer = setInterval(() => {
    if (!hasPermission()) return;
    clearInterval(timer);
    logEvent('permission-granted');
    startHook();
  }, PERMISSION_POLL_MS);
}

// MARK: Running an action

function sendCopy(): void {
  uIOhook.keyTap(UiohookKey.C, [isMac ? UiohookKey.Meta : UiohookKey.Ctrl]);
}

/** Clicking the bar can make this app frontmost on macOS, which would send the copy here. */
async function returnFocusToSource(): Promise<boolean> {
  if (!isMac || !sourceApp) return false;
  const current = frontmostBundleId();
  const refocus = shouldRefocus({ source: sourceApp, current, own: ownBundleId() });
  logEvent('refocus-checked', { source: sourceApp, current: current ?? 'unknown', refocus });
  if (!refocus) return false;
  await activateApp(sourceApp);
  await new Promise((resolve) => setTimeout(resolve, REFOCUS_SETTLE_MS));
  return true;
}

async function runAction(action: ActionId): Promise<void> {
  logEvent('action-clicked', { action });
  hideBar();
  if (popup && session) {
    focusPopup();
    return;
  }
  if (capturing) return;
  if (!hasPermission()) {
    showMessage(permissionMessage());
    return;
  }

  capturing = true;
  try {
    const refocused = await returnFocusToSource();
    const startedAt = Date.now();
    let copySends = 0;
    const captured = await captureSelection({
      clipboard: electronClipboard,
      sendCopy: () => {
        copySends += 1;
        sendCopy();
      },
      marker: `textactions-${randomUUID()}`,
      // A reactivated app can report itself frontmost before it accepts the copy shortcut.
      ...(refocused ? { timeoutMs: REFOCUSED_COPY_TIMEOUT_MS, resendMs: COPY_RESEND_MS } : {}),
    });
    logEvent('capture-finished', {
      kind: captured.kind,
      length: captured.kind === 'text' ? captured.text.length : 0,
      copySends,
      ms: Date.now() - startedAt,
      front: frontmostBundleId() ?? 'unknown',
    });
    if (captured.kind === 'text') {
      startSession(action, captured.text);
    } else if (captured.kind === 'noSelection') {
      showMessage({ kind: 'message', title: 'No text selected', detail: 'Select some text, then try again.' });
    } else {
      showMessage({ kind: 'message', title: 'The selection cannot be read', detail: 'The copied content has no text.' });
    }
  } catch (error) {
    logEvent('capture-failed', { error: String(error) });
    showMessage({ kind: 'message', title: 'The selection could not be copied', detail: String(error) });
  } finally {
    capturing = false;
  }
}

function startSession(action: ActionId, selectedText: string): void {
  const { title, editable, inputLabel } = ACTIONS[action];
  const current = new ActionSession({
    action,
    selectedText,
    languages: settings,
    loadStyle: () => style().load(),
    generate: (request, onText) =>
      runCli(
        { executable: settings.cliPath, model: settings.model, cwd: app.getPath('userData'), ...request },
        onText,
      ),
  });
  session = current;
  current.onChange = (state) => {
    if (session !== current) return;
    view = { kind: 'session', title, editable, inputLabel, state };
    sendView();
  };
  view = { kind: 'session', title, editable, inputLabel, state: current.state };
  openPopup();
  current.start();
}

// MARK: Popup

function permissionMessage(): PopupView {
  return {
    kind: 'message',
    title: 'TextActions needs Accessibility access',
    detail: 'It uses this to detect selections and copy your selected text. Turn on TextActions in System Settings, then try again.',
    button: 'accessibility',
  };
}

function showMessage(message: PopupView): void {
  if (popup && session) return;
  view = message;
  openPopup();
}

function focusPopup(): void {
  if (!popup) return;
  app.focus({ steal: true });
  popup.show();
  popup.focus();
}

function sendView(): void {
  // The renderer asks for the current view once it is listening, so earlier sends can be skipped.
  if (popup && popupReady) popup.webContents.send('popup:view', view);
}

function openPopup(): void {
  if (popup) {
    sendView();
    focusPopup();
    return;
  }
  const window = new BrowserWindow({
    width: 500,
    height: 340,
    minWidth: 380,
    minHeight: 240,
    show: false,
    title: 'TextActions',
    alwaysOnTop: true,
    webPreferences,
  });
  lockDown(window);
  window.removeMenu();
  popup = window;
  popupReady = false;
  window.on('closed', () => {
    if (popup !== window) return;
    popup = null;
    popupReady = false;
    view = null;
    session?.cancel();
    session = null;
    void returnFocusToSource();
  });
  window.webContents.once('did-finish-load', () => {
    window.show();
    window.focus();
  });
  void window.loadFile(staticFile('popup.html'));
}

// MARK: Settings

function applyHotkey(): void {
  globalShortcut.unregisterAll();
  let registered = false;
  try {
    registered = globalShortcut.register(settings.hotkey, () => {
      ignoreKeysUntil = Date.now() + HOTKEY_KEY_GRACE_MS;
      if (!hasPermission()) {
        showMessage(permissionMessage());
        return;
      }
      showBar(screen.getCursorScreenPoint());
    });
  } catch (error) {
    logEvent('hotkey-error', { error: String(error) });
  }
  logEvent('hotkey-registered', { hotkey: settings.hotkey, registered });
  if (!registered) {
    showMessage({
      kind: 'message',
      title: 'The hotkey is not available',
      detail: `Another app may be using ${settings.hotkey}. Choose a different hotkey in Settings.`,
      button: 'settings',
    });
  }
}

function openSettings(): void {
  if (settingsWindow) {
    app.focus({ steal: true });
    settingsWindow.focus();
    return;
  }
  const window = new BrowserWindow({
    width: 460,
    height: 460,
    resizable: false,
    title: 'TextActions Settings',
    alwaysOnTop: true,
    webPreferences,
  });
  lockDown(window);
  window.removeMenu();
  settingsWindow = window;
  window.on('closed', () => {
    settingsWindow = null;
  });
  void window.loadFile(staticFile('settings.html'));
  app.focus({ steal: true });
}

function editStyle(): void {
  const store = style();
  store.installTemplate();
  void shell.openPath(store.path);
}

// MARK: App setup

function registerIpc(): void {
  ipcMain.on('bar:run', (_event, action: unknown) => {
    if (isActionId(action)) void runAction(action);
  });
  ipcMain.on('popup:ready', (event) => {
    if (event.sender !== popup?.webContents) return;
    popupReady = true;
    sendView();
  });
  ipcMain.on('popup:edit', (_event, text: unknown) => {
    if (typeof text === 'string') session?.setResult(text);
  });
  ipcMain.on('popup:submit', (_event, text: unknown) => {
    if (typeof text === 'string') session?.submitInput(text);
  });
  ipcMain.on('popup:retry', () => session?.retry());
  ipcMain.on('popup:copy', (_event, text: unknown) => {
    if (typeof text !== 'string' || !text.trim() || session?.state.phase === 'generating') return;
    clipboard.writeText(text);
    popup?.close();
  });
  ipcMain.on('popup:close', () => popup?.close());
  ipcMain.on('popup:open-settings', () => openSettings());
  ipcMain.on('popup:open-accessibility', () => {
    void shell.openExternal('x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility');
  });
  ipcMain.on('settings:edit-style', () => editStyle());
  ipcMain.handle('settings:get', () => ({ settings, hotkeys: HOTKEYS, models: MODELS, hookRunning }));
  ipcMain.handle('settings:save', (_event, raw: unknown) => {
    const previous = settings.hotkey;
    settings = parseSettings({ ...(raw as object), styleTemplateInstalled: settings.styleTemplateInstalled }, settings);
    saveSettings(settingsPath(), settings);
    if (settings.hotkey !== previous) applyHotkey();
    return settings;
  });
}

function createTray(): void {
  // macOS recolors template images to suit the menu bar, so only other systems pick a color.
  const light = !isMac && nativeTheme.shouldUseDarkColors;
  const { bitmap, size } = renderPattern(HII_PATTERN, {
    canvas: 16,
    scale: 2,
    color: light ? [255, 255, 255] : [0, 0, 0],
  });
  const icon = nativeImage.createFromBitmap(bitmap, { width: size, height: size, scaleFactor: 2 });
  if (isMac) icon.setTemplateImage(true);
  tray = new Tray(icon);
  tray.setToolTip('TextActions');
  tray.setContextMenu(
    Menu.buildFromTemplate([
      { label: 'Settings…', click: openSettings },
      { label: 'Edit Style…', click: editStyle },
      { label: 'Open Diagnostic Log', click: () => void shell.openPath(logPath()) },
      { type: 'separator' },
      { label: 'Quit TextActions', click: () => app.quit() },
    ]),
  );
}

function installStyleTemplateOnce(): void {
  if (settings.styleTemplateInstalled) return;
  try {
    style().installTemplate();
    settings = { ...settings, styleTemplateInstalled: true };
    saveSettings(settingsPath(), settings);
  } catch (error) {
    logEvent('style-template-failed', { error: String(error) });
  }
}

if (!app.requestSingleInstanceLock()) {
  app.quit();
} else {
  if (isMac) app.dock?.hide();
  app.on('window-all-closed', () => {
    // A tray app keeps running with no windows.
  });
  app.on('will-quit', () => {
    globalShortcut.unregisterAll();
    if (hookRunning) uIOhook.stop();
  });
  registerIpc();

  void app.whenReady().then(() => {
    settings = loadSettings(settingsPath(), defaults);
    logEvent('started', { platform: process.platform, arch: process.arch, version: app.getVersion() });
    // On macOS, text fields get their editing shortcuts from the main menu.
    Menu.setApplicationMenu(isMac ? Menu.buildFromTemplate([{ role: 'editMenu' }]) : null);
    installStyleTemplateOnce();
    createTray();
    bar = createBar();
    waitForPermissionThenStartHook();
    applyHotkey();
  });
}

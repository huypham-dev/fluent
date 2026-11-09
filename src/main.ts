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
  screen,
  shell,
  systemPreferences,
  Tray,
} from 'electron';
import { uIOhook, UiohookKey } from 'uiohook-napi';
import { ActionSession } from './actionSession';
import { ACTIONS, isActionId, modelFor, type ActionId } from './actions';
import { barPosition } from './barPosition';
import { POPUP_WINDOW, popupPlacement, selectionAnchor, type PopupPlacement } from './popupPosition';
import { captureSelection } from './captureSelection';
import { defaultCliPath, runCli } from './cliGenerator';
import { logEvent, logPath } from './diagnostics';
import { electronClipboard } from './electronClipboard';
import { createHeldKeys } from './heldKeys';
import { activateApp, frontmostBundleId, shouldRefocus } from './macFrontApp';
import { LEGACY_DATA_FOLDER, migrateLegacyData } from './migrateLegacyData';
import { createSelectionDetector, type MouseInfo, type Point } from './selectionDetector';
import { HOTKEYS, loadSettings, MODELS, parseSettings, saveSettings, type Settings } from './settings';
import { StyleStore } from './styleStore';

const BAR_SIZE = { width: 132, height: 40 };
const COPIED_CLOSE_DELAY_MS = 450;
const BAR_TIMEOUT_MS = 4000;
const HOTKEY_KEY_GRACE_MS = 600;
const HOTKEY_RELEASE_TIMEOUT_MS = 1000;
const PERMISSION_POLL_MS = 2000;
const REFOCUS_SETTLE_MS = 80;
const COPY_RESEND_MS = 150;
const REFOCUSED_COPY_TIMEOUT_MS = 1000;
// An unpackaged run is the stock Electron app, so it has Electron's bundle id.
const ownBundleId = () => (app.isPackaged ? 'local.fluent.app' : 'com.github.Electron');
const isMac = process.platform === 'darwin';

type PopupView =
  | {
      kind: 'session';
      action: ActionId;
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
/** Where the last mouse press started, to find the middle of a dragged selection. */
let pressPoint: Point | null = null;
/** Where the selection the bar acts on is on screen; the popup points at it. */
let selectionPoint: Point | null = null;
let placement: PopupPlacement | null = null;
/** An outside click already moved focus where the user wanted it. */
let refocusOnClose = true;

const heldModifiers = createHeldKeys([
  UiohookKey.Ctrl,
  UiohookKey.CtrlRight,
  UiohookKey.Alt,
  UiohookKey.AltRight,
  UiohookKey.Shift,
  UiohookKey.ShiftRight,
  UiohookKey.Meta,
  UiohookKey.MetaRight,
]);

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

function showBar(pointer: Point, anchor: Point = pointer): void {
  if (!bar) return;
  selectionPoint = anchor;
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
    pressPoint = info;
    if (!isInside(bar, info)) hideBar();
    closePopupOnOutsideClick(info);
  });
  uIOhook.on('mouseup', (event) => {
    const info = mouseInfo(event);
    if (detector.mouseUp(info) && settings.autoBar && !capturing) showBar(info, selectionAnchor(pressPoint, info));
  });
  uIOhook.on('keydown', (event) => {
    heldModifiers.keyDown(event.keycode);
    if (Date.now() >= ignoreKeysUntil) hideBar();
  });
  uIOhook.on('keyup', (event) => heldModifiers.keyUp(event.keycode));
  try {
    uIOhook.start();
    hookRunning = true;
    logEvent('hook-started');
  } catch (error) {
    logEvent('hook-failed', { error: String(error) });
    showMessage({
      kind: 'message',
      title: 'The icon bar is turned off',
      detail: 'Fluent could not monitor the mouse, which security software can block. You can still select text and press the hotkey.',
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
      marker: `fluent-${randomUUID()}`,
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
    } else if (captured.kind === 'noText') {
      showMessage(
        { kind: 'message', title: 'The selection cannot be read', detail: 'The copied content has no text.' },
        selectionPoint,
      );
    }
  } catch (error) {
    logEvent('capture-failed', { error: String(error) });
    showMessage({ kind: 'message', title: 'The selection could not be copied', detail: String(error) }, selectionPoint);
  } finally {
    capturing = false;
  }
}

/**
 * The hotkey is pressed on purpose, so it can afford a copy to check for a selection first.
 * On Windows that copy is Ctrl+C, which would interrupt a command in a terminal.
 * If the check cannot run, the bar shows, and choosing an action checks again.
 */
async function showBarForHotkey(): Promise<void> {
  if (!isMac || !hookRunning || popup) {
    showBar(screen.getCursorScreenPoint());
    return;
  }
  if (capturing) return;

  capturing = true;
  let show = true;
  try {
    // A held Control or Alt would turn the simulated copy into a different shortcut.
    if (await heldModifiers.whenReleased(HOTKEY_RELEASE_TIMEOUT_MS)) {
      // The hook also sees the simulated copy, which must not hide the bar.
      ignoreKeysUntil = Date.now() + HOTKEY_KEY_GRACE_MS;
      const captured = await captureSelection({
        clipboard: electronClipboard,
        sendCopy,
        marker: `fluent-${randomUUID()}`,
      });
      logEvent('hotkey-capture-finished', { kind: captured.kind });
      show = captured.kind === 'text';
    } else {
      logEvent('hotkey-modifiers-held');
    }
  } catch (error) {
    logEvent('hotkey-capture-failed', { error: String(error) });
  } finally {
    capturing = false;
  }
  if (show) showBar(screen.getCursorScreenPoint());
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
        { executable: settings.cliPath, model: modelFor(action, settings.model), cwd: app.getPath('userData'), ...request },
        onText,
      ),
  });
  session = current;
  current.onChange = (state) => {
    if (session !== current) return;
    view = { kind: 'session', action, title, editable, inputLabel, state };
    sendView();
  };
  view = { kind: 'session', action, title, editable, inputLabel, state: current.state };
  openPopup(selectionPoint);
  current.start();
}

// MARK: Popup

function permissionMessage(): PopupView {
  return {
    kind: 'message',
    title: 'Fluent needs Accessibility access',
    detail: 'It uses this to detect selections and copy your selected text. Turn on Fluent in System Settings, then try again.',
    button: 'accessibility',
  };
}

function showMessage(message: PopupView, anchor: Point | null = null): void {
  if (popup && session) return;
  view = message;
  openPopup(anchor);
}

/** Like a page's translate bubble, a click anywhere else dismisses the popup. */
function closePopupOnOutsideClick(point: Point): void {
  if (!popup || isInside(popup, point) || isInside(bar, point) || isInside(settingsWindow, point)) return;
  logEvent('popup-dismissed');
  refocusOnClose = false;
  popup.close();
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

function sendPlacement(): void {
  if (popup && popupReady) popup.webContents.send('popup:placement', placement && { side: placement.side, arrowX: placement.arrowX });
}

/** Points the popup at the selection, or centers it without an arrow when there is none. */
function placePopup(window: BrowserWindow, anchor: Point | null): void {
  const point = anchor ?? screen.getCursorScreenPoint();
  const { workArea } = screen.getDisplayNearestPoint(point);
  placement = anchor ? popupPlacement(anchor, workArea) : null;
  const x = placement?.x ?? workArea.x + (workArea.width - POPUP_WINDOW.width) / 2;
  const y = placement?.y ?? workArea.y + (workArea.height - POPUP_WINDOW.height) / 2;
  window.setPosition(Math.round(x), Math.round(y));
}

function openPopup(anchor: Point | null = null): void {
  if (popup) {
    sendView();
    focusPopup();
    return;
  }
  const window = new BrowserWindow({
    ...POPUP_WINDOW,
    frame: false,
    show: false,
    resizable: false,
    minimizable: false,
    maximizable: false,
    fullscreenable: false,
    // The page draws the card, its arrow and its shadow on a see-through window.
    transparent: true,
    backgroundColor: '#00000000',
    hasShadow: false,
    title: 'Fluent',
    alwaysOnTop: true,
    webPreferences,
  });
  placePopup(window, anchor);
  refocusOnClose = true;
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
    placement = null;
    if (refocusOnClose) void returnFocusToSource();
  });
  // Once dragged away, the arrow no longer points at the selection.
  window.on('will-move', () => {
    if (!placement) return;
    placement = null;
    sendPlacement();
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
      void showBarForHotkey();
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
    width: 500,
    height: 670,
    show: false,
    resizable: false,
    title: 'Fluent Settings',
    alwaysOnTop: true,
    webPreferences,
  });
  lockDown(window);
  window.removeMenu();
  settingsWindow = window;
  window.on('closed', () => {
    settingsWindow = null;
  });
  // Showing it only once painted avoids a white flash in dark mode.
  window.once('ready-to-show', () => {
    window.show();
    app.focus({ steal: true });
  });
  void window.loadFile(staticFile('settings.html'));
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
    sendPlacement();
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
    // Leave the "Copied" confirmation on screen briefly before closing.
    const copiedSession = session;
    setTimeout(() => {
      if (session === copiedSession) popup?.close();
    }, COPIED_CLOSE_DELAY_MS);
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
  // tray.png has a tray@2x.png beside it, which Electron picks up for Retina displays.
  const icon = nativeImage.createFromPath(staticFile(isMac ? 'tray.png' : 'tray.ico'));
  tray = new Tray(icon);
  tray.setToolTip('Fluent');
  tray.setContextMenu(
    Menu.buildFromTemplate([
      { label: 'Settings…', click: openSettings },
      { label: 'Edit Style…', click: editStyle },
      { label: 'Open Diagnostic Log', click: () => void shell.openPath(logPath()) },
      { type: 'separator' },
      { label: 'Quit Fluent', click: () => app.quit() },
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
    const migration = migrateLegacyData(join(app.getPath('appData'), LEGACY_DATA_FOLDER), app.getPath('userData'));
    settings = loadSettings(settingsPath(), defaults);
    logEvent('started', { platform: process.platform, arch: process.arch, version: app.getVersion() });
    if (migration.copied.length || migration.failed.length) {
      logEvent('legacy-data-migrated', { copied: migration.copied.join(','), failed: migration.failed.join(',') });
    }
    // On macOS, text fields get their editing shortcuts from the main menu.
    Menu.setApplicationMenu(isMac ? Menu.buildFromTemplate([{ role: 'editMenu' }]) : null);
    installStyleTemplateOnce();
    createTray();
    bar = createBar();
    waitForPermissionThenStartHook();
    applyHotkey();
  });
}

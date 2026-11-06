# TextActions

A tray app for macOS and Windows that acts on selected text in any app. Select text,
choose an action, and read the result in a popup. The app never pastes or sends
anything.

| Action | Result |
|---|---|
| Draft reply | A reply in the language of the selected text, using your style file. You can edit it. |
| Explain | An explanation in your primary language. It is read-only, and you can ask follow-up questions. |
| Translate | Primary language to secondary language; any other language to primary. You can edit it. |

Selected text is sent to Anthropic through the Claude Code CLI, only after you choose
an action. The app stores no selected text or results.

## Requirements

- macOS or Windows
- Claude Code CLI, logged in. The path can be changed in settings. The default path is:
  - macOS: `~/.local/bin/claude`
  - Windows: `%USERPROFILE%\.local\bin\claude.exe`

## Download and install

### Install with one command

On macOS, paste this into Terminal:

```bash
curl -fsSL https://raw.githubusercontent.com/huypham-dev/text-actions/main/install.sh | bash
```

On Windows, paste this into PowerShell:

```powershell
irm https://raw.githubusercontent.com/huypham-dev/text-actions/main/install.ps1 | iex
```

The command downloads the latest release for your computer, installs it, and opens the
app. To see what it runs first, read [install.sh](install.sh) or [install.ps1](install.ps1).
On macOS, you still turn on TextActions under System Settings > Privacy & Security >
Accessibility when the app asks.

### Download an installer

Or download the installer for your computer:

| Computer | Download |
|---|---|
| Mac with Apple silicon (M1 or later) | [TextActions-0.0.1-arm64.dmg](https://github.com/huypham-dev/text-actions/releases/download/v0.0.1/TextActions-0.0.1-arm64.dmg) |
| Mac with an Intel processor | [TextActions-0.0.1.dmg](https://github.com/huypham-dev/text-actions/releases/download/v0.0.1/TextActions-0.0.1.dmg) |
| Windows (64-bit) | [TextActions.Setup.0.0.1.exe](https://github.com/huypham-dev/text-actions/releases/download/v0.0.1/TextActions.Setup.0.0.1.exe) |

Newer versions, if any, are on the [latest release](https://github.com/huypham-dev/text-actions/releases/latest) page.

To check which Mac you have, open the Apple menu and choose **About This Mac**.

The installers are not signed, so your computer shows a warning the first time.

### macOS

1. Open the `.dmg` file and drag TextActions to Applications.
2. Open TextActions from Applications. If macOS blocks it, go to System Settings >
   Privacy & Security and choose **Open Anyway**.
3. When the app asks, turn on TextActions under System Settings > Privacy & Security >
   Accessibility.

The app runs from the menu bar and has no Dock icon. If macOS asks for the Accessibility
permission again after an update, turn it off and on again.

### Windows

1. Run the `.exe` file. If SmartScreen appears, choose **More info**, then **Run anyway**.
2. The app installs and opens by itself, and runs from the system tray.

Security software may warn that the app monitors mouse input, because this is how it
detects a selection.

### Updates

The app does not update itself. To update, run the install command again, or download
and install the latest release.

## Build from source

Building requires Node.js and npm:

```bash
npm install
```

```bash
npm start
```

Build an installer into `release/`:

```bash
npm run dist:mac
```

```bash
npm run dist:win
```

When you run the app with `npm start` on macOS, the Accessibility permission may apply
to your terminal, and the app is listed as Electron rather than TextActions.

## Using the app

After you select text with the mouse, an icon bar appears near the pointer. After you
select text with the keyboard, press the hotkey instead. The default hotkey is
`Control+Alt+R`.

The hotkey, model, CLI path, languages, and automatic icon bar can be changed under
**Settings…** in the tray menu.

## Your style

Choose **Edit Style…** from the tray menu. The file is read for every Draft reply and
is not used by other actions. It is at:

- macOS: `~/Library/Application Support/TextActions/style.md`
- Windows: `%APPDATA%\TextActions\style.md`

## Keys in the popup

| Key | In the result | In the input field |
|---|---|---|
| Enter | Copy and close (Draft reply and Translate) | Send the rewrite instruction or question |
| Shift+Enter | New line | |
| Esc | Close | Close |

## Tests

```bash
npm test
```

```bash
npm run typecheck
```

This command calls the real CLI and uses real usage:

```bash
TEXTACTIONS_LIVE=1 npx vitest run test/live
```

## Manual checklist

Run these on each OS, in Slack, Chrome, Mail or Outlook, VS Code, and Word:

- [ ] Clicking an icon keeps focus in the source app.
- [ ] Simulated copy reaches the source app.
- [ ] The original clipboard is restored after capture and after Esc.
- [ ] Focus returns to the source app when the popup closes.
- [ ] Enter and Esc while composing Vietnamese text do not copy or close.
- [ ] The bar is placed correctly on a second display and at different scale factors.
- [ ] Selecting text in a terminal does not interrupt a running command.

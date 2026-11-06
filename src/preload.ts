import { contextBridge, ipcRenderer } from 'electron';

contextBridge.exposeInMainWorld('fluent', {
  runAction: (action: string) => ipcRenderer.send('bar:run', action),

  onView: (callback: (view: unknown) => void) => {
    ipcRenderer.on('popup:view', (_event, view) => callback(view));
    ipcRenderer.send('popup:ready');
  },
  edit: (text: string) => ipcRenderer.send('popup:edit', text),
  submit: (text: string) => ipcRenderer.send('popup:submit', text),
  retry: () => ipcRenderer.send('popup:retry'),
  copy: (text: string) => ipcRenderer.send('popup:copy', text),
  close: () => ipcRenderer.send('popup:close'),
  openSettings: () => ipcRenderer.send('popup:open-settings'),
  openAccessibility: () => ipcRenderer.send('popup:open-accessibility'),

  getSettings: () => ipcRenderer.invoke('settings:get'),
  saveSettings: (settings: unknown) => ipcRenderer.invoke('settings:save', settings),
  editStyle: () => ipcRenderer.send('settings:edit-style'),
});

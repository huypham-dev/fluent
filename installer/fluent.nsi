; Per-user Windows installer for Fluent. Build with:
;   makensis /DVERSION=X.Y.Z installer\fluent.nsi
; It installs without questions, like the Electron one-click installer it replaces.

Unicode true
!ifndef VERSION
  !error "Pass the version: makensis /DVERSION=X.Y.Z"
!endif

!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\Fluent"

Name "Fluent"
OutFile "..\release\Fluent.Setup.${VERSION}.exe"
Icon "..\build\icon.ico"
UninstallIcon "..\build\icon.ico"
RequestExecutionLevel user
; The folder the Electron installer used, so an update replaces it.
InstallDir "$LOCALAPPDATA\Programs\fluent"
ShowInstDetails nevershow
AutoCloseWindow true

Section
  nsExec::Exec 'taskkill /IM Fluent.exe /F'
  Sleep 500

  ; An Electron install of Fluent leaves Chromium beside the app; its own uninstaller removes it.
  ; It keeps the settings and style in %APPDATA%\Fluent.
  IfFileExists "$INSTDIR\resources\app.asar" 0 +3
    ExecWait '"$INSTDIR\Uninstall Fluent.exe" /S _?=$INSTDIR'
    Delete "$INSTDIR\Uninstall Fluent.exe"

  SetOutPath "$INSTDIR"
  File "/oname=Fluent.exe" "..\target\x86_64-pc-windows-msvc\release\fluent.exe"
  WriteUninstaller "$INSTDIR\Uninstall Fluent.exe"
  CreateShortcut "$SMPROGRAMS\Fluent.lnk" "$INSTDIR\Fluent.exe"

  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "Fluent"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "Fluent"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\Fluent.exe"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\Uninstall Fluent.exe"'
  WriteRegStr HKCU "${UNINSTALL_KEY}" "QuietUninstallString" '"$INSTDIR\Uninstall Fluent.exe" /S'
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1

  Exec '"$INSTDIR\Fluent.exe"'
SectionEnd

Section "Uninstall"
  nsExec::Exec 'taskkill /IM Fluent.exe /F'
  Sleep 500
  Delete "$INSTDIR\Fluent.exe"
  Delete "$INSTDIR\Uninstall Fluent.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\Fluent.lnk"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
SectionEnd

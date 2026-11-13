; KohdaLab Camera Windows installer.
; The build script supplies explicit File/Delete includes after validating the payload.
; This installer deliberately does not install drivers, services, startup tasks, or policy changes.

Unicode true
RequestExecutionLevel admin
SetCompressor /SOLID zlib
SetDatablockOptimize on

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "x64.nsh"

!ifndef OUTPUT_FILE
  !error "OUTPUT_FILE must be supplied by build_windows_installer.py"
!endif
!ifndef PRODUCT_VERSION
  !error "PRODUCT_VERSION must be supplied by build_windows_installer.py"
!endif
!ifndef PRODUCT_VERSION_QUAD
  !error "PRODUCT_VERSION_QUAD must be supplied by build_windows_installer.py"
!endif
!ifndef PAYLOAD_STAGE_INCLUDE
  !error "PAYLOAD_STAGE_INCLUDE must be supplied by build_windows_installer.py"
!endif
!ifndef PAYLOAD_COMMIT_INCLUDE
  !error "PAYLOAD_COMMIT_INCLUDE must be supplied by build_windows_installer.py"
!endif
!ifndef PAYLOAD_ROLLBACK_INCLUDE
  !error "PAYLOAD_ROLLBACK_INCLUDE must be supplied by build_windows_installer.py"
!endif
!ifndef PAYLOAD_CLEANUP_BACKUP_INCLUDE
  !error "PAYLOAD_CLEANUP_BACKUP_INCLUDE must be supplied by build_windows_installer.py"
!endif
!ifndef PAYLOAD_UNINSTALL_INCLUDE
  !error "PAYLOAD_UNINSTALL_INCLUDE must be supplied by build_windows_installer.py"
!endif
!ifndef ICON_FILE
  !error "ICON_FILE must be supplied by build_windows_installer.py"
!endif

!define PRODUCT_NAME "KohdaLab Camera"
!define PRODUCT_PUBLISHER "KohdaLab"
!define PRODUCT_REGKEY "Software\KohdaLab\Camera"
!define UNINSTALL_REGKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\KohdaLab Camera"
!define MAIN_EXE "KohdaLab Camera.exe"
!define HELPER_EXE "continuous-helper.exe"

Name "${PRODUCT_NAME} ${PRODUCT_VERSION}"
OutFile "${OUTPUT_FILE}"
InstallDir "$PROGRAMFILES64\Kohdalab\KohdaLab Camera"
Icon "${ICON_FILE}"
UninstallIcon "${ICON_FILE}"
BrandingText "KohdaLab Camera"
ShowInstDetails show
ShowUninstDetails show

VIProductVersion "${PRODUCT_VERSION_QUAD}"
VIAddVersionKey /LANG=1033 "ProductName" "${PRODUCT_NAME}"
VIAddVersionKey /LANG=1033 "ProductVersion" "${PRODUCT_VERSION}"
VIAddVersionKey /LANG=1033 "FileVersion" "${PRODUCT_VERSION}"
VIAddVersionKey /LANG=1033 "CompanyName" "${PRODUCT_PUBLISHER}"
VIAddVersionKey /LANG=1033 "FileDescription" "KohdaLab Camera installer"
VIAddVersionKey /LANG=1033 "OriginalFilename" "KohdaLab-Camera-Setup.exe"
VIAddVersionKey /LANG=1033 "LegalCopyright" "MIT licensed; see accompanying license notices"

; Export an exact unsigned uninstaller without running Setup on a build runner.
; A provider-approved signing/import stage is intentionally not enabled yet.
!ifdef EXPORT_UNINSTALLER
  !uninstfinalize 'cmd /C copy /B /Y "%1" "${EXPORT_UNINSTALLER}" >nul' = 0
!endif

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_WELCOME
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP|MB_OK "KohdaLab Camera requires 64-bit Windows."
    Abort
  ${EndIf}
  SetRegView 64
  SetShellVarContext current
  ; Earlier releases installed for the current user. Require normal removal
  ; before creating a machine installation and common shortcuts.
  ReadRegStr $0 HKCU "${PRODUCT_REGKEY}" "InstallDir"
  ${If} $0 != ""
    Goto legacy_install_present
  ${EndIf}
  IfFileExists "$LOCALAPPDATA\Programs\KohdaLab Camera\${MAIN_EXE}" legacy_install_present legacy_install_clear
legacy_install_present:
  MessageBox MB_ICONSTOP|MB_OK "An earlier per-user KohdaLab Camera installation is present. Close the application, uninstall that installation from Windows Settings, then run this Setup again. Saved settings and images are preserved." /SD IDOK
  SetErrorLevel 1
  Quit
legacy_install_clear:
  SetShellVarContext all
  ; Keep this product's program files separate from every other KohdaLab app.
  ; Ignore caller-supplied /D paths for a machine installation.
  StrCpy $INSTDIR "$PROGRAMFILES64\Kohdalab\KohdaLab Camera"
FunctionEnd

; This is intentionally a conservative preflight, rather than process enumeration.
; Exclusive access fails when Windows has either installed runtime binary open. A
; process that starts after this check will make the later File command fail safely.
Function CheckInstalledRuntimeIsClosed
  IfFileExists "$INSTDIR\${MAIN_EXE}" 0 check_helper
  System::Call 'kernel32::CreateFileW(w "$INSTDIR\${MAIN_EXE}", i 0xC0000000, i 0, p 0, i 3, i 0, p 0) p.r0'
  ${If} $0 == -1
    MessageBox MB_ICONSTOP|MB_OK "KohdaLab Camera is in use. Close it before installing or updating."
    Abort
  ${EndIf}
  System::Call 'kernel32::CloseHandle(p r0)'
check_helper:
  IfFileExists "$INSTDIR\${HELPER_EXE}" 0 done
  System::Call 'kernel32::CreateFileW(w "$INSTDIR\${HELPER_EXE}", i 0xC0000000, i 0, p 0, i 3, i 0, p 0) p.r0'
  ${If} $0 == -1
    MessageBox MB_ICONSTOP|MB_OK "KohdaLab Camera helper is in use. Close it before installing or updating."
    Abort
  ${EndIf}
  System::Call 'kernel32::CloseHandle(p r0)'
done:
FunctionEnd

Section "KohdaLab Camera" SEC_APP
  SetShellVarContext all
  SetRegView 64
  Call CheckInstalledRuntimeIsClosed
  IfFileExists "$INSTDIR.install-backup" payload_backup_present
  Goto payload_backup_clear
payload_backup_present:
  MessageBox MB_ICONSTOP|MB_OK "A previous KohdaLab Camera update left a recovery backup at $INSTDIR.install-backup. Review or recover it before installing again."
  Abort
payload_backup_clear:
  InitPluginsDir
  ClearErrors
  !include "${PAYLOAD_STAGE_INCLUDE}"
  ${If} ${Errors}
    Goto payload_stage_failed
  ${EndIf}
  ClearErrors
  !include "${PAYLOAD_COMMIT_INCLUDE}"
  Goto payload_transaction_complete
payload_stage_failed:
  MessageBox MB_ICONSTOP|MB_OK "The installation could not prepare its temporary files. Existing program files were not changed. No camera was started."
  Abort
payload_transaction_failed:
  !include "${PAYLOAD_ROLLBACK_INCLUDE}"
  MessageBox MB_ICONSTOP|MB_OK "The installation could not update all program files. The previous files were restored where Windows allowed it. Any file Windows could not restore remains at $INSTDIR.install-backup. No camera was started."
  Abort
payload_transaction_complete:
  !include "${PAYLOAD_CLEANUP_BACKUP_INCLUDE}"
  WriteUninstaller "$INSTDIR\Uninstall KohdaLab Camera.exe"
  WriteRegStr HKLM "${PRODUCT_REGKEY}" "InstallDir" "$INSTDIR"
  WriteRegStr HKLM "${PRODUCT_REGKEY}" "Version" "${PRODUCT_VERSION}"
  WriteRegStr HKLM "${UNINSTALL_REGKEY}" "DisplayName" "${PRODUCT_NAME}"
  WriteRegStr HKLM "${UNINSTALL_REGKEY}" "DisplayVersion" "${PRODUCT_VERSION}"
  WriteRegStr HKLM "${UNINSTALL_REGKEY}" "Publisher" "${PRODUCT_PUBLISHER}"
  WriteRegStr HKLM "${UNINSTALL_REGKEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "${UNINSTALL_REGKEY}" "DisplayIcon" "$INSTDIR\${MAIN_EXE}"
  WriteRegStr HKLM "${UNINSTALL_REGKEY}" "UninstallString" '"$INSTDIR\Uninstall KohdaLab Camera.exe"'
  WriteRegDWORD HKLM "${UNINSTALL_REGKEY}" "NoModify" 1
  WriteRegDWORD HKLM "${UNINSTALL_REGKEY}" "NoRepair" 1
  CreateDirectory "$SMPROGRAMS\KohdaLab Camera"
  CreateShortcut "$SMPROGRAMS\KohdaLab Camera\KohdaLab Camera.lnk" "$INSTDIR\${MAIN_EXE}"
  CreateShortcut "$DESKTOP\KohdaLab Camera.lnk" "$INSTDIR\${MAIN_EXE}"
SectionEnd

Function un.onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP|MB_OK "KohdaLab Camera requires 64-bit Windows."
    Abort
  ${EndIf}
  SetRegView 64
  SetShellVarContext all
  ; A temporary uninstaller retains its original install path. Refuse a path
  ; supplied by another product or a caller.
  StrCpy $0 $INSTDIR
  StrCpy $INSTDIR "$PROGRAMFILES64\Kohdalab\KohdaLab Camera"
  ${If} $0 != ""
  ${AndIf} $0 != $INSTDIR
    MessageBox MB_ICONSTOP|MB_OK "KohdaLab Camera can only be removed from its installed location."
    Abort
  ${EndIf}
FunctionEnd

Function un.CheckInstalledRuntimeIsClosed
  IfFileExists "$INSTDIR\${MAIN_EXE}" 0 check_helper
  System::Call 'kernel32::CreateFileW(w "$INSTDIR\${MAIN_EXE}", i 0xC0000000, i 0, p 0, i 3, i 0, p 0) p.r0'
  ${If} $0 == -1
    MessageBox MB_ICONSTOP|MB_OK "KohdaLab Camera is in use. Close it before uninstalling."
    Abort
  ${EndIf}
  System::Call 'kernel32::CloseHandle(p r0)'
check_helper:
  IfFileExists "$INSTDIR\${HELPER_EXE}" 0 done
  System::Call 'kernel32::CreateFileW(w "$INSTDIR\${HELPER_EXE}", i 0xC0000000, i 0, p 0, i 3, i 0, p 0) p.r0'
  ${If} $0 == -1
    MessageBox MB_ICONSTOP|MB_OK "KohdaLab Camera helper is in use. Close it before uninstalling."
    Abort
  ${EndIf}
  System::Call 'kernel32::CloseHandle(p r0)'
done:
FunctionEnd

Section "Uninstall"
  SetShellVarContext all
  SetRegView 64
  Call un.CheckInstalledRuntimeIsClosed
  Delete "$DESKTOP\KohdaLab Camera.lnk"
  Delete "$SMPROGRAMS\KohdaLab Camera\KohdaLab Camera.lnk"
  RMDir "$SMPROGRAMS\KohdaLab Camera"
  !include "${PAYLOAD_UNINSTALL_INCLUDE}"
  Delete "$INSTDIR\Uninstall KohdaLab Camera.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKLM "${UNINSTALL_REGKEY}"
  DeleteRegKey HKLM "${PRODUCT_REGKEY}"
SectionEnd

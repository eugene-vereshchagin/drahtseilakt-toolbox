; Windows installer of the Drahtseilakt toolbox (NSIS, built on Linux by scripts/ci-package.sh with makensis).
; Defines from the command line: SRC (the portable folder), OUT (the installer file), VERSION, ARCH (x86_64 | aarch64).
Unicode true
RequestExecutionLevel admin
SetCompressor /SOLID lzma
Name "Drahtseilakt toolbox ${VERSION}"
OutFile "${OUT}"
InstallDir "$PROGRAMFILES64\Drahtseilakt toolbox"
; a fixed folder (no directory page): the uninstaller removes the whole folder, so it must never be one the user chose

!include "MUI2.nsh"
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

; the command line's /D= sets INSTDIR before this runs: put the fixed folder back, so the uninstaller's RMDir /r can
; never be pointed at a folder that holds other files
Function .onInit
  StrCpy $INSTDIR "$PROGRAMFILES64\Drahtseilakt toolbox"
FunctionEnd

Section "Install"
  SetRegView 64
  SetOutPath "$INSTDIR"
  File /r "${SRC}\*.*"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  CreateDirectory "$SMPROGRAMS\Drahtseilakt toolbox"
  CreateShortcut "$SMPROGRAMS\Drahtseilakt toolbox\Drahtseilakt toolbox.lnk" "$INSTDIR\drahtseilakt-toolbox.exe"
  CreateShortcut "$SMPROGRAMS\Drahtseilakt toolbox\Uninstall.lnk" "$INSTDIR\uninstall.exe"
  WriteRegStr HKLM "Software\Drahtseilakt toolbox" "Install_Dir" "$INSTDIR"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Drahtseilakt toolbox" "DisplayName" "Drahtseilakt toolbox"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Drahtseilakt toolbox" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Drahtseilakt toolbox" "Publisher" "Veresk"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Drahtseilakt toolbox" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Drahtseilakt toolbox" "NoModify" 1
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Drahtseilakt toolbox" "NoRepair" 1
SectionEnd

Section "Uninstall"
  SetRegView 64
  RMDir /r "$INSTDIR"
  Delete "$SMPROGRAMS\Drahtseilakt toolbox\*.lnk"
  RMDir "$SMPROGRAMS\Drahtseilakt toolbox"
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Drahtseilakt toolbox"
  DeleteRegKey HKLM "Software\Drahtseilakt toolbox"
SectionEnd

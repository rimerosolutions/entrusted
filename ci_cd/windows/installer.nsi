!include "MUI2.nsh"
Unicode True

; Reference https://gist.github.com/CoolOppo/5fb681682281b6adf6d8e2a5446f06ff
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_LANGUAGE "English"

; The name of the installer
Name "Dangerzone"

; The setup filename
OutFile "dangerzone-windows-amd64-_APPVERSION_-installer.exe"

; The default installation directory
InstallDir $PROGRAMFILES\Dangerzone

; Registry key to check for directory (so if you install again, it will
; overwrite the old one automatically)
InstallDirRegKey HKLM "Software\Dangerzone" "Install_Dir"

; For removing Start Menu shortcut in Windows 7
RequestExecutionLevel admin

UninstPage uninstConfirm
UninstPage instfiles

; start default section
Section "Install Dangerzone"

  SectionIn RO

  ; set the installation directory as the destination for the following actions
  SetOutPath $INSTDIR

  File dangerzone-cli.exe
  File dangerzone-gui.exe
  File dangerzone-httpclient.exe
  File dangerzone-httpserver.exe
  File LICENSE

  ; Write the installation path into the registry
  WriteRegStr HKLM SOFTWARE\YOURPROGRAM "Install_Dir" "$INSTDIR"

  ; Write the uninstall keys for Windows
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Dangerzone" "DisplayName" "Dangerzone"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Dangerzone" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Dangerzone" "NoModify" 1
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Dangerzone" "NoRepair" 1
  WriteUninstaller "$INSTDIR\uninstall.exe"
SectionEnd

; Optional section (can be disabled by the user)
Section "Start Menu Shortcuts (required)"
  SectionIn RO

  CreateDirectory "$SMPROGRAMS\Dangerzone"
  CreateShortcut "$SMPROGRAMS\Dangerzone\Uninstall.lnk" "$INSTDIR\uninstall.exe" "" "$INSTDIR\uninstall.exe" 0
  CreateShortcut "$SMPROGRAMS\Dangerzone\Dangerzone.lnk" "$INSTDIR\dangerzone-gui.exe" "" "$INSTDIR\dangerzone-gui.exe" 0
SectionEnd

; uninstaller section
Section -Uninstall
  ; Remove registry keys
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Dangerzone"
  DeleteRegKey HKLM SOFTWARE\Dangerzone  

  Delete $INSTDIR\dangerzone-cli.exe
  Delete $INSTDIR\dangerzone-gui.exe
  Delete $INSTDIR\dangerzone-httpclient.exe
  Delete $INSTDIR\dangerzone-httpserver.exe
  Delete $INSTDIR\LICENSE
  Delete $INSTDIR\uninstaller.exe

  Delete "$SMPROGRAMS\Dangerzone\*.*"
  RMDir "$SMPROGRAMS\Dangerzone"

  RMDir $INSTDIR
SectionEnd

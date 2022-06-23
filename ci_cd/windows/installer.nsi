!include "MUI2.nsh"
Unicode True
Var SMDir
; Reference https://gist.github.com/CoolOppo/5fb681682281b6adf6d8e2a5446f06ff
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "LICENSE.txt"
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_STARTMENU 0 $SMDir
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_LANGUAGE "English"

; The name of the installer
Name "Entrusted"

; The setup filename
OutFile "entrusted-windows-amd64-_APPVERSION_.exe"

; The default installation directory
InstallDir $PROGRAMFILES\Entrusted

; Registry key to check for directory (so if you install again, it will
; overwrite the old one automatically)
InstallDirRegKey HKLM "Software\Entrusted" "Install_Dir"

; For removing Start Menu shortcut in Windows 7
RequestExecutionLevel admin

UninstPage uninstConfirm
UninstPage instfiles

; start default section
Section "Install Entrusted"

  SectionIn RO

  ; set the installation directory as the destination for the following actions
  SetOutPath $INSTDIR

  File entrusted-cli.exe
  File entrusted-gui.exe
  File LICENSE.txt

  ; Write the installation path into the registry
  WriteRegStr HKLM SOFTWARE\YOURPROGRAM "Install_Dir" "$INSTDIR"

  ; Write the uninstall keys for Windows
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Entrusted" "DisplayName" "Entrusted"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Entrusted" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Entrusted" "NoModify" 1
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Entrusted" "NoRepair" 1
  WriteUninstaller "$INSTDIR\uninstall.exe"
SectionEnd

; Optional section (can be disabled by the user)
Section -StartMenu
  !insertmacro MUI_STARTMENU_WRITE_BEGIN 0 ;This macro sets $SMDir and skips to MUI_STARTMENU_WRITE_END if the "Don't create shortcuts" checkbox is checked... 

  CreateDirectory "$SMPROGRAMS\Entrusted"
  CreateShortcut "$SMPROGRAMS\Entrusted\Uninstall.lnk" "$INSTDIR\uninstall.exe" "" "$INSTDIR\uninstall.exe" 0
  CreateShortcut "$SMPROGRAMS\Entrusted\Entrusted.lnk" "$INSTDIR\entrusted-gui.exe" "" "$INSTDIR\entrusted-gui.exe" 0

  !insertmacro MUI_STARTMENU_WRITE_END
SectionEnd

; uninstaller section
Section -Uninstall
  ; Remove registry keys
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Entrusted"
  DeleteRegKey HKLM SOFTWARE\Entrusted  

  Delete $INSTDIR\entrusted-cli.exe
  Delete $INSTDIR\entrusted-gui.exe
  Delete $INSTDIR\LICENSE.txt
  Delete $INSTDIR\uninstaller.exe

  Delete "$SMPROGRAMS\Entrusted\*.*"
  RMDir "$SMPROGRAMS\Entrusted"

  RMDir $INSTDIR
SectionEnd

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

!define ProjectName      "entrusted"
!define ProgramName      "Entrusted"
!define ProgramNameGUI   "entrusted-gui"
!define ProgramNameCLI   "entrusted-cli"
!define ProgramPublisher "Rimero Solutions Inc."
!define ProgramVersion   _APPVERSION_

VIProductVersion ${ProgramVersion}
VIAddVersionKey  "ProductVersion"  "${ProgramVersion}"
VIAddVersionKey  "ProductName"     "${ProgramName}"
VIAddVersionKey  "FileVersion"     "${ProgramVersion}"
VIAddVersionKey  "LegalCopyright"  "${ProgramPublisher}"
VIAddVersionKey  "FileDescription" "${ProgramName} Installer"

; The name of the installer
Name "Entrusted"

; The setup filename
OutFile "entrusted-windows-amd64-_APPVERSION_.exe"

; The default installation directory
InstallDir "$PROGRAMFILES\${ProgramName}"

; Registry key to check for directory (so if you install again, it will
; overwrite the old one automatically)
InstallDirRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${ProgramName}" "InstallLocation"

; For removing Start Menu shortcut in Windows 7
RequestExecutionLevel admin

UninstPage uninstConfirm
UninstPage instfiles

; start default section
Section "Executable and uninstaller"
  SectionIn RO
  ; set the installation directory as the destination for the following actions
  SetOutPath $INSTDIR

  File entrusted-cli.exe
  File entrusted-gui.exe
  File LICENSE.txt

  WriteUninstaller "$INSTDIR\${ProgramName}-uninstall.exe"
SectionEnd

Section "Add to Windows Programs & Features"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${ProgramName}" "DisplayName" "${ProgramName}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${ProgramName}" "Publisher" "${ProgramPublisher}"

  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${ProgramName}" "DisplayIcon" "$INSTDIR\${ProgramNameGUI}.exe"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${ProgramName}" "InstallLocation" "$INSTDIR\"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${ProgramName}" "UninstallString" "$INSTDIR\${ProgramName}-uninstall.exe"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${ProgramName}" "DisplayVersion" "${ProgramVersion}"

  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${ProgramName}" "NoModify" 1
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${ProgramName}" "NoRepair" 1

  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${ProgramName}" "HelpLink" "https://github.com/rimerosolutions/${ProjectName}/issues/new"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${ProgramName}" "URLInfoAbout" "https://github.com/rimerosolutions/${ProjectName}" ; Support Link
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${ProgramName}" "URLUpdateInfo" "https://github.com/rimerosolutions/${ProjectName}/releases" ; Update Info Link
SectionEnd

; Optional section (can be disabled by the user)
Section "Start Menu Shortcusts"
  !insertmacro MUI_STARTMENU_WRITE_BEGIN 0 ;This macro sets $SMDir and skips to MUI_STARTMENU_WRITE_END if the "Don't create shortcuts" checkbox is checked...

  CreateDirectory "$SMPROGRAMS\${ProgramName}"
  CreateShortcut "$SMPROGRAMS\${ProgramName}\Uninstall.lnk" "$INSTDIR\${ProgramName}-uninstall.exe" "" "$INSTDIR\uninstall.exe" 0
  CreateShortcut "$SMPROGRAMS\${ProgramName}\${ProgramName}.lnk" "$INSTDIR\${ProgramNameGUI}.exe" "" "$INSTDIR\${ProgramNameGUI}.exe" 0

!insertmacro MUI_STARTMENU_WRITE_END
SectionEnd

Section "Add to Open With menu"
  WriteRegStr HKCR "Applications\${ProgramName}.exe\shell\open\command" "" "$\"$INSTDIR\${ProgramNameGUI}.exe$\" $\"%1$\""
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.rtf\OpenWithList"  "z" "${ProgramName}.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.doc\OpenWithList"  "z" "${ProgramName}.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.docx\OpenWithList" "z" "${ProgramName}.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.odt\OpenWithList"  "z" "${ProgramName}.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.xls\OpenWithList"  "z" "${ProgramName}.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.xlsx\OpenWithList" "z" "${ProgramName}.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.ppt\OpenWithList"  "z" "${ProgramName}.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.pptx\OpenWithList" "z" "${ProgramName}.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.odp\OpenWithList"  "z" "${ProgramName}.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.odg\OpenWithList"  "z" "${ProgramName}.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.gif\OpenWithList"  "z" "${ProgramName}.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.png\OpenWithList"  "z" "${ProgramName}.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.jpg\OpenWithList"  "z" "${ProgramName}.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.jpeg\OpenWithList" "z" "${ProgramName}.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.tiff\OpenWithList" "z" "${ProgramName}.exe"
SectionEnd

; uninstaller section
Section "Uninstall"
  ; Remove application files
  Delete "$INSTDIR\${ProgramName}-uninstall.exe"
  Delete "$INSTDIR\${ProgramNameCLI}.exe"
  Delete "$INSTDIR\${ProgramNameGUI}.exe"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\${ProgramName}-uninstall.exe"
  RMDir "$INSTDIR"

  ; Remove Windows Programs & Features integration (uninstall info)
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${ProgramName}"

  ; Remove Start Menu Shortcuts & Folder
  Delete "$SMPROGRAMS\${ProgramName}\${ProgramName}.lnk"
  Delete "$SMPROGRAMS\${ProgramName}\Uninstall.lnk"
  RMDir "$SMPROGRAMS\${ProgramName}"

  ; Remove open with association
  DeleteRegKey HKCR Applications\${ProgramName}.exe
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.rtf\OpenWithList"  "z"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.doc\OpenWithList"  "z"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.docx\OpenWithList" "z"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.odt\OpenWithList"  "z"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.xls\OpenWithList"  "z"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.xlsx\OpenWithList" "z"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.ods\OpenWithList"  "z"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.ppt\OpenWithList"  "z"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.pptx\OpenWithList" "z"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.odp\OpenWithList"  "z"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.odg\OpenWithList"  "z"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.gif\OpenWithList"  "z"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.png\OpenWithList"  "z"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.jpg\OpenWithList"  "z"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.jpeg\OpenWithList" "z"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.tiff\OpenWithList" "z"
SectionEnd

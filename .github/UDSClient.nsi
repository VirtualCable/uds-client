!define VERSION 1.0.0

Name "UDSClient"
OutFile "openUDS-Client_Installer-${VERSION}.exe"
InstallDir "$PROGRAMFILES\UDSClient"
Page Directory
Page InstFiles
Section
  SetOutPath $INSTDIR
  File /nonfatal /a /r "UDSClient\"

  WriteRegStr HKCR "uds" "" "URL:UDS Protocol"
  WriteRegStr HKCR "uds" "URL Protocol" ""
  WriteRegStr HKCR "uds\shell\open\command" "" '"$INSTDIR\UDSClient.exe" "%1"'

  WriteRegStr HKCR "udss" "" "URL:UDS Protocol SSL"
  WriteRegStr HKCR "udss" "URL Protocol" ""
  WriteRegStr HKCR "udss\shell\open\command" "" '"$INSTDIR\UDSClient.exe" "%1"'

  WriteUninstaller $INSTDIR\uninstaller.exe
SectionEnd

Section "Uninstall"

Delete $INSTDIR\uninstaller.exe

RMDir /r $INSTDIR
SectionEnd
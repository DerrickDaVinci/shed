; The Explorer entries are written by the app on every start, following its
; setting and language. The installer adds them right away so they work
; before the first launch, and the uninstaller removes them.

!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr HKCU "Software\Classes\Directory\shell\Shed" "" "Clean with Shed"
  WriteRegStr HKCU "Software\Classes\Directory\shell\Shed" "Icon" '"$INSTDIR\${MAINBINARYNAME}.exe",0'
  WriteRegStr HKCU "Software\Classes\Directory\shell\Shed\command" "" '"$INSTDIR\${MAINBINARYNAME}.exe" --open "%1"'
  WriteRegStr HKCU "Software\Classes\Directory\Background\shell\Shed" "" "Clean with Shed"
  WriteRegStr HKCU "Software\Classes\Directory\Background\shell\Shed" "Icon" '"$INSTDIR\${MAINBINARYNAME}.exe",0'
  WriteRegStr HKCU "Software\Classes\Directory\Background\shell\Shed\command" "" '"$INSTDIR\${MAINBINARYNAME}.exe" --open "%V"'
  WriteRegStr HKCU "Software\Classes\DesktopBackground\Shell\Shed" "" "Clean This PC with Shed"
  WriteRegStr HKCU "Software\Classes\DesktopBackground\Shell\Shed" "Icon" '"$INSTDIR\${MAINBINARYNAME}.exe",0'
  WriteRegStr HKCU "Software\Classes\DesktopBackground\Shell\Shed\command" "" '"$INSTDIR\${MAINBINARYNAME}.exe" --computer'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegKey HKCU "Software\Classes\Directory\shell\Shed"
  DeleteRegKey HKCU "Software\Classes\Directory\Background\shell\Shed"
  DeleteRegKey HKCU "Software\Classes\DesktopBackground\Shell\Shed"
!macroend

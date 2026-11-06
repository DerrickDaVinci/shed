; The Explorer entry ("Clean with Shed" on folders) is written by the app on
; every start, following its setting. The installer adds it right away so it
; works before the first launch, and the uninstaller removes it.

!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr HKCU "Software\Classes\Directory\shell\Shed" "" "Clean with Shed"
  WriteRegStr HKCU "Software\Classes\Directory\shell\Shed" "Icon" '"$INSTDIR\${MAINBINARYNAME}.exe",0'
  WriteRegStr HKCU "Software\Classes\Directory\shell\Shed\command" "" '"$INSTDIR\${MAINBINARYNAME}.exe" --open "%1"'
  WriteRegStr HKCU "Software\Classes\Directory\Background\shell\Shed" "" "Clean with Shed"
  WriteRegStr HKCU "Software\Classes\Directory\Background\shell\Shed" "Icon" '"$INSTDIR\${MAINBINARYNAME}.exe",0'
  WriteRegStr HKCU "Software\Classes\Directory\Background\shell\Shed\command" "" '"$INSTDIR\${MAINBINARYNAME}.exe" --open "%V"'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegKey HKCU "Software\Classes\Directory\shell\Shed"
  DeleteRegKey HKCU "Software\Classes\Directory\Background\shell\Shed"
!macroend

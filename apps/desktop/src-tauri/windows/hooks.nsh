; Titi finds phones on the same Wi-Fi via UDP multicast 239.77.84.84:41414.
; Allow inbound on Private/Domain networks only (never Public), per program.
!macro NSIS_HOOK_POSTINSTALL
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="Titi LAN"'
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="Titi LAN" dir=in action=allow program="$INSTDIR\titi.exe" enable=yes profile=private,domain protocol=UDP'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="Titi LAN"'
!macroend

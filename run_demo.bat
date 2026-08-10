@echo off
setlocal
set "TEMPLEFIX_EXE=%~dp0src-tauri\target\debug\templefix.exe"

if not exist "%TEMPLEFIX_EXE%" (
  echo TempleFix debug executable was not found.
  echo Build the project first, then run this file again.
  exit /b 1
)

start "" "%TEMPLEFIX_EXE%"
endlocal

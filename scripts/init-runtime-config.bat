@echo off
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0init-runtime-config.ps1" %*

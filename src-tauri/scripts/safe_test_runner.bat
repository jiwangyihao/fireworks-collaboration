@echo off
REM Safe Test Runner for Windows
REM Avoids DLL conflicts by removing paths that contain conflicting DLLs
REM while preserving the rest of the user's PATH.
REM
REM REMOVES from PATH: Git\usr\bin, Git\mingw64\bin (contain zlib1.dll, etc.)
REM KEEPS: Everything else including Git\cmd, nodejs, etc.

setlocal enabledelayedexpansion

REM Build new PATH excluding problematic Git paths
set "NEWPATH="
for %%p in ("%PATH:;=";"%") do (
    set "ENTRY=%%~p"
    REM Skip Git\usr\bin and Git\mingw64\bin
    echo !ENTRY! | findstr /i "\\Git\\usr\\bin \\Git\\mingw64\\bin" >nul
    if errorlevel 1 (
        if defined NEWPATH (
            set "NEWPATH=!NEWPATH!;!ENTRY!"
        ) else (
            set "NEWPATH=!ENTRY!"
        )
    )
)

endlocal & set "PATH=%NEWPATH%"
%*

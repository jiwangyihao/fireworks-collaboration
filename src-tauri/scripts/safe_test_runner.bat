@echo off
REM Safe Test Runner for Windows
REM Avoids DLL conflicts by using a minimal PATH that includes:
REM - System32 (core Windows)
REM - Git\cmd (git.exe only, no conflicting DLLs)
REM
REM EXCLUDES: Git\usr\bin, Git\mingw64\bin (contain zlib1.dll, etc.)

set "PATH=C:\Windows\System32;C:\Windows;C:\Program Files\Git\cmd"
%*

@echo off
REM Safe Test Runner for Windows
REM Avoids DLL conflicts by removing paths that contain conflicting DLLs
REM while preserving the rest of the user's PATH.
REM
REM REMOVES from PATH: Git\usr\bin, Git\mingw64\bin (contain zlib1.dll, etc.)
REM KEEPS: Everything else including Git\cmd, nodejs, etc.

setlocal EnableDelayedExpansion

REM Fast string replacement to disable problematic paths without looping (O(1))
set "NEWPATH=%PATH%"
set "NEWPATH=!NEWPATH:Git\usr\bin=Git\usr\bin_disabled!"
set "NEWPATH=!NEWPATH:Git\mingw64\bin=Git\mingw64\bin_disabled!"

REM Set the modified PATH
set "PATH=!NEWPATH!"
%*

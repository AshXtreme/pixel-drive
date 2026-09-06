@echo off
setlocal enabledelayedexpansion
echo ============================================
echo   PixelDrive Windows Distribution Builder
echo ============================================

set TAG=v1.3
if not "%1"=="" set TAG=%1

echo Building PixelDrive in Release mode...
cargo build --release
if errorlevel 1 (
    echo [ERROR] Build failed!
    exit /b %errorlevel%
)

set DIST_DIR=dist\PixelDrive-Windows
if exist dist\PixelDrive-Windows rd /s /q dist\PixelDrive-Windows
mkdir %DIST_DIR%
mkdir %DIST_DIR%\cores
mkdir %DIST_DIR%\saves
mkdir %DIST_DIR%\assets

echo Copying binaries and assets...
copy target\release\pixel-drive.exe %DIST_DIR%\PixelDrive.exe >nul 2>&1
if not exist %DIST_DIR%\PixelDrive.exe copy target\release\pixeldrive.exe %DIST_DIR%\PixelDrive.exe >nul 2>&1
copy README.md %DIST_DIR%\ >nul 2>&1
copy LICENSE %DIST_DIR%\ >nul 2>&1
copy LEGAL.md %DIST_DIR%\ >nul 2>&1
if exist assets\windows\icon.ico copy assets\windows\icon.ico %DIST_DIR%\assets\ >nul 2>&1
if exist cores\*.dll copy cores\*.dll %DIST_DIR%\cores\ >nul 2>&1

echo Creating ZIP distribution archive...
powershell -Command "Compress-Archive -Path '%DIST_DIR%' -DestinationPath 'dist\PixelDrive-Windows-%TAG%.zip' -Force"
powershell -Command "Copy-Item 'dist\PixelDrive-Windows-%TAG%.zip' 'dist\PixelDrive-Windows-x86_64.zip' -Force"

echo ============================================
echo [SUCCESS] Windows package created:
echo dist\PixelDrive-Windows-%TAG%.zip
echo ============================================

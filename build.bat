@echo off
setlocal
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
pushd "%~dp0" || exit /b 1
timeout /t 1 /nobreak >nul
cargo build -p windows_shell --release --locked --features skate3
set "BUILD_EXIT_CODE=%ERRORLEVEL%"
popd
echo.
echo Build finished with exit code %BUILD_EXIT_CODE%.
pause
exit /b %BUILD_EXIT_CODE%

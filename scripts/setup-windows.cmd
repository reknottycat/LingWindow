@echo off
chcp 65001 >nul
setlocal
set "MODEL_DIR=%LOCALAPPDATA%\com.audioscript.hymt2\models\mt-hymt2"
echo AudioScript HYMT2 Windows setup
echo Model: Tencent HY-MT2 1.8B BF16, about 4.1 GB from ModelScope.
echo Runtime: local Python venv and official PyTorch CUDA 12.8 wheels.
echo Requires Python 3.12 x64, NVIDIA driver with CUDA 12.8 support and 12 GB free disk.
echo No administrator rights are required. No audio is uploaded.
where py >nul 2>nul
if errorlevel 1 (
  echo Install Python 3.12 x64 from https://www.python.org/downloads/windows/ first.
  pause
  exit /b 1
)
py -3.12 -X utf8 "%~dp0setup-windows-hymt2.py" --model-dir "%MODEL_DIR%"
if errorlevel 1 (
  echo Setup failed. Keep this window open and copy the error for support.
  pause
  exit /b 1
)
echo Setup complete. Open AudioScript HYMT2, download Nemotron 3.5 on the Models page,
echo then select Russian - Chinese, system audio, and show the subtitle overlay.
pause

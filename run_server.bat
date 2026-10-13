@echo off
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul
set PATH=%PATH%;C:\Users\USER\.cargo\bin
cd /d C:\ai\meta_server
timeout /t 3 /nobuff >nul
cargo run 2>&1

@echo off
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul
set PATH=%PATH%;C:\Users\USER\.cargo\bin
set JWT_DISABLED=true
cd /d C:\ai\meta_server
cargo run 2>&1

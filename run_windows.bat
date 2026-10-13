@echo off
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul
set PATH=%PATH%;C:\Users\USER\.cargo\bin
cd /d C:\ai\meta_server
set DATABASE_BACKEND=memory
set REDIS_ENABLED=false
set JWT_DISABLED=true
set ADMIN_TOKEN=admin-secret-123
set RATE_LIMIT_PER_MIN=1000
echo Starting meta_server on http://localhost:3000 ...
echo Admin page: http://localhost:3000/admin
echo Blog API: http://localhost:3000/api/blog/articles
echo Template API: http://localhost:3000/api/templates
echo Admin Token: X-Admin-Token: admin-secret-123
start "meta_server" /min target\debug\meta_server.exe

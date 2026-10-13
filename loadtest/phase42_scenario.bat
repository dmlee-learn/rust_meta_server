@echo off
REM Phase 42 P4: Multi-Domain Chain Live Server Verification Scenario
REM Usage: 1) Start server with JWT_DISABLED=true (port 3000) 2) Run this script
REM Prereq: curl + python in PATH

set SERVER=http://127.0.0.1:3000
set PASS=0
set FAIL=0

echo === Phase 42 P4 Scenario Start ===
echo.

REM 1. Server health check
echo [1/6] Server health check...
curl -s %SERVER%/health >nul 2>&1
if %errorlevel%==0 (
  echo   PASS: Server is up
  set /a PASS+=1
) else (
  echo   FAIL: Server not reachable
  set /a FAIL+=1
  goto :end
)

REM 2. Template list - expect 8 templates
echo [2/6] Template list check...
curl -s "%SERVER%/api/templates?tenant=default" | python -c "import json,sys; d=json.load(sys.stdin); print('  count:',d['count']); sys.exit(0 if d['count']>=8 else 1)" 2>nul
if %errorlevel%==0 (set /a PASS+=1) else (set /a FAIL+=1)

REM 3. Tree check - 8 nodes under site.shop
echo [3/6] Template tree check...
curl -s "%SERVER%/api/templates/site.shop/tree?tenant=default" | python -c "
import json,sys
d=json.load(sys.stdin)
nodes=[]
def walk(n):
    nodes.append(n['template'].get('template_id','?'))
    for c in n.get('children',[]): walk(c)
walk(d['template'])
print('  nodes:',len(nodes))
sys.exit(0 if len(nodes)>=8 else 1)
" 2>nul
if %errorlevel%==0 (set /a PASS+=1) else (set /a FAIL+=1)

REM 4. Create board instance from body.hero
echo [4/6] Create board instance from body.hero...
curl -s -X POST "%SERVER%/api/board/items?template_id=body.hero&tenant=default&sync_policy=live" -H "Content-Type: application/json" -d "{\"title\":\"test\"}" | python -c "import json,sys; d=json.load(sys.stdin); print('  code:',d.get('code'),'id:',d.get('id')); sys.exit(0 if d.get('code')=='OK' else 1)" 2>nul
if %errorlevel%==0 (set /a PASS+=1) else (set /a FAIL+=1)

REM 5. Bump menu.main + propagate
echo [5/6] Bump + Propagate menu.main...
curl -s -X POST "%SERVER%/api/admin/templates/menu.main/bump?tenant=default" -H "X-Admin-Token: test" | python -c "import json,sys; d=json.load(sys.stdin); print('  bump:',d.get('code'),'v'+str(d.get('new_version'))); sys.exit(0 if d.get('code')=='OK' else 1)" 2>nul
if %errorlevel%==0 (set /a PASS+=1) else (set /a FAIL+=1)

REM 6. Apply site template to acme
echo [6/6] Apply site template to tenant acme...
curl -s -X POST "%SERVER%/api/admin/tenants/acme/apply-site-template?site_template_id=site.shop&tenant=default" -H "X-Admin-Token: test" | python -c "import json,sys; d=json.load(sys.stdin); print('  code:',d.get('code'),'copied:',d.get('copied')); sys.exit(0 if d.get('code')=='OK' else 1)" 2>nul
if %errorlevel%==0 (set /a PASS+=1) else (set /a FAIL+=1)

:end
echo.
echo === Phase 42 P4 Scenario End ===
echo PASS: %PASS%, FAIL: %FAIL%
if %FAIL%==0 (
  echo RESULT: ALL PASS
) else (
  echo RESULT: %FAIL% FAILED
)
exit /b %FAIL%

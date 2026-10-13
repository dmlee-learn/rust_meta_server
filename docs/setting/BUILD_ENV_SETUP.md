# rquickjs 0.14 빌드/동작 설정 가이드 (Windows MSVC)

> 이 문서는 **금방 다시 빌드하고 동작하는** 방법을 기록합니다.
> 처음 이 환경에 설치하는 개발자가 이 문서만 읽어도 `cargo test`까지 재현할 수 있습니다.
> 조사 시점: 2026-09-30, rquickjs 0.14.0, Rust 1.98.1, Windows 11 (MSYS2/MinGW bash 기반)

---

## 0. 한 줄 요약

MSVC Build Tools 2022를 설치하고, Windows SDK의 `kernel32.lib` 등 시스템 라이브러리를
공백이 없는 경로(junction `C:\wsdk10`)로 link.exe에 전달하면 rquickjs 0.14가 빌드/동작한다.

```
빌드 성공 = MSVC(cl.exe + link.exe) + Windows SDK lib(junction) + MSYS 경로변환 차단
```

---

## 1. 환경 구성 (일회성 설치)

### 1.1 Rust 툴체인
```bash
winget install --id Rustlang.Rustup -e --accept-source-agreements --accept-package-agreements
# 이후 ~/.cargo/bin 가 PATH에 추가됨 (새 셸에서)
cargo --version   # 1.98.1
```

### 1.2 Visual Studio Build Tools 2022 (MSVC C/C++ 도구)
```bash
winget install --id Microsoft.VisualStudio.2022.BuildTools \
  --override "--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```
설치 후 위치:
```
C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat
C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\<버전>\bin\HostX64\x64\cl.exe
C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\<버전>\bin\HostX64\x64\link.exe
```

### 1.3 Windows SDK 라이브러리 junction 생성 (필수)
Windows SDK의 lib 경로에는 `Program Files (x86)`의 **공백**이 있어
bash/rustc link-arg 전달 시 인자 분할로 깨진다. 공백 없는 junction을 만든다.

```bat
:: C:\temp\mkjunction.bat  (bat 파일로 만들어야 cmd 경로변환 안전)
@echo off
mklink /J C:\wsdk10 "C:\Program Files (x86)\Windows Kits\10\lib"
```
```bash
cmd //c "C:\\temp\\mkjunction.bat"
# 확인: C:\wsdk10\10.0.26100.0\um\x64\kernel32.lib 이 존재해야 함
```

---

## 2. 빌드/실행 환경 스크립트

### 2.1 MSVC 환경 로드 프로파일 `/c/temp/load_msenv.sh`

`vcvars64.bat`의 `set` 출력을 파일로 받아, PATH 외의 핵심 변수만 로드한다.
**PATH는 MSVC 앞단, 기존 Unix 도구를 뒤단에 보존**해서 `grep`/`ls` 등이 계속 동작한다.

```bash
# /c/temp/load_msenv.sh
ORIG_PATH="$PATH"
_set_env() {
  local key="$1" val="$2"
  case "$key" in
    INCLUDE|LIB|LIBPATH|VCToolsInstallDir|WindowsSdkDir|WindowsSDKVersion|FrameworkVersion|VSInstallationPath|VCTargetsPath|NETFrameworkSDKDir)
      export "$key=$val"
      ;;
    PATH)
      export "PATH=$val:$ORIG_PATH"
      ;;
  esac
}
while IFS= read -r line; do
  key="${line%%=*}"; val="${line#*=}"
  _set_env "$key" "$val"
done < /c/temp/vcenv.txt
```

`/c/temp/vcenv.txt` 생성 (MSVC 환경 덤프):
```bat
:: /c/temp/VC.bat
@echo off
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1
set
```
```bash
cmd //c "C:\\temp\\VC.bat" > /c/temp/vcenv.txt 2>&1
```

> 주의: `set` 출력에는 `CommonProgramFiles(x86)` 같은 `(` 포함 변수가 있어
> bash `export`에 대입하면 `not a valid identifier` 오류를 낸다.
> 그래서 위 스크립트는 **필터 화이트리스트**만 export 한다.

### 2.2 빌드 실행 (매 세션마다)

```bash
# MSYS2/MinGW bash 기준
export MSYS_NO_PATHCONV=1            # MSYS 경로자동변환 차단 (kernel32.lib 등 윈도우 경로 보존)
export MSYS2_ARG_CONV_EXCL="*"       # 인자 변환 완전 차단
export PATH="$HOME/.cargo/bin:$PATH"
source /c/temp/load_msenv.sh

export CC="C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\14.44.35207\bin\HostX64\x64\cl.exe"
export RUSTFLAGS='-C link-arg=/LIBPATH:C:\wsdk10\10.0.26100.0\um\x64 -C link-arg=/LIBPATH:C:\wsdk10\10.0.26100.0\ucrt\x64'

cd <프로젝트>
cargo test
```

`14.44.35207`은 설치된 MSVC 버전이다. `vcvars64` 실행 후 `echo $PATH`에서
`MSVC\<버전>`을 확인하면 된다.

---

## 3. 왜 이렇게 하는가 (장애 원인 기록)

| 증상 | 원인 | 해결 |
|------|------|------|
| `link.exe` not found | MSVC 미설치 | Build Tools 2022 설치 |
| `link: extra operand '....o'` | MSYS가 MSVC 링크 인자를 Unix 경로로 변환 | `MSYS_NO_PATHCONV=1` |
| `LNK1181: kernel32.lib` | link.exe가 Windows SDK lib를 LIB에서 못 찾음 | `RUSTFLAGS`에 `/LIBPATH:C:\wsdk10\...` 지정 |
| `multiple input filenames provided` | RUSTFLAGS 경로 공백이 bash에서 인자 분할 | 공백 없는 junction `C:\wsdk10` 사용 |
| `libgcc`/`libgcc_eh` not found | llvm-mingw에 Windows용 GCC 런타임 라이브러리 없음 | MSVC target으로 전환 (GNU target 폐기) |

**결론:** MSVC target(`x86_64-pc-windows-msvc`) + MSVC `cl.exe` + Windows SDK lib(junction) 조합만
이 환경에서 rquickjs C 컴파일과 Rust 링크를 동시에 만족시킨다.

---

## 4. rquickjs 0.14 핵심 API (빌드 확인 포함)

빌드/링크가 확인된 상태의 실제 동작을 검증한 API 요약.
상세 변경점은 `docs/rquickjs/API_CHANGES.md` 참조.

```rust
use rquickjs::{Context, Runtime, Error};
use std::time::Instant;

let rt = Runtime::new()?;                 // -> Result<Runtime>
rt.set_memory_limit(1 << 20);             // 바이트 단위. 0 = 무제한. (rust-alloc feature 사용 시 No-op)
rt.set_max_stack_size(256 * 1024);        // 기본 256KB
rt.set_gc_threshold(0);                   // 0 = GC 자동

// CPU 타임아웃: 인터럽트 핸들러로 구현
let start = Instant::now();
rt.set_interrupt_handler(Some(Box::new(move || start.elapsed().as_millis() >= 500)));

let ctx = Context::full(&rt)?;            // Context 생성
ctx.with(|ctx| {
    let _ = ctx.eval::<f64, _>("let s=0;for(let i=0;i<2e9;i++){s+=i}s")?; // 무한루프 -> 500ms에 강제 인터럽트
    Ok::<f64, Error>(0.0)
});
rt.set_interrupt_handler(None);           // 해제
```

검증된 동작:
- `set_memory_limit`으로 제한 초과 시 `Error::Exception`(`is_exception()==true`) 반환 → **CJ1103** 매핑 대상
- `set_interrupt_handler`로 CPU 타임아웃 초과 시 `Error::Exception` 반환 → **CJ1102** 매핑 대상
- 인터럽트 후 핸들러 해제하면 런타임은 정상 가동 지속

---

## 5. 재현 체크리스트

```bash
# 1) 경로변환 차단
export MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL="*"
# 2) MSVC env 로드
source /c/temp/load_msenv.sh
# 3) C 컴파일러 + SDK lib 경로
export CC="C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\14.44.35207\bin\HostX64\x64\cl.exe"
export RUSTFLAGS='-C link-arg=/LIBPATH:C:\wsdk10\10.0.26100.0\um\x64 -C link-arg=/LIBPATH:C:\wsdk10\10.0.26100.0\ucrt\x64'
# 4) 빌드
cargo clean && cargo test
```

`cl.exe` 경로의 `14.44.35207`은 `vcvars64` 실행 후 `echo $PATH`에서 확인된 실제 MSVC 버전으로 교체.

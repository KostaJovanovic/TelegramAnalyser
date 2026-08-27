@echo off
setlocal enabledelayedexpansion
cd /d "%~dp0"

rem Same shape as telegram_rust's save.bat: run it bare for a menu, or pass the
rem action as an argument. Exit code is non-zero if anything failed, so it can
rem be chained.
rem
rem The git actions are lifted from telegram_rust rather than rewritten, and the
rem --force-with-lease --force-if-includes reasoning down in :push is worth
rem having verbatim: a bare --force overwrites work nobody has seen, and the
rem lease on its own is weaker than it reads.

rem The two corpora. Both live on removable drives, so every step that uses one
rem degrades to a skip when the drive is not mounted rather than failing the
rem run.
set "UAEXPORT=N:\telegram export\UA KOLAB TELEGRAM"

rem **The KRGM folder is not spelled out, and that is deliberate.** Its name is
rem "KRGM TRECI 2026.06.09" with a C-acute, and a .bat is read in the console's
rem OEM codepage rather than as UTF-8 -- so the literal would arrive mangled on
rem any machine whose console is not cp1250, and `if exist` would then report a
rem mounted drive as missing. `for /d` gets the name from the filesystem, in
rem whatever encoding the shell is already using, and hands it straight back out
rem to cargo and python through the same layer.
set "KRGMEXPORT="
for /d %%d in ("J:\temp pureraw\KRGM*") do set "KRGMEXPORT=%%~fd"
if not defined KRGMEXPORT set "KRGMEXPORT=J:\temp pureraw\KRGM (not mounted)"

rem The Python analyser, which is the oracle for both the numbers and the HTML.
rem Its venv, not the system python: `analyser.read` and `analyser.metrics` are
rem plain modules, but the package imports PySide6 on the window path.
set "PYSRC=C:\Users\Kosta\Projekti\telegram"
set "PYEXE=%PYSRC%\.venv\Scripts\python.exe"

rem This machine owns both exports, so a `cargo test` here that finds no corpus
rem has not "skipped the legs" -- it has a broken setup, and libtest throws away
rem the eprintln that would have said so. With this set, the corpus tests panic
rem instead. `set TGA_REQUIRE_CORPUS=0` before calling opts back out; a plain
rem `cargo test --all` is unaffected, which is what keeps a fresh clone working.
if not defined TGA_REQUIRE_CORPUS set "TGA_REQUIRE_CORPUS=1"

rem Two clocks. T0 covers the whole run and is reported at the end; TS is
rem restarted per step, so a slow run says *which* step was slow rather than
rem leaving you watching a crate counter and guessing.
call :clock T0
set SAVE_ERROR=0
set ACTION=%~1

set COMMIT_ONLY=0
if /i "%ACTION%"=="commit"   (set "COMMIT_ONLY=1" & set "ACTION=save")
if /i "%ACTION%"=="--commit" (set "COMMIT_ONLY=1" & set "ACTION=save")
if /i "%ACTION%"=="save"    goto save
if /i "%ACTION%"=="push"    goto push
if /i "%ACTION%"=="pull"    goto pull
if /i "%ACTION%"=="test"    goto test
if /i "%ACTION%"=="tests"   goto test
if /i "%ACTION%"=="build"   goto build
if /i "%ACTION%"=="exe"     goto build
if /i "%ACTION%"=="run"     goto runwindow
if /i "%ACTION%"=="window"  goto runwindow
if /i "%ACTION%"=="report"  goto report
if /i "%ACTION%"=="stats"   goto stats
if /i "%ACTION%"=="oracle"  goto oracle
if /i "%ACTION%"=="parity"  goto parity
if /i "%ACTION%"=="bless"   goto bless
if /i "%ACTION%"=="clean"   goto clean

:menu
echo.
echo === telegram analyser (rust) ===
echo.
echo   1  save     test + commit + push
echo   2  commit   test + commit, no push
echo   3  push     push the current branch
echo   4  pull     pull the current branch
echo   5  test     fmt + clippy + every suite
echo   6  build    cargo build --release
echo   7  run      open the window
echo   8  report   write report.html for an export folder
echo   9  stats    write the --stats dump for an export folder
echo   10 oracle   re-record the Python side for both corpora
echo   11 parity   diff both reports against the Python analyser's
echo   12 bless    re-record the two committed goldens
echo   13 clean    report the build cache, and empty it
echo   14 quit
echo.
set /p CHOICE=select [1-14]:
if "%CHOICE%"=="1" goto save
if "%CHOICE%"=="2" (set "COMMIT_ONLY=1" & goto save)
if "%CHOICE%"=="3" goto push
if "%CHOICE%"=="4" goto pull
if "%CHOICE%"=="5" goto test
if "%CHOICE%"=="6" goto build
if "%CHOICE%"=="7" goto runwindow
if "%CHOICE%"=="8" goto report
if "%CHOICE%"=="9" goto stats
if "%CHOICE%"=="10" goto oracle
if "%CHOICE%"=="11" goto parity
if "%CHOICE%"=="12" goto bless
if "%CHOICE%"=="13" goto clean
if "%CHOICE%"=="14" exit /b 0
echo [err]  invalid choice
goto menu


rem ---------------------------------------------------------------------------
rem Test, then commit, then push. The order is the point: a commit that skipped
rem the suite is a commit somebody else has to find.
:save
echo.
echo === save ===
call :checkcargo
if errorlevel 1 goto end
call :isrepo
if errorlevel 1 goto end

for /f %%i in ('git rev-list --count HEAD 2^>nul') do set COMMIT_COUNT=%%i
if not defined COMMIT_COUNT set COMMIT_COUNT=0
set /a NEXT_COUNT=%COMMIT_COUNT%+1
rem Same scheme as the other two projects: v0.NN until something is crowned a
rem major release, which nothing here has been.
for /f %%v in ('powershell -NoProfile -Command "'0.{0:D2}' -f %NEXT_COUNT%"') do set VERLABEL=%%v
echo bump: v%VERLABEL% (commit %NEXT_COUNT%)

call :runtests
if errorlevel 1 goto end

echo.
git add -A
if errorlevel 1 (
  echo [err]  git add
  set SAVE_ERROR=1
  goto end
)
git diff --cached --quiet
if not errorlevel 1 (
  echo [git]  nothing to commit
  goto end
)
echo.
git status --short
echo.
set "MSG="
set /p MSG=commit message (blank cancels):
if "%MSG%"=="" (
  echo [git]  cancelled; the changes are left staged
  set SAVE_ERROR=1
  goto end
)
call :clock TS
git commit -m "v%VERLABEL%: %MSG%"
if errorlevel 1 (
  echo [err]  git commit
  set SAVE_ERROR=1
  goto end
)
call :since TS "commit"
if "%COMMIT_ONLY%"=="1" (
  echo [git]  not pushing
  goto end
)
goto push


rem ---------------------------------------------------------------------------
:push
echo.
echo === push ===
call :isrepo
if errorlevel 1 goto end
call :branchname
git remote get-url origin >nul 2>&1
if errorlevel 1 (
  echo [skip] no `origin` remote. Add one with:
  echo        git remote add origin ^<url^>
  goto end
)
git push -u origin %BRANCH%
if not errorlevel 1 goto end
echo.
rem --force-with-lease, never a bare --force: it still overwrites
rem origin/%BRANCH%, but refuses if origin moved since the last fetch.
rem --force-if-includes rides with it everywhere, because the lease alone is
rem weaker than it reads -- it compares against the *local* remote-tracking ref,
rem and anything that fetches in the background (an IDE, a fetch in another
rem window, a periodic prefetch) advances that ref without showing anybody the
rem commits it just recorded. The lease is then satisfied by a state nobody has
rem looked at, which is exactly the clobber it was chosen to prevent.
rem --force-if-includes additionally requires that whatever is being overwritten
rem is already an ancestor of what is being pushed, so a fetch nobody read
rem cannot license the overwrite. Needs git 2.30 or newer; on an older git the
rem push is rejected outright rather than silently unprotected.
set "FORCE="
set /p FORCE=push failed. force-with-lease push? overwrites the remote unless it carries work you have not built on. (y/N):
if /i not "%FORCE%"=="y" (
  echo [git]  not forced
  set SAVE_ERROR=1
  goto end
)
git push -u origin %BRANCH% --force-with-lease --force-if-includes
if errorlevel 1 set SAVE_ERROR=1
goto end


rem ---------------------------------------------------------------------------
:pull
echo.
echo === pull ===
call :isrepo
if errorlevel 1 goto end
call :branchname
git remote get-url origin >nul 2>&1
if errorlevel 1 (
  echo [skip] no `origin` remote
  goto end
)
git pull origin %BRANCH%
if errorlevel 1 set SAVE_ERROR=1
goto end


rem ---------------------------------------------------------------------------
:test
echo.
echo === test ===
call :checkcargo
if errorlevel 1 goto end
call :runtests
goto end


rem The suite, as one routine, so `save` runs exactly what `test` runs. Two
rem copies of this is how a commit ends up having passed a different suite from
rem the one anybody else runs.
:runtests
rem Formatting and lints first: they are the fastest to fail and the cheapest to
rem fix, and a suite run before them just delays the same answer.
echo.
echo [chk]  cargo fmt
call :clock TS
cargo fmt --all -- --check
if errorlevel 1 (
  echo [err]  formatting differs -- run: cargo fmt --all
  set SAVE_ERROR=1
  exit /b 1
)
call :since TS "fmt"

echo.
echo [chk]  cargo clippy
call :clock TS
cargo clippy --all-targets --all-features -- -D warnings
if errorlevel 1 (
  echo [err]  clippy
  set SAVE_ERROR=1
  exit /b 1
)
call :since TS "clippy"

echo.
echo [chk]  cargo test --all   (TGA_REQUIRE_CORPUS=%TGA_REQUIRE_CORPUS%)
call :clock TS
rem --nocapture, because a corpus skip is an eprintln and libtest discards
rem stdout and stderr for a passing test. A skip nobody sees is how a suite
rem reports as coverage while having compared nothing.
cargo test --all -- --nocapture
if errorlevel 1 (
  echo [err]  tests
  set SAVE_ERROR=1
  exit /b 1
)
call :since TS "tests"
exit /b 0


rem ---------------------------------------------------------------------------
:build
echo.
echo === build ===
call :checkcargo
if errorlevel 1 goto end
call :clock TS
cargo build --release
if errorlevel 1 (
  echo [err]  build
  set SAVE_ERROR=1
  goto end
)
call :since TS "build"
echo.
for %%f in ("target\release\tga.exe" "target\release\TelegramAnalyser.exe") do (
  if exist "%%~f" (
    for /f "usebackq delims=" %%s in (`powershell -NoProfile -Command "'{0,7:N1} MB' -f ((Get-Item '%%~f').Length/1MB)"`) do echo [size] %%~nxf %%s
  )
)
goto end


rem ---------------------------------------------------------------------------
:runwindow
echo.
echo === run ===
call :checkcargo
if errorlevel 1 goto end
cargo run -p tga-app --bin TelegramAnalyser
if errorlevel 1 set SAVE_ERROR=1
goto end


rem ---------------------------------------------------------------------------
:report
call :askfolder "%~2"
if errorlevel 1 goto end
echo.
echo === report: %FOLDER% ===
call :checkcargo
if errorlevel 1 goto end
call :clock TS
cargo run --release -p tga-cli --bin tga -- "%FOLDER%" --quiet
if errorlevel 1 (
  echo [err]  report
  set SAVE_ERROR=1
  goto end
)
call :since TS "report"
if exist "%FOLDER%\report.html" (
  for /f "usebackq delims=" %%s in (`powershell -NoProfile -Command "'{0,7:N0} KB' -f ((Get-Item '%FOLDER%\report.html').Length/1KB)"`) do echo [size] report.html %%s
)
goto end


rem ---------------------------------------------------------------------------
:stats
call :askfolder "%~2"
if errorlevel 1 goto end
echo.
echo === stats: %FOLDER% ===
call :checkcargo
if errorlevel 1 goto end
if not exist "reference" mkdir "reference"
cargo run --release -p tga-cli --bin tga -- "%FOLDER%" --quiet --stats "reference\rust.stats.json" --out "reference\_stats-only.html"
if errorlevel 1 set SAVE_ERROR=1
goto end


rem ---------------------------------------------------------------------------
rem Re-record the Python side. Both dumps carry verbatim chat content, which is
rem why reference\ is gitignored -- see the note there.
:oracle
echo.
echo === oracle: re-record the Python analyser ===
if not exist "%PYEXE%" (
  echo [skip] no venv at %PYEXE%
  goto end
)
if not exist "reference" mkdir "reference"
rem The stamp has to be the same on both sides or the Notes line differs and
rem nothing else does. Today, on both, so a run that straddles midnight is the
rem only way to get it wrong -- and re-running fixes that.
for /f %%d in ('powershell -NoProfile -Command "(Get-Date).ToString('d MMMM yyyy',[Globalization.CultureInfo]::InvariantCulture)"') do set "STAMP=%%d"
echo [use]  stamp "%STAMP%"

call :onecorpus "ua-kolab" "%UAEXPORT%"
call :onecorpus "krgm" "%KRGMEXPORT%"
goto end

:onecorpus
if not exist "%~2" (
  echo [skip] %~1: no export at %~2
  exit /b 0
)
echo.
echo [rec]  %~1 stats
call :clock TS
"%PYEXE%" tools\dump_python_stats.py "%~2" "reference\%~1.stats.json"
if errorlevel 1 (set SAVE_ERROR=1 & exit /b 1)
call :since TS "%~1 stats"
echo [rec]  %~1 report
call :clock TS
"%PYEXE%" tools\dump_python_report.py "%~2" "reference\%~1.html" --stamp "%STAMP%"
if errorlevel 1 (set SAVE_ERROR=1 & exit /b 1)
call :since TS "%~1 report"
exit /b 0


rem ---------------------------------------------------------------------------
rem The HTML leg, from outside. `cargo test -p tga-report --test parity` is the
rem same comparison; what this adds is a readable account of *where* the two
rem differ, which an assert on a 1.5 MB string cannot give.
:parity
echo.
echo === parity: the report against the Python analyser's ===
call :checkcargo
if errorlevel 1 goto end
cargo build --release -p tga-cli
if errorlevel 1 (set SAVE_ERROR=1 & goto end)

call :oneparity "ua-kolab" "%UAEXPORT%"
call :oneparity "krgm" "%KRGMEXPORT%"
goto end

:oneparity
if not exist "reference\%~1.html" (
  echo [skip] %~1: no recorded Python report -- run: save.bat oracle
  exit /b 0
)
if not exist "%~2" (
  echo [skip] %~1: no export at %~2
  exit /b 0
)
echo.
echo [leg]  %~1
call :clock TS
rem **--classic.** The oracle is the document `report.py` writes, and `tga` now
rem writes the redesign by default. Without this flag the leg diffs the phase-5
rem report against the Python one and reports the whole redesign as a failure,
rem which is true and useless.
target\release\tga.exe "%~2" --quiet --classic --out "reference\rust-%~1.html"
if errorlevel 1 (set SAVE_ERROR=1 & exit /b 1)
python tools\diff_report.py "reference\%~1.html" "reference\rust-%~1.html"
if errorlevel 1 (
  echo [err]  %~1 differs
  set SAVE_ERROR=1
)
call :since TS "%~1"
exit /b 0


rem ---------------------------------------------------------------------------
:bless
echo.
echo === bless the golden ===
call :checkcargo
if errorlevel 1 goto end
set "TGA_BLESS=1"
cargo test -p tga-report --test golden -- --nocapture
set "TGA_BLESS="
if errorlevel 1 (
  echo [err]  bless
  set SAVE_ERROR=1
  goto end
)
echo.
echo [note] read the diff before committing it. A golden updated without being
echo        read is a golden that records whatever the bug did.
goto end


rem ---------------------------------------------------------------------------
rem Cargo never garbage-collects target\: a dep bump, a feature change or a
rem toolchain move gives an artifact a new metadata hash and the old one stays
rem for good. No profile setting reaches that; this is where it gets reported
rem and cleared. The exporter's tree reached 45 GB before anything measured it.
:clean
echo.
echo === clean ===
call :dirsize "target\debug" "target\debug  "
call :dirsize "target\release" "target\release"
echo.
set /p SURE=empty target\ ? [y/N]:
if /i not "%SURE%"=="y" (
  echo [skip] left alone
  goto end
)
call :clock TS
cargo clean
if errorlevel 1 (
  echo [err]  clean
  set SAVE_ERROR=1
  goto end
)
call :since TS "clean"
goto end


rem ---------------------------------------------------------------------------
rem helpers

rem :askfolder <folder or empty>  ->  FOLDER
rem
rem Offers the two known corpora by number, because typing a path with a Serbian
rem character in it into a cmd prompt is its own adventure.
:askfolder
set "FOLDER=%~1"
if not "%FOLDER%"=="" goto :askfolder_check
echo.
echo   1  %UAEXPORT%
echo   2  %KRGMEXPORT%
echo   or paste a folder
echo.
set /p FOLDER=export folder:
if "%FOLDER%"=="1" set "FOLDER=%UAEXPORT%"
if "%FOLDER%"=="2" set "FOLDER=%KRGMEXPORT%"
:askfolder_check
rem Explorer's "Copy as path" wraps in quotes and `set /p` keeps them.
set "FOLDER=%FOLDER:"=%"
if "%FOLDER%"=="" (
  echo [err]  no folder given
  set SAVE_ERROR=1
  exit /b 1
)
if not exist "%FOLDER%\" (
  echo [err]  not a folder: %FOLDER%
  set SAVE_ERROR=1
  exit /b 1
)
exit /b 0

:isrepo
git rev-parse --is-inside-work-tree >nul 2>&1
if errorlevel 1 (
  echo [err]  not a git repository. Start one with:  git init
  set SAVE_ERROR=1
  exit /b 1
)
exit /b 0

:branchname
for /f %%b in ('git rev-parse --abbrev-ref HEAD 2^>nul') do set BRANCH=%%b
if not defined BRANCH set BRANCH=main
exit /b 0

:checkcargo
where cargo >nul 2>&1
if errorlevel 1 (
  echo [err]  cargo is not on PATH
  set SAVE_ERROR=1
  exit /b 1
)
exit /b 0

:clock
for /f %%t in ('powershell -NoProfile -Command "[DateTimeOffset]::UtcNow.ToUnixTimeSeconds()"') do set %~1=%%t
exit /b 0

rem :since <clock var> <label>  ->  [time] <label> 4m 12s
:since
set CLKVAL=!%~1!
if not defined CLKVAL exit /b 0
rem usebackq + backticks, so the PowerShell can use single quotes: the plain
rem for /f form wraps the command in single quotes itself and there is no way to
rem escape one inside it. Floor, not [int]: PowerShell rounds to nearest on a
rem cast, so 752s printed as 13m 32s.
for /f "usebackq delims=" %%e in (`powershell -NoProfile -Command "$s=[DateTimeOffset]::UtcNow.ToUnixTimeSeconds()-!CLKVAL!; if($s -ge 60){'{0}m {1:D2}s' -f [math]::Floor($s/60),($s%%60)}else{'{0}s' -f $s}"`) do set TOOK=%%e
echo [time] %~2 !TOOK!
exit /b 0

rem :dirsize <path relative to the repo> <label>
rem
rem A missing directory prints "not built" rather than 0, because those are
rem different facts and only one of them means "already clean".
rem
rem **No pipe in the PowerShell.** `for /f` re-parses the backquoted string
rem through a second cmd and a `^|` does not survive both passes, which in the
rem exporter printed a perfectly formatted "0.00 GB, 0 files" for a 1.5 GB tree.
rem A size report that reads zero when it cannot measure is worse than none.
:dirsize
set "DSPATH=%~dp0%~1"
if not exist "%DSPATH%" (
  echo [size] %~2 not built
  exit /b 0
)
for /f "usebackq delims=" %%z in (`powershell -NoProfile -Command "$f=Get-ChildItem -LiteralPath '%DSPATH%' -Recurse -Force -File -ErrorAction SilentlyContinue; $b=0; foreach($x in $f){$b+=$x.Length}; '{0,8:N2} GB  {1,7:N0} files' -f ($b/1GB),$f.Count"`) do echo [size] %~2 %%z
exit /b 0


rem ---------------------------------------------------------------------------
rem One exit path, so every action reports its total and its status the same way.
:end
echo.
call :since T0 "total"
if "%SAVE_ERROR%"=="1" (
  echo [FAIL]
  exit /b 1
)
echo [ok]
exit /b 0

<#
.SYNOPSIS
  Fetches Mirrorly at a pinned commit, adds the keymap crate and applies the
  keymapping patch. Optionally builds it and stages a portable folder.

.DESCRIPTION
  Windows PowerShell 5.1 and PowerShell 7 compatible.

  What it does, in order:
    1. clone Mirrorly (shallow, exactly -Ref) into -Dest
    2. copy ..\crates\keymap into <Dest>\crates\keymap
    3. git apply --check, then git apply, integration\mirrorly-keymap.patch
    4. with -Build: get-ffmpeg.ps1, cargo test, cargo build --release,
       then stage dist\Mirrorly-keymap\ (FFmpeg DLLs from
       third_party\ffmpeg\bin, verified against packaging\ffmpeg.txt)
    5. with -Package: also run Mirrorly's own packaging\build-release.ps1
       (needs cargo-about and Inno Setup 6)
    6. with -StageOnly: only stage (the build was done by the caller, e.g. CI)

  Status: GitHub Actions run #1 (windows-latest) ran the clone, copy, patch
  and build steps of this script successfully; its staging step failed
  (no FFmpeg DLLs in target\release) and was rewritten (DLLs now come from
  third_party\ffmpeg\bin, SHA-256 checked). The rewritten staging has not
  been run yet. Nothing here has been run on a real Windows PC or an iPhone.

.PARAMETER Dest
  Where Mirrorly is checked out. Default: <ios-keymapping>\build\mirrorly.
.PARAMETER Ref
  Mirrorly commit to use. Default: the commit the patch was made against.
.PARAMETER Build
  Download FFmpeg, run tests, build Mirrorly.exe (release) and stage a folder.
.PARAMETER Package
  Also run Mirrorly's packaging\build-release.ps1 (installer + zip).
.PARAMETER StageOnly
  Skip clone/patch/build; stage the folder from an already built -Dest.
.PARAMETER Force
  Delete a non-empty -Dest first.
#>
param(
    [string]$Dest,
    [string]$Ref = 'ac70daa58ee5f24a067e80b6d0ffbcec4c735f1e',
    [switch]$Build,
    [switch]$Package,
    [switch]$StageOnly,
    [switch]$Force
)

$ErrorActionPreference = 'Stop'

$here = $PSScriptRoot                                  # ...\ios-keymapping\integration
$ios = Split-Path $here -Parent                        # ...\ios-keymapping
if (-not $Dest) { $Dest = Join-Path $ios 'build\mirrorly' }
$patch = Join-Path $here 'mirrorly-keymap.patch'
$crate = Join-Path $ios 'crates\keymap'
$upstreamUrl = 'https://github.com/AnasSAV/mirrorly.git'

# Run a native command; fail on a non-zero exit code. Native stderr output
# (git progress) must not become a PowerShell error, so relax the
# preference only around the call.
function Invoke-Native {
    param([string]$Exe, [string[]]$Arguments)
    $old = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        & $Exe @Arguments
        $code = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $old
    }
    if ($code -ne 0) { throw "$Exe $($Arguments -join ' ') failed with exit code $code" }
}

function Stage-Folder {
    param([string]$MirrorlyDir)
    $rel = Join-Path $MirrorlyDir 'target\release'
    $stage = Join-Path $ios 'dist\Mirrorly-keymap'
    if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
    New-Item -ItemType Directory -Force $stage | Out-Null
    if (-not (Test-Path (Join-Path $rel 'Mirrorly.exe'))) {
        throw "Mirrorly.exe not found in $rel (build first)"
    }
    Copy-Item (Join-Path $rel 'Mirrorly.exe') $stage
    # FFmpeg DLLs. Do NOT rely on target\release: upstream's build.rs copies
    # them there only on a best-effort basis (errors ignored), and CI run #1
    # found none. The pinned, hash-checked build lives in
    # <Mirrorly>\third_party\ffmpeg\bin (get-ffmpeg.ps1); target\release is
    # only a fallback when that directory does not exist.
    $ffbin = Join-Path $MirrorlyDir 'third_party\ffmpeg\bin'
    $dllDir = $ffbin
    if (-not (Test-Path $ffbin)) {
        Write-Warning "$ffbin not found; falling back to $rel for the FFmpeg DLLs"
        $dllDir = $rel
    }
    $pinFile = Join-Path $MirrorlyDir 'packaging\ffmpeg.txt'
    if (-not (Test-Path $pinFile)) { throw "pin file not found: $pinFile (cannot verify the FFmpeg DLLs)" }
    # Lines like: bin/avcodec-63.dll=<sha256>
    $pinned = @{}
    foreach ($line in (Get-Content $pinFile)) {
        if ($line -match '^bin/([^=\s]+)=([0-9a-fA-F]{64})\s*$') { $pinned[$Matches[1]] = $Matches[2].ToLower() }
    }
    foreach ($lib in 'avcodec', 'avutil', 'swresample') {
        $dll = @(Get-ChildItem (Join-Path $dllDir "$lib-*.dll") -ErrorAction SilentlyContinue)
        if ($dll.Count -ne 1) {
            throw "expected exactly one $lib DLL in $dllDir (searched $dllDir\$lib-*.dll), found $($dll.Count). Run packaging\get-ffmpeg.ps1 in $MirrorlyDir."
        }
        $name = $dll[0].Name
        $got = (Get-FileHash $dll[0].FullName -Algorithm SHA256).Hash.ToLower()
        if (-not $pinned.ContainsKey($name)) {
            throw "$name is not listed in $pinFile (expected a bin/$name=<sha256> line)"
        }
        if ($got -ne $pinned[$name]) {
            throw "$($dll[0].FullName) is not the pinned FFmpeg build (SHA-256 $got, pinned $($pinned[$name])); run packaging\get-ffmpeg.ps1 and rebuild"
        }
        Copy-Item $dll[0].FullName $stage
    }
    # Everything below is required: a staged folder without our licence,
    # attribution, sample profiles or editor must never be produced.
    $sim = Join-Path $rel 'keymap-sim.exe'
    if (-not (Test-Path $sim)) { throw "keymap-sim.exe not found in $rel (build it first)" }
    Copy-Item $sim $stage
    Copy-Item (Join-Path $MirrorlyDir 'LICENSE') (Join-Path $stage 'LICENSE-Mirrorly.txt')
    $lic = Join-Path $MirrorlyDir 'packaging\licenses'
    if (Test-Path $lic) {
        New-Item -ItemType Directory -Force (Join-Path $stage 'licenses') | Out-Null
        Copy-Item (Join-Path $lic '*.txt') (Join-Path $stage 'licenses')
    }
    # FFmpeg (LGPL v3) DLLs ship in this folder: add the notice upstream adds
    # (modeled on packaging\build-release.ps1).
    $pin = Join-Path $MirrorlyDir 'packaging\ffmpeg.txt'
    $ffurl = ''
    if (Test-Path $pin) {
        $m = @(Get-Content $pin | Where-Object { $_ -match '^url=' })
        if ($m.Count -gt 0) { $ffurl = $m[0] -replace '^url=', '' }
    }
    $dllNames = (Get-ChildItem (Join-Path $stage '*.dll') | ForEach-Object Name) -join ', '
    @"
FFmpeg in this folder
=====================

Mirrorly uses these FFmpeg libraries, unmodified, as separate DLLs:
  $dllNames

Download: $ffurl
License:  GNU Lesser General Public License, version 3 (licenses\LGPL-3.0.txt
          if present, which supplements the GNU GPL v3).
Binaries: BtbN FFmpeg-Builds, "lgpl-shared" variant
          https://github.com/BtbN/FFmpeg-Builds
Source:   https://github.com/FFmpeg/FFmpeg and the BtbN repository above.

You may replace these DLLs with your own build of the same FFmpeg version;
Mirrorly loads them by name from its own folder.

This portable folder does NOT contain THIRD-PARTY-NOTICES.txt (upstream's
cargo-about output); see the Mirrorly repository for the notices of the Rust
dependencies that are statically linked into Mirrorly.exe.
"@ | Set-Content -Encoding utf8 (Join-Path $stage 'FFMPEG-NOTICE.txt')
    foreach ($f in 'LICENSE', 'NOTICE', 'README.md') {
        $src = Join-Path $ios $f
        if (-not (Test-Path $src)) { throw "required file missing: $src" }
        Copy-Item $src $stage
    }
    $profiles = Join-Path $ios 'profiles'
    if (@(Get-ChildItem $profiles -Filter '*.json' -ErrorAction SilentlyContinue).Count -lt 1) {
        throw "no sample profiles (*.json) in $profiles"
    }
    Copy-Item $profiles (Join-Path $stage 'profiles') -Recurse
    $editor = Join-Path $ios 'editor\index.html'
    if (-not (Test-Path $editor)) { throw "required file missing: $editor" }
    New-Item -ItemType Directory -Force (Join-Path $stage 'editor') | Out-Null
    Copy-Item $editor (Join-Path $stage 'editor')
    # Double-click launchers, so nobody has to type a command line. ASCII
    # names and content: cmd.exe reads .bat files in the OEM code page.
    $launchers = [ordered]@{
        'Chay-keymap-doc.bat'   = "@echo off`r`nrem Mirrorly with the portrait touch-test profile.`r`nstart `"`" `"%~dp0Mirrorly.exe`" --keymap `"%~dp0profiles\touch-test-portrait.json`"`r`n"
        'Chay-keymap-ngang.bat' = "@echo off`r`nrem Mirrorly with the landscape touch-test profile.`r`nstart `"`" `"%~dp0Mirrorly.exe`" --keymap `"%~dp0profiles\touch-test-landscape.json`"`r`n"
        'Keo-ho-so-vao-day.bat' = "@echo off`r`nrem Drop a profile .json (for example one exported by editor\index.html) on this file.`r`nif `"%~1`"==`"`" (`r`n  echo Keo mot file ho so .json tha vao file nay de chay Mirrorly voi ho so do.`r`n  pause`r`n  exit /b 1`r`n)`r`nstart `"`" `"%~dp0Mirrorly.exe`" --keymap `"%~f1`"`r`n"
    }
    foreach ($name in $launchers.Keys) {
        [IO.File]::WriteAllText((Join-Path $stage $name), $launchers[$name], [Text.Encoding]::ASCII)
    }
    Write-Host "Staged: $stage"
}

# --- StageOnly: nothing to fetch or build -------------------------------------
if ($StageOnly) {
    if (-not (Test-Path $Dest)) { throw "-StageOnly needs an existing -Dest ($Dest)" }
    Stage-Folder $Dest
    return
}

# --- 1. checkout --------------------------------------------------------------
if (-not (Get-Command git -ErrorAction SilentlyContinue)) { throw 'git is required (https://git-scm.com)' }
if (-not (Test-Path $patch)) { throw "patch not found: $patch" }
if (-not (Test-Path $crate)) { throw "keymap crate not found: $crate" }

if ((Test-Path $Dest) -and (@(Get-ChildItem $Dest -Force).Count -gt 0)) {
    if (-not $Force) { throw "$Dest is not empty (use -Force to delete it)" }
    Remove-Item $Dest -Recurse -Force
}
New-Item -ItemType Directory -Force $Dest | Out-Null
$Dest = (Resolve-Path $Dest).Path

Push-Location $Dest
try {
    Invoke-Native git @('init', '-q')
    # Never let Git for Windows convert this clone to CRLF (autocrlf=true is
    # its default); the patch is applied as LF, see below.
    Invoke-Native git @('config', 'core.autocrlf', 'false')
    Invoke-Native git @('remote', 'add', 'origin', $upstreamUrl)
    Invoke-Native git @('fetch', '--depth', '1', 'origin', $Ref)
    Invoke-Native git @('-c', 'advice.detachedHead=false', 'checkout', '--detach', 'FETCH_HEAD')
    $head = (& git rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'git rev-parse failed' }
    if ($head -ne $Ref) { throw "checked out $head, expected $Ref" }

    # --- 2. keymap crate ------------------------------------------------------
    $target = Join-Path $Dest 'crates\keymap'
    New-Item -ItemType Directory -Force $target | Out-Null
    Copy-Item (Join-Path $crate '*') $target -Recurse -Force
    $stray = Join-Path $target 'target'
    if (Test-Path $stray) { Remove-Item $stray -Recurse -Force }

    # --- 3. patch ---------------------------------------------------------------
    # A Windows checkout of this repo may have turned the patch into CRLF
    # (core.autocrlf=true); the upstream tree here is LF, so apply an
    # LF-normalized copy.
    $patchToApply = $patch
    $bytes = [IO.File]::ReadAllBytes($patch)
    if ($bytes -contains 13) {
        $text = [Text.Encoding]::UTF8.GetString($bytes).Replace("`r`n", "`n")
        $patchToApply = Join-Path ([IO.Path]::GetTempPath()) ('mirrorly-keymap-' + [guid]::NewGuid().ToString('N') + '.patch')
        [IO.File]::WriteAllText($patchToApply, $text, (New-Object Text.UTF8Encoding($false)))
        Write-Host 'patch had CRLF line endings; applying an LF copy'
    }
    try {
        Invoke-Native git @('apply', '--check', $patchToApply)
        Invoke-Native git @('apply', $patchToApply)
    } finally {
        if ($patchToApply -ne $patch) { Remove-Item $patchToApply -Force -ErrorAction SilentlyContinue }
    }
    # Snapshot for CI: lets a later step show what cargo changed in Cargo.lock.
    Copy-Item 'Cargo.lock' 'Cargo.lock.patched' -Force
    Write-Host "Applied $patch to Mirrorly $($Ref.Substring(0, 7)) in $Dest"
} finally {
    Pop-Location
}

# --- 4. build -----------------------------------------------------------------
if ($Build) {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
        if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw 'cargo is required (https://rustup.rs, MSVC toolchain)' }
    }
    Push-Location $Dest
    try {
        if (-not (Test-Path 'third_party\ffmpeg')) {
            & powershell -NoProfile -ExecutionPolicy Bypass -File 'packaging\get-ffmpeg.ps1'
            if ($LASTEXITCODE -ne 0) { throw 'get-ffmpeg.ps1 failed' }
        }
        $env:PATH = "$Dest\third_party\ffmpeg\bin;$env:PATH"

        $steps = 'cargo test -p keymap -p input -p device && cargo build --release -p app && cargo build --release -p keymap --bin keymap-sim'
        if ($env:VCINSTALLDIR) {
            # Already inside a Developer prompt (or CI's msvc-dev-cmd step).
            & cmd /c $steps
        } else {
            $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
            $vcvars = $null
            if (Test-Path $vswhere) {
                $vcvars = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -find 'VC\Auxiliary\Build\vcvars64.bat' | Select-Object -First 1
            }
            if (-not $vcvars) {
                $vcvars = Get-ChildItem 'C:\Program Files*\Microsoft Visual Studio\*\*\VC\Auxiliary\Build\vcvars64.bat' -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty FullName
            }
            if (-not $vcvars) { throw 'Visual Studio C++ build tools (vcvars64.bat) not found' }
            & cmd /c "set PATH=C:\Program Files (x86)\Microsoft Visual Studio\Installer;%PATH% && `"$vcvars`" >nul && $steps"
        }
        if ($LASTEXITCODE -ne 0) { throw 'cargo test / build failed' }
    } finally {
        Pop-Location
    }
    Stage-Folder $Dest
}

# --- 5. Mirrorly's own installer + portable zip ---------------------------------
if ($Package) {
    Push-Location $Dest
    try {
        & powershell -NoProfile -ExecutionPolicy Bypass -File 'packaging\build-release.ps1'
        if ($LASTEXITCODE -ne 0) { throw 'packaging\build-release.ps1 failed (needs cargo-about and Inno Setup 6)' }
    } finally {
        Pop-Location
    }
}

Write-Host ''
Write-Host 'Next steps'
Write-Host "  Source:  $Dest"
if (-not $Build) {
    Write-Host '  Build:   re-run with -Build, or follow the README of Mirrorly (Rust MSVC, VS Build Tools, LLVM, FFmpeg).'
} else {
    Write-Host "  Run:     $ios\dist\Mirrorly-keymap\Mirrorly.exe --keymap $ios\profiles\touch-test-portrait.json"
    Write-Host '           (run the staged dist folder, not target\release\Mirrorly.exe: the FFmpeg DLLs are not there)'
}
Write-Host '  The keymap works over USB only, and nothing here has been tested on a real iPhone unless you did it.'

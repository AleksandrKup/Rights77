param([switch]$Run)
$ErrorActionPreference = 'Stop'
Push-Location $PSScriptRoot
$previousRustFlags = $env:RUSTFLAGS
$previousEncodedRustFlags = $env:CARGO_ENCODED_RUSTFLAGS
try {
    cargo fetch --locked
    if ($LASTEXITCODE -ne 0) { throw 'Не удалось получить зависимости' }
    $cargoHomeDirectory = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
    $nativeLibraryDirectory = Get-ChildItem (Join-Path $cargoHomeDirectory 'registry\src') -Directory |
        ForEach-Object { Join-Path $_.FullName 'windows_x86_64_msvc-0.48.5\lib' } |
        Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
    if (!$nativeLibraryDirectory) { throw 'Не найдена библиотека windows-targets 0.48.5' }
    $env:CARGO_ENCODED_RUSTFLAGS = @('-C', 'target-feature=+crt-static', '-L', "native=$nativeLibraryDirectory") -join [char]31
    # The Win7 target has no precompiled std: nightly + rust-src are required.
    cargo +nightly build --locked -Z build-std=std,panic_abort --release --target x86_64-win7-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'Не удалось собрать Rights77' }
    New-Item -ItemType Directory -Path (Join-Path $PSScriptRoot 'dist') -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'target\x86_64-win7-windows-msvc\release\rights77.exe') -Destination (Join-Path $PSScriptRoot 'dist\Rights77.exe') -Force
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'README.md') -Destination (Join-Path $PSScriptRoot 'dist\README.md') -Force
    Write-Host 'Готово: dist\Rights77.exe (Windows 7 SP1 / 10 / 11, x64, один EXE)'
    if ($Run) { Start-Process -FilePath (Join-Path $PSScriptRoot 'dist\Rights77.exe') }
} finally {
    $env:RUSTFLAGS = $previousRustFlags
    $env:CARGO_ENCODED_RUSTFLAGS = $previousEncodedRustFlags
    Pop-Location
}

param(
    [string]$TargetTriple = $(if ($env:TAURI_ENV_TARGET_TRIPLE) { $env:TAURI_ENV_TARGET_TRIPLE } else { 'x86_64-pc-windows-msvc' })
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

if ($TargetTriple -ne 'x86_64-pc-windows-msvc') {
    throw "Windows binary fetch currently supports x86_64-pc-windows-msvc only: $TargetTriple"
}

$root = Split-Path -Parent $PSScriptRoot
$binDir = Join-Path $root 'src-tauri\binaries'
$cacheDir = Join-Path $root 'src-tauri\resources\yt-dlp-cache'
New-Item -ItemType Directory -Force -Path $binDir, $cacheDir | Out-Null

$ytDlp = Join-Path $binDir "yt-dlp-$TargetTriple.exe"
$ffmpeg = Join-Path $binDir "ffmpeg-$TargetTriple.exe"
$deno = Join-Path $binDir "deno-$TargetTriple.exe"
$denoVersion = '2.6.6'

function Test-Executable([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { return $false }
    $file = Get-Item -LiteralPath $Path
    if ($file.Length -lt 1MB) { return $false }
    $stream = [System.IO.File]::OpenRead($Path)
    try {
        return ($stream.ReadByte() -eq 0x4D -and $stream.ReadByte() -eq 0x5A)
    } finally {
        $stream.Dispose()
    }
}

function Receive-File([string]$Url, [string]$Destination) {
    $partial = "$Destination.partial"
    for ($attempt = 1; $attempt -le 3; $attempt++) {
        try {
            Write-Host "Downloading $Url (attempt $attempt/3)"
            Invoke-WebRequest -Uri $Url -OutFile $partial -MaximumRedirection 10 -UseBasicParsing
            Move-Item -LiteralPath $partial -Destination $Destination -Force
            return
        } catch {
            Remove-Item -LiteralPath $partial -Force -ErrorAction SilentlyContinue
            if ($attempt -eq 3) { throw }
            Start-Sleep -Seconds (2 * $attempt)
        }
    }
}

function Remove-ExtractionDirectory([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path)) { return }
    $resolvedBinDir = [System.IO.Path]::GetFullPath($binDir).TrimEnd('\') + '\'
    $resolvedTarget = [System.IO.Path]::GetFullPath($Path).TrimEnd('\')
    if (-not $resolvedTarget.StartsWith($resolvedBinDir, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to remove a directory outside the binary staging area: $resolvedTarget"
    }
    Remove-Item -LiteralPath $resolvedTarget -Recurse -Force
}

function Assert-Executable([string]$Path, [string]$Name, [string]$VersionArgument = '--version') {
    if (-not (Test-Executable $Path)) {
        throw "$Name is missing or is not a valid Windows executable: $Path"
    }
    $versionOutput = & $Path $VersionArgument 2>&1
    $exitCode = $LASTEXITCODE
    if ($exitCode -ne 0) { throw "$Name failed its version check: $Path ($versionOutput)" }
    Write-Host ($versionOutput | Select-Object -First 1)
}

if ($env:FORCE -eq '1' -or -not (Test-Executable $ytDlp)) {
    Receive-File 'https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe' $ytDlp
}
Assert-Executable $ytDlp 'yt-dlp'

if ($env:FORCE -eq '1' -or -not (Test-Executable $ffmpeg)) {
    $archive = Join-Path $binDir 'ffmpeg-release-essentials.zip'
    $extractDir = Join-Path $binDir 'ffmpeg-extracted'
    try {
        Receive-File 'https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip' $archive
        Remove-ExtractionDirectory $extractDir
        Expand-Archive -LiteralPath $archive -DestinationPath $extractDir -Force
        $source = Get-ChildItem -LiteralPath $extractDir -Filter 'ffmpeg.exe' -Recurse -File |
            Where-Object { $_.DirectoryName -match '[\\/]bin$' } |
            Select-Object -First 1
        if (-not $source) { throw 'ffmpeg.exe was not found in the downloaded archive' }
        Copy-Item -LiteralPath $source.FullName -Destination $ffmpeg -Force
    } finally {
        Remove-Item -LiteralPath $archive -Force -ErrorAction SilentlyContinue
        Remove-ExtractionDirectory $extractDir
    }
}
Assert-Executable $ffmpeg 'ffmpeg' '-version'

if ($env:FORCE -eq '1' -or -not (Test-Executable $deno)) {
    $archive = Join-Path $binDir 'deno-windows.zip'
    $extractDir = Join-Path $binDir 'deno-extracted'
    try {
        Receive-File "https://github.com/denoland/deno/releases/download/v$denoVersion/deno-x86_64-pc-windows-msvc.zip" $archive
        Remove-ExtractionDirectory $extractDir
        Expand-Archive -LiteralPath $archive -DestinationPath $extractDir -Force
        $source = Join-Path $extractDir 'deno.exe'
        if (-not (Test-Path -LiteralPath $source)) { throw 'deno.exe was not found in the downloaded archive' }
        Copy-Item -LiteralPath $source -Destination $deno -Force
    } finally {
        Remove-Item -LiteralPath $archive -Force -ErrorAction SilentlyContinue
        Remove-ExtractionDirectory $extractDir
    }
}
Assert-Executable $deno 'deno'

Write-Host 'Windows sidecars are ready for the Tauri bundle.'

# Downloads FFmpeg/FFprobe and whisper-cli static binaries (run this before `bun tauri build`)

$binDir = Join-Path $PSScriptRoot "src-tauri\binaries"

if (-not (Test-Path $binDir)) {
    New-Item -ItemType Directory -Path $binDir -Force | Out-Null
}

# ── FFmpeg ───────────────────────────────────────────────────────────────
$ffmpegZipUrl = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip"
$ffmpegZipPath = Join-Path $binDir "ffmpeg.zip"

if (-not (Test-Path (Join-Path $binDir "ffmpeg.exe"))) {
    Write-Host "Downloading FFmpeg essentials build..." -ForegroundColor Cyan
    Invoke-WebRequest -Uri $ffmpegZipUrl -OutFile $ffmpegZipPath -UserAgent "Mozilla/5.0"

    Write-Host "Extracting..." -ForegroundColor Cyan
    Expand-Archive -Path $ffmpegZipPath -DestinationPath $binDir -Force

    $extracted = Get-ChildItem -Path $binDir -Directory | Where-Object { $_.Name -like "ffmpeg-*" } | Select-Object -First 1
    if ($extracted) {
        Copy-Item -Path "$($extracted.FullName)\bin\ffmpeg.exe" -Destination $binDir -Force
        Copy-Item -Path "$($extracted.FullName)\bin\ffprobe.exe" -Destination $binDir -Force
        Remove-Item -Path $extracted.FullName -Recurse -Force
    }
    Remove-Item -Path $ffmpegZipPath -Force

    Write-Host "FFmpeg binaries ready!" -ForegroundColor Green
} else {
    Write-Host "FFmpeg already present, skipping." -ForegroundColor DarkGray
}

# ── whisper-cli ──────────────────────────────────────────────────────────
$whisperUrl = "https://github.com/ggml-org/whisper.cpp/releases/latest/download/whisper-bin-x64.zip"
$whisperZipPath = Join-Path $binDir "whisper.zip"

if (-not (Test-Path (Join-Path $binDir "whisper-cli.exe"))) {
    Write-Host "Downloading whisper-cli..." -ForegroundColor Cyan
    Invoke-WebRequest -Uri $whisperUrl -OutFile $whisperZipPath -UserAgent "Mozilla/5.0"

    Write-Host "Extracting..." -ForegroundColor Cyan
    Expand-Archive -Path $whisperZipPath -DestinationPath $binDir -Force

    # The release zip typically contains a subfolder; move contents up
    $whisperExtracted = Get-ChildItem -Path $binDir -Directory | Where-Object { $_.Name -like "whisper*" -or $_.Name -like "whisper.cpp*" } | Select-Object -First 1
    if ($whisperExtracted) {
        Get-ChildItem -Path $whisperExtracted.FullName -File | ForEach-Object {
            Copy-Item -Path $_.FullName -Destination $binDir -Force
        }
        Remove-Item -Path $whisperExtracted.FullName -Recurse -Force
    }
    Remove-Item -Path $whisperZipPath -Force

    Write-Host "whisper-cli binary ready!" -ForegroundColor Green
} else {
    Write-Host "whisper-cli already present, skipping." -ForegroundColor DarkGray
}

Write-Host "`nAll binaries ready in $binDir" -ForegroundColor Green

$ErrorActionPreference = "Stop"

$Repo = "orbit-omen/orbitpixel"
$BinName = "orbitpixel.exe"
$InstallDir = if ($env:ORBITPIXEL_INSTALL_DIR) { $env:ORBITPIXEL_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "OrbitPixel\bin" }

Write-Host "[orbitpixel] Preparing installation..."

if (-not [Environment]::Is64BitOperatingSystem) {
    throw "[orbitpixel] Only 64-bit Windows (x86_64) is supported."
}

$Target = "x86_64-pc-windows-msvc"
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

$TempDir = Join-Path ([System.IO.Path]::GetTempPath()) ("orbitpixel-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $TempDir | Out-Null

try {
    $Release = $null
    try {
        $Release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -Headers @{ "User-Agent" = "OrbitPixel-Installer" }
    } catch {
        $Release = $null
    }

    if ($null -eq $Release -or [string]::IsNullOrWhiteSpace($Release.tag_name)) {
        Write-Host "[orbitpixel] No published GitHub release found yet. Falling back to cargo install..."
        if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
            throw "[orbitpixel] Neither a release binary nor 'cargo' was found. Install Rust from https://rustup.rs and retry."
        }
        cargo install --git "https://github.com/$Repo" --branch main
        Write-Host "[orbitpixel] Installed via cargo."
        exit 0
    }

    $Tag = $Release.tag_name
    $ArchiveName = "orbitpixel-$Target.zip"
    $DownloadUrl = "https://github.com/$Repo/releases/download/$Tag/$ArchiveName"
    $ZipPath = Join-Path $TempDir $ArchiveName

    Write-Host "[orbitpixel] Downloading $Tag ($Target)..."
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $ZipPath
    Expand-Archive -Path $ZipPath -DestinationPath $TempDir -Force

    $ExtractedBin = Join-Path $TempDir $BinName
    if (-not (Test-Path $ExtractedBin)) {
        throw "[orbitpixel] Expected binary $BinName not found in archive."
    }

    Move-Item -Force -Path $ExtractedBin -Destination (Join-Path $InstallDir $BinName)
    Write-Host "[orbitpixel] Installed to $(Join-Path $InstallDir $BinName)"

    $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($UserPath -notlike "*$InstallDir*") {
        [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
        Write-Host "[orbitpixel] Added $InstallDir to User PATH. Restart your terminal to run 'orbitpixel'."
    }
} finally {
    if (Test-Path $TempDir) {
        Remove-Item -Recurse -Force $TempDir
    }
}

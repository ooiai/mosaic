param(
    [string]$Version = "",
    [string]$InstallDir = "$env:LOCALAPPDATA\mosaic\bin",
    [switch]$FromSource
)

$ErrorActionPreference = "Stop"
$Repo = "ooiai/mosaic"
$GitUrl = "https://github.com/$Repo.git"

function Resolve-LatestVersion {
    $apiUrl = "https://api.github.com/repos/$Repo/releases/latest"
    try {
        $response = Invoke-RestMethod -Uri $apiUrl -Method Get
    } catch {
        return $null
    }
    if (-not $response -or -not $response.tag_name) {
        return $null
    }
    return $response.tag_name
}

function Install-FromSource {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        throw "cargo not found. Install Rust first: https://rustup.rs/"
    }

    $cargoRoot = Join-Path $tempRoot "cargo-root"
    New-Item -ItemType Directory -Path $cargoRoot -Force | Out-Null

    $args = @("install", "--git", $GitUrl, "--locked", "--force", "--root", $cargoRoot)
    if ($Version) {
        $args += @("--tag", $Version)
    }
    $args += "mosaic-cli"
    Write-Host "Installing mosaic from source"
    & cargo @args
    if ($LASTEXITCODE -ne 0) {
        throw "cargo install failed with exit code $LASTEXITCODE"
    }

    $sourceExe = Join-Path $cargoRoot "bin\mosaic.exe"
    if (-not (Test-Path $sourceExe)) {
        throw "cargo install completed but mosaic.exe was not found"
    }

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Copy-Item -Path $sourceExe -Destination (Join-Path $InstallDir "mosaic.exe") -Force
}

function Install-FromRelease {
    if (-not $Version) {
        $Version = Resolve-LatestVersion
    }
    if (-not $Version) {
        return $false
    }

    $platform = "windows-x64"
    $asset = "mosaic-$Version-$platform.zip"
    $url = "https://github.com/$Repo/releases/download/$Version/$asset"
    $zipPath = Join-Path $tempRoot $asset

    Write-Host "Installing mosaic $Version ($platform)"
    Write-Host "Download: $url"
    try {
        Invoke-WebRequest -Uri $url -OutFile $zipPath
    } catch {
        return $false
    }

    Expand-Archive -Path $zipPath -DestinationPath $tempRoot -Force
    $sourceExe = Join-Path $tempRoot ("mosaic-$Version-$platform\mosaic.exe")
    if (-not (Test-Path $sourceExe)) {
        return $false
    }

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Copy-Item -Path $sourceExe -Destination (Join-Path $InstallDir "mosaic.exe") -Force
    return $true
}

$tempRoot = Join-Path $env:TEMP ("mosaic-install-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $tempRoot | Out-Null

try {
    $installed = $false
    if ($FromSource) {
        Install-FromSource
        $installed = $true
    } else {
        $installed = Install-FromRelease
        if (-not $installed) {
            Write-Warning "Release asset install unavailable; falling back to source build."
            Install-FromSource
            $installed = $true
        }
    }

    if (-not $installed) {
        throw "Install failed"
    }

    Write-Host "Installed: $InstallDir\mosaic.exe"
    $currentUserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if (-not $currentUserPath) {
        $currentUserPath = ""
    }
    if (-not ($currentUserPath -split ";" | Where-Object { $_ -eq $InstallDir })) {
        $nextPath = if ($currentUserPath) { "$currentUserPath;$InstallDir" } else { $InstallDir }
        [Environment]::SetEnvironmentVariable("Path", $nextPath, "User")
        Write-Host "Added to user PATH: $InstallDir"
        Write-Host "Restart terminal to use 'mosaic' directly."
    } else {
        Write-Host "Install directory already in PATH."
    }

    Write-Host "Verify:"
    Write-Host "  mosaic --help"
}
finally {
    if (Test-Path $tempRoot) {
        Remove-Item -Path $tempRoot -Recurse -Force
    }
}

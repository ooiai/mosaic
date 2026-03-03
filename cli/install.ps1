param(
    [string]$Version = "",
    [string]$InstallDir = "$env:LOCALAPPDATA\mosaic\bin"
)

$ErrorActionPreference = "Stop"
$Repo = "ooiai/mosaic"

function Resolve-LatestVersion {
    $apiUrl = "https://api.github.com/repos/$Repo/releases/latest"
    $response = Invoke-RestMethod -Uri $apiUrl -Method Get
    if (-not $response.tag_name) {
        throw "Failed to resolve latest release tag from $apiUrl"
    }
    return $response.tag_name
}

if (-not $Version) {
    $Version = Resolve-LatestVersion
}

$platform = "windows-x64"
$asset = "mosaic-$Version-$platform.zip"
$url = "https://github.com/$Repo/releases/download/$Version/$asset"

$tempRoot = Join-Path $env:TEMP ("mosaic-install-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $tempRoot | Out-Null

try {
    $zipPath = Join-Path $tempRoot $asset
    Write-Host "Installing mosaic $Version ($platform)"
    Write-Host "Download: $url"
    Invoke-WebRequest -Uri $url -OutFile $zipPath

    Expand-Archive -Path $zipPath -DestinationPath $tempRoot -Force
    $sourceExe = Join-Path $tempRoot ("mosaic-$Version-$platform\mosaic.exe")
    if (-not (Test-Path $sourceExe)) {
        throw "Extracted package does not contain mosaic.exe"
    }

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Copy-Item -Path $sourceExe -Destination (Join-Path $InstallDir "mosaic.exe") -Force

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

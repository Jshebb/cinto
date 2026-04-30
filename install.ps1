#Requires -Version 5.1

$ErrorActionPreference = "Stop"

$repo = "Jshebb/cinto"
$version = "latest"
$target = "x86_64-pc-windows-msvc"

$binDir = [Environment]::ExpandEnvironmentVariables("%USERPROFILE%\.local\bin")
$dataDir = [Environment]::ExpandEnvironmentVariables("%USERPROFILE%\.local\share\cinto")

Write-Host "Installing cinto for Windows..."

$asset = "cinto-$target.tar.gz"

if ($version -eq "latest") {
    $url = "https://github.com/$repo/releases/latest/download/$asset"
} else {
    $url = "https://github.com/$repo/releases/download/$version/$asset"
}

$tmpDir = Join-Path ([IO.Path]::GetTempPath()) "cinto-install-$(New-Guid)"
New-Item -ItemType Directory -Force -Path $tmpDir | Out-Null

try {
    $archivePath = Join-Path $tmpDir $asset
    $checksumPath = Join-Path $tmpDir "$asset.sha256"

    Write-Host "Downloading $url..."
    Invoke-WebRequest -Uri $url -OutFile $archivePath -UseBasicParsing
    Invoke-WebRequest -Uri "$url.sha256" -OutFile $checksumPath -UseBasicParsing

    $checksumLine = Get-Content $checksumPath -TotalCount 1
    $expectedHash = ($checksumLine -split '\s+')[0].Trim().ToUpperInvariant()
    $actualHash = (Get-FileHash -Path $archivePath -Algorithm SHA256).Hash

    if ($expectedHash -ne $actualHash) {
        Write-Error "Checksum mismatch! Expected: $expectedHash, Actual: $actualHash"
    }

    $extractDir = Join-Path $tmpDir "extract"
    New-Item -ItemType Directory -Force -Path $extractDir | Out-Null
    
    # Path traversal and tar bomb protection
    $entries = tar -tzf $archivePath
    foreach ($entry in $entries) {
        if ($entry.Trim() -notin @("cinto", "cinto.exe")) {
            Write-Error "Release archive contains unexpected path: $entry"
        }
    }

    Write-Host "Extracting..."
    tar -xzf $archivePath -C $extractDir

    $binary = Join-Path $extractDir "cinto.exe"
    if (-not (Test-Path $binary)) {
        Write-Error "Release archive did not contain cinto.exe"
    }

    New-Item -ItemType Directory -Force -Path $binDir | Out-Null
    Copy-Item -Path $binary -Destination (Join-Path $binDir "cinto.exe") -Force

    New-Item -ItemType Directory -Force -Path $dataDir | Out-Null
    $installInfo = @"
binary=$binDir\cinto.exe
repo=$repo
version=$version
target=$target
"@
    Set-Content -Path (Join-Path $dataDir "install.toml") -Value $installInfo

    Write-Host "Installed cinto to $binDir\cinto.exe"

    $userPath = [Environment]::GetEnvironmentVariable("PATH", "User")
    if ($userPath -notlike "*$binDir*") {
        Write-Host "Adding $binDir to user PATH..."
        $newPath = "$binDir;$userPath".Trim(";")
        [Environment]::SetEnvironmentVariable("PATH", $newPath, "User")
        $env:PATH = "$binDir;$env:PATH"
        Write-Host "PATH updated. You can now use cinto from your terminal."
    } else {
        Write-Host "cinto is already on your PATH."
    }
} finally {
    Remove-Item -Path $tmpDir -Recurse -Force -ErrorAction Ignore
}

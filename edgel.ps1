$ErrorActionPreference = "Stop"

$Version = if ($env:EDGEL_VERSION) { $env:EDGEL_VERSION } else { "v0.1.0" }
$BaseUrl = if ($env:EDGEL_RELEASE_BASE_URL) {
  $env:EDGEL_RELEASE_BASE_URL
} else {
  "https://github.com/josephwere/EDGEL/releases/download/$Version"
}
$InstallDir = if ($env:EDGEL_INSTALL_DIR) {
  $env:EDGEL_INSTALL_DIR
} else {
  Join-Path $HOME "AppData\Local\Programs\EDGEL\bin"
}

function Get-Target {
  $arch = switch ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()) {
    "X64" { "x86_64" }
    "Arm64" { "aarch64" }
    default { throw "Unsupported Windows architecture: $_" }
  }

  return "$arch-pc-windows-msvc"
}

$Target = Get-Target
$Archive = "edgel-$Version-$Target.zip"
$TempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("edgel-" + [guid]::NewGuid())
$ArchivePath = Join-Path $TempRoot $Archive
$ExtractPath = Join-Path $TempRoot "stage"

New-Item -ItemType Directory -Force -Path $TempRoot | Out-Null
New-Item -ItemType Directory -Force -Path $ExtractPath | Out-Null

try {
  Write-Host "Downloading EDGEL $Version for $Target"
  Invoke-WebRequest -Uri "$BaseUrl/$Archive" -OutFile $ArchivePath
  Expand-Archive -Path $ArchivePath -DestinationPath $ExtractPath -Force

  $StageDir = Join-Path $ExtractPath "edgel-$Version-$Target"
  if (-not (Test-Path $StageDir)) {
    throw "Release archive layout is invalid: $StageDir missing"
  }

  New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
  Copy-Item -Force (Join-Path $StageDir "bin\edgel.exe") (Join-Path $InstallDir "edgel.exe")
  Copy-Item -Force (Join-Path $StageDir "bin\goldedge-browser.exe") (Join-Path $InstallDir "goldedge-browser.exe")

  Write-Host "Installed:"
  Write-Host " - $(Join-Path $InstallDir 'edgel.exe')"
  Write-Host " - $(Join-Path $InstallDir 'goldedge-browser.exe')"
  Write-Host ""
  Write-Host "Next steps:"
  Write-Host "  edgel new my-app"
  Write-Host "  cd my-app"
  Write-Host "  edgel run"
} finally {
  if (Test-Path $TempRoot) {
    Remove-Item -Recurse -Force $TempRoot
  }
}

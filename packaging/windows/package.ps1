# Package the Windows build: the app, the CLI (the agents' MCP bridge) and the licences in a zip.
#   powershell -File packaging/windows/package.ps1   ->  dist\SYSTM-Cut-<version>-windows.zip
$ErrorActionPreference = "Stop"
$root = Resolve-Path "$PSScriptRoot\..\.."
$version = if ($env:SYSTM_CUT_VERSION) { $env:SYSTM_CUT_VERSION } else { "0.1.0" }
$stage = Join-Path $root "dist\SYSTM Cut"
Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item (Join-Path $root "app\target\release\systmcut.exe") (Join-Path $stage "SYSTM Cut.exe")
Copy-Item (Join-Path $root "app\target\release\systmcut-cli.exe") $stage
foreach ($f in "LICENSE-MIT", "LICENSE-APACHE", "NOTICE", "ATTRIBUTION.md") { Copy-Item (Join-Path $root "app\$f") $stage -ErrorAction SilentlyContinue }
$zip = Join-Path $root "dist\SYSTM-Cut-$version-windows.zip"
Remove-Item $zip -ErrorAction SilentlyContinue
Compress-Archive -Path $stage -DestinationPath $zip
Write-Output $zip

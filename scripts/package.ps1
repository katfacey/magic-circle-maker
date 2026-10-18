$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
$version=((Get-Content Cargo.toml | Select-String '^version\s*=\s*"([^"]+)"').Matches.Groups[1].Value)
if(-not $version){throw 'Package version not found'}
$binary=Join-Path $projectRoot 'target\release\magic-circle-maker.exe'
if(-not(Test-Path -LiteralPath $binary)){throw 'Build the release executable first'}
& (Join-Path $PSScriptRoot 'collect-licenses.ps1')
$releaseRoot=Join-Path $projectRoot 'release'
$packageName="MagicCircleMaker-$version-windows-x64"
$packageRoot=Join-Path $releaseRoot $packageName
New-Item -ItemType Directory -Path $packageRoot -Force | Out-Null
Copy-Item -LiteralPath $binary -Destination (Join-Path $packageRoot 'MagicCircleMaker.exe') -Force
foreach($file in @('LICENSE','README.md','THIRD_PARTY_NOTICES.md')){Copy-Item -LiteralPath $file -Destination $packageRoot -Force}
New-Item -ItemType Directory -Path (Join-Path $packageRoot 'assets\fonts') -Force | Out-Null
Get-ChildItem assets\fonts -File | Where-Object {$_.Extension -in @('.txt','.md')} | Copy-Item -Destination (Join-Path $packageRoot 'assets\fonts') -Force
New-Item -ItemType Directory -Path (Join-Path $packageRoot 'docs') -Force | Out-Null
Copy-Item -LiteralPath docs\licenses -Destination (Join-Path $packageRoot 'docs') -Recurse -Force
Copy-Item -LiteralPath docs\dependency-licenses.json,docs\obsidian.png,docs\ivory.png -Destination (Join-Path $packageRoot 'docs') -Force
Compress-Archive -LiteralPath $packageRoot -DestinationPath (Join-Path $releaseRoot "$packageName.zip") -Force
$exeName="MagicCircleMaker-$version-windows-x64.exe"
Copy-Item -LiteralPath $binary -Destination (Join-Path $releaseRoot $exeName) -Force
$lines=foreach($file in @("$packageName.zip",$exeName)){(Get-FileHash -LiteralPath (Join-Path $releaseRoot $file) -Algorithm SHA256).Hash.ToLower()+'  '+$file}
[IO.File]::WriteAllLines((Join-Path $releaseRoot 'SHA256SUMS.txt'),$lines)
Write-Output "Packaged $packageName"

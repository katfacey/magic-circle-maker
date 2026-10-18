$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
if(Test-Path .tooling\cargo\bin\cargo.exe){$env:CARGO_HOME=Join-Path $projectRoot '.tooling\cargo';$env:RUSTUP_HOME=Join-Path $projectRoot '.tooling\rustup';$env:PATH=(Join-Path $env:CARGO_HOME 'bin')+';'+$env:PATH}
$metadata=cargo metadata --locked --offline --filter-platform x86_64-pc-windows-msvc --format-version 1 | ConvertFrom-Json
if($LASTEXITCODE -ne 0){throw 'cargo metadata failed'}
$ids=@{}; foreach($node in $metadata.resolve.nodes){$ids[$node.id]=$true}
$packages=$metadata.packages | Where-Object {$ids.ContainsKey($_.id) -and $_.name -ne 'magic-circle-maker'} | Sort-Object name,version
$packages | Select-Object name,version,license,repository | ConvertTo-Json -Depth 4 | Set-Content -Encoding utf8 docs\dependency-licenses.json
$noticeRoot=Join-Path $projectRoot 'docs\licenses'
New-Item -ItemType Directory -Path $noticeRoot -Force | Out-Null
foreach($package in $packages){
 $sourceRoot=Split-Path -Parent $package.manifest_path
 $destination=Join-Path $noticeRoot ($package.name+'-'+$package.version)
 $files=Get-ChildItem -LiteralPath $sourceRoot -File -Recurse | Where-Object {($_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE|OFL|UFL)' -or ($package.name -eq 'epaint_default_fonts' -and $_.Extension -eq '.txt')) -and $_.FullName -notmatch '\\(tests|testdata)\\'}
 foreach($file in $files){$relative=[IO.Path]::GetRelativePath($sourceRoot,$file.FullName);$target=Join-Path $destination $relative;New-Item -ItemType Directory -Path (Split-Path -Parent $target) -Force | Out-Null;Copy-Item -LiteralPath $file.FullName -Destination $target}
 if($package.name -eq 'option-ext'){New-Item -ItemType Directory -Path (Join-Path $destination 'source') -Force | Out-Null;Copy-Item -LiteralPath (Join-Path $sourceRoot 'src') -Destination (Join-Path $destination 'source') -Recurse -Force;foreach($name in @('Cargo.toml','README.md','LICENSE.txt')){Copy-Item -LiteralPath (Join-Path $sourceRoot $name) -Destination (Join-Path $destination 'source') -Force}}
}
Write-Output "Collected notices for $($packages.Count) locked Windows dependencies."

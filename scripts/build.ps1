param([switch]$Test)
$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
$localCargo=Join-Path $projectRoot '.tooling\cargo'
if(Test-Path -LiteralPath $localCargo){$env:CARGO_HOME=$localCargo;$env:RUSTUP_HOME=Join-Path $projectRoot '.tooling\rustup';$env:PATH=(Join-Path $localCargo 'bin')+';'+$env:PATH}
$toolchainRoot=$env:MCM_MSVC_ROOT
if(-not $toolchainRoot){
 $vswhere=Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
 if(Test-Path -LiteralPath $vswhere){$library=& $vswhere -products '*' -find 'VC\Tools\MSVC\*\lib\x64\libcmt.lib' | Where-Object {Test-Path -LiteralPath $_} | Select-Object -Last 1;if($library){$toolchainRoot=Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $library))}}
}
if($toolchainRoot -and (Test-Path -LiteralPath $toolchainRoot)){
 $env:PATH=(Join-Path $toolchainRoot 'bin\Hostx64\x64')+';'+$env:PATH
 $sdk='C:\Program Files (x86)\Windows Kits\10'
 $sdkVersion=Get-ChildItem -LiteralPath (Join-Path $sdk 'Lib') -Directory | Sort-Object Name -Descending | Select-Object -First 1 -ExpandProperty Name
 $env:LIB=(Join-Path $toolchainRoot 'lib\x64')+';'+(Join-Path $sdk "Lib\$sdkVersion\ucrt\x64")+';'+(Join-Path $sdk "Lib\$sdkVersion\um\x64")+';'+$env:LIB
}
if($Test){cargo test --locked;if($LASTEXITCODE -ne 0){exit $LASTEXITCODE}}
cargo build --release --locked
exit $LASTEXITCODE

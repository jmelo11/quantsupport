[CmdletBinding()]
param(
    [ValidateSet("debug", "release")]
    [string]$Profile = "release"
)

$ErrorActionPreference = "Stop"

if (-not [Environment]::Is64BitProcess) {
    throw "Build the QuantSupport XLL from a 64-bit PowerShell process."
}

$excelRoot = Split-Path -Parent $PSScriptRoot
$repositoryRoot = (Resolve-Path (Join-Path $excelRoot "..\..")).Path
$targetTriple = "x86_64-pc-windows-msvc"
$cargoArguments = @(
    "build",
    "--package", "quantsupport-xll",
    "--target", $targetTriple
)
if ($Profile -eq "release") {
    $cargoArguments += "--release"
}

Push-Location $repositoryRoot
try {
    & cargo @cargoArguments
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build failed with exit code $LASTEXITCODE"
    }
}
finally {
    Pop-Location
}

$source = Join-Path $repositoryRoot "target\$targetTriple\$Profile\quantsupport_xll.dll"
if (-not (Test-Path -LiteralPath $source)) {
    throw "The expected XLL library was not produced: $source"
}

$dist = Join-Path $excelRoot "dist"
New-Item -ItemType Directory -Path $dist -Force | Out-Null
$destination = Join-Path $dist "quantsupport.xll"
Copy-Item -LiteralPath $source -Destination $destination -Force

$intelliSense = Join-Path $dist "quantsupport.intellisense.xml"
$metadataArguments = @(
    "run",
    "--manifest-path", (Join-Path $excelRoot "Cargo.toml"),
    "--package", "quantsupport-xll",
    "--target", $targetTriple,
    "--example", "function_catalog"
)
if ($Profile -eq "release") {
    $metadataArguments += "--release"
}
$metadataArguments += @("--", "--intellisense", $intelliSense)
& cargo @metadataArguments
if ($LASTEXITCODE -ne 0) {
    throw "IntelliSense metadata generation failed with exit code $LASTEXITCODE"
}

$pdb = [IO.Path]::ChangeExtension($source, ".pdb")
if (Test-Path -LiteralPath $pdb) {
    Copy-Item -LiteralPath $pdb -Destination (Join-Path $dist "quantsupport.pdb") -Force
}

Write-Host "Packaged QuantSupport XLL: $destination"
Write-Host "Packaged parameter hints: $intelliSense"
Write-Host "For inline hints, load ExcelDna.IntelliSense64.xll alongside QuantSupport."

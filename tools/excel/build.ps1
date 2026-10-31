[CmdletBinding()]
param(
    [ValidateSet("debug", "release")]
    [string]$Profile = "release",
    [string]$Dist
)

$ErrorActionPreference = "Stop"

if (-not (Get-Command dotnet -ErrorAction SilentlyContinue)) {
    throw "Install the .NET 8 SDK or later to build the QuantSupport ribbon companion."
}

if (-not [Environment]::Is64BitProcess) {
    throw "Build the QuantSupport XLL from a 64-bit PowerShell process."
}

$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path
$excelRoot = Join-Path $repositoryRoot "bindings/excel"
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

if (-not $Dist) { $Dist = Join-Path $excelRoot "dist" }
$dist = [IO.Path]::GetFullPath($Dist)
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
$configuration = if ($Profile -eq "release") { "Release" } else { "Debug" }
& dotnet build (Join-Path $excelRoot "ribbon\QuantSupport.Ribbon.csproj") --configuration $configuration
if ($LASTEXITCODE -ne 0) {
    throw "Ribbon companion build failed with exit code $LASTEXITCODE"
}
$ribbon = Join-Path $excelRoot "ribbon\bin\$configuration\net462\publish\QuantSupport.Ribbon64.xll"
if (-not (Test-Path -LiteralPath $ribbon -PathType Leaf)) {
    throw "The ribbon build did not produce the packed companion: $ribbon"
}
Copy-Item -LiteralPath $ribbon -Destination (Join-Path $dist "QuantSupport.Ribbon64.xll") -Force
& (Join-Path $PSScriptRoot "package.ps1") -Dist $dist

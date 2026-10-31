[CmdletBinding()]
param(
    [string]$Dist = (Join-Path $PSScriptRoot "../../bindings/excel/dist"),
    [string]$IntelliSensePath
)

$ErrorActionPreference = "Stop"
$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path
$excelRoot = Join-Path $repositoryRoot "bindings/excel"
$thirdParty = Join-Path $excelRoot "third-party"
$dependency = Get-Content -LiteralPath (Join-Path $thirdParty "intellisense.json") -Raw | ConvertFrom-Json
$companionName = "ExcelDna.IntelliSense64.xll"
$archiveName = "quantsupport-excel-windows-x64.zip"

foreach ($name in @("quantsupport.xll", "quantsupport.intellisense.xml", "QuantSupport.Ribbon64.xll")) {
    $file = Join-Path $Dist $name
    if (-not (Test-Path -LiteralPath $file -PathType Leaf) -or (Get-Item -LiteralPath $file).Length -eq 0) {
        throw "Missing build output: $file"
    }
}
$Dist = (Resolve-Path -LiteralPath $Dist).Path
$temporary = Join-Path ([IO.Path]::GetTempPath()) ("quantsupport-package-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $temporary | Out-Null
try {
    $companion = Join-Path $temporary $companionName
    if ($IntelliSensePath) {
        Copy-Item -LiteralPath $IntelliSensePath -Destination $companion
    }
    else {
        $cached = Join-Path $Dist $companionName
        if ((Test-Path -LiteralPath $cached -PathType Leaf) -and
            (Get-FileHash -LiteralPath $cached -Algorithm SHA256).Hash -eq $dependency.sha256) {
            Copy-Item -LiteralPath $cached -Destination $companion
        }
        else {
            Invoke-WebRequest -Uri $dependency.url -OutFile $companion -UseBasicParsing
        }
    }
    if ((Get-FileHash -LiteralPath $companion -Algorithm SHA256).Hash -ne $dependency.sha256) {
        throw "Excel-DNA IntelliSense $($dependency.version) checksum mismatch."
    }

    $files = @($companion)
    foreach ($name in @("quantsupport.xll", "quantsupport.intellisense.xml", "QuantSupport.Ribbon64.xll")) {
        $target = Join-Path $temporary $name
        Copy-Item -LiteralPath (Join-Path $Dist $name) -Destination $target
        $files += $target
    }
    foreach ($entry in @(
        @{ Source = (Join-Path $excelRoot "INSTALL.txt"); Name = "INSTALL.txt" },
        @{ Source = (Join-Path $repositoryRoot "LICENSE"); Name = "QuantSupport-LICENSE.txt" },
        @{ Source = (Join-Path $thirdParty "ExcelDna.IntelliSense-LICENSE.txt"); Name = "ExcelDna.IntelliSense-LICENSE.txt" },
        @{ Source = (Join-Path $thirdParty "ExcelDna-LICENSE.txt"); Name = "ExcelDna-LICENSE.txt" },
        @{ Source = (Join-Path $excelRoot "ribbon/Icons/LICENSE.txt"); Name = "MaterialSymbols-LICENSE.txt" },
        @{ Source = (Join-Path $excelRoot "ribbon/Icons/NOTICE.txt"); Name = "MaterialSymbols-NOTICE.txt" }
    )) {
        $target = Join-Path $temporary $entry.Name
        Copy-Item -LiteralPath $entry.Source -Destination $target
        $files += $target
    }
    $archive = Join-Path $temporary $archiveName
    Compress-Archive -LiteralPath $files -DestinationPath $archive -CompressionLevel Optimal
    Copy-Item -LiteralPath $companion -Destination (Join-Path $Dist $companionName) -Force
    Copy-Item -LiteralPath $archive -Destination (Join-Path $Dist $archiveName) -Force
}
finally {
    $resolvedTemporary = (Resolve-Path -LiteralPath $temporary).Path
    $temporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd([IO.Path]::DirectorySeparatorChar)
    if (-not [string]::Equals([IO.Path]::GetDirectoryName($resolvedTemporary), $temporaryRoot, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to remove a packaging directory outside the temporary folder: $resolvedTemporary"
    }
    Remove-Item -LiteralPath $temporary -Recurse -Force
}
Write-Host "Packaged Excel add-in, ribbon and IntelliSense: $(Join-Path $Dist $archiveName)"

[CmdletBinding()]
param(
    [string]$OutputPath,
    [string]$XllPath
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path $PSScriptRoot "swap_pricing_example.xlsx"
}
if ([string]::IsNullOrWhiteSpace($XllPath)) {
    $XllPath = Join-Path (Split-Path -Parent $PSScriptRoot) "dist\quantsupport.xll"
}

function Set-TitleStyle {
    param($Range)
    $Range.Font.Bold = $true
    $Range.Font.Size = 18
    $Range.Font.Color = 0xFFFFFF
    $Range.Interior.Color = 0x703000
    $Range.RowHeight = 28
}

function Set-HeaderStyle {
    param($Range)
    $Range.Font.Bold = $true
    $Range.Font.Color = 0xFFFFFF
    $Range.Interior.Color = 0xA05A2C
    $Range.Borders.LineStyle = 1
}

function Set-SectionStyle {
    param($Range)
    $Range.Font.Bold = $true
    $Range.Font.Color = 0x703000
    $Range.Interior.Color = 0xE8D9CF
}

$outputDirectory = Split-Path -Parent $OutputPath
New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
$OutputPath = [IO.Path]::GetFullPath($OutputPath)
$XllPath = [IO.Path]::GetFullPath($XllPath)
$savePath = $OutputPath
if (Test-Path -LiteralPath $OutputPath) {
    $outputName = [IO.Path]::GetFileNameWithoutExtension($OutputPath)
    $savePath = Join-Path ([IO.Path]::GetTempPath()) "$outputName.generated.$([Guid]::NewGuid().ToString('N')).xlsx"
}
$saveSucceeded = $false

$excel = $null
$workbook = $null
try {
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false
    $workbook = $excel.Workbooks.Add()
    $excel.Calculation = -4135 # xlCalculationManual
    $decimalSeparator = $excel.International(3) # xlDecimalSeparator
    $thousandsSeparator = $excel.International(4) # xlThousandsSeparator
    $numberFormat = "#$thousandsSeparator##0$decimalSeparator" + "00"
    $percentFormat = "0$decimalSeparator" + "0000%"
    $factorFormat = "0$decimalSeparator" + "000000"

    if (Test-Path -LiteralPath $XllPath) {
        if (-not $excel.RegisterXLL($XllPath)) {
            throw "Excel could not register the QuantSupport XLL at $XllPath"
        }
    }

    while ($workbook.Worksheets.Count -lt 7) {
        $null = $workbook.Worksheets.Add()
    }
    while ($workbook.Worksheets.Count -gt 7) {
        $workbook.Worksheets.Item($workbook.Worksheets.Count).Delete()
    }

    $readme = $workbook.Worksheets.Item(1)
    $curve = $workbook.Worksheets.Item(2)
    $inputs = $workbook.Worksheets.Item(3)
    $pricing = $workbook.Worksheets.Item(4)
    $curveResults = $workbook.Worksheets.Item(5)
    $outputs = $workbook.Worksheets.Item(6)
    $help = $workbook.Worksheets.Item(7)
    $readme.Name = "README"
    $curve.Name = "Curve Setup"
    $inputs.Name = "Trade Inputs"
    $pricing.Name = "Pricing"
    $curveResults.Name = "Curve Results"
    $outputs.Name = "Outputs"
    $help.Name = "Function Help"

    $readme.Range("A1:F1").Merge()
    $readme.Range("A1").Value2 = "QuantSupport: USD SOFR Swap Pricing"
    Set-TitleStyle $readme.Range("A1:F1")
    $readme.Range("A3").Value2 = "How to use this workbook"
    Set-SectionStyle $readme.Range("A3:F3")
    $instructions = @(
        "1. Build and load bindings/excel/dist/quantsupport.xll in 64-bit Excel.",
        "2. Open this workbook after the add-in is loaded, then press Ctrl+Alt+F9 once.",
        "3. Change the blue cells on Curve Setup or Trade Inputs. The curve, context, price, cashflows, and risk recalculate.",
        "4. Pricing shows the complete object chain. Curve Results shows bootstrapped discount factors and forwards. Outputs shows cashflows and sensitivities."
    )
    for ($index = 0; $index -lt $instructions.Count; $index++) {
        $readme.Cells.Item(4 + $index, 1).Value2 = $instructions[$index]
    }
    $readme.Range("A9").Value2 = "The curve is configured in this workbook"
    Set-SectionStyle $readme.Range("A9:F9")
    $readme.Range("A10").Value2 = "Curve Setup contains the valuation date, deposit/OIS market quotes, interpolation, day count, extrapolation, and selected pillar range."
    $readme.Range("A11").Value2 = "Each QS.QUOTES.SET row returns a revised QuoteStore handle. QS.CURVE.CONFIGS.ADD then reads the visible identifier range A6:A13."
    $readme.Range("A12").Value2 = "QS.CONTEXT.CREATE bootstraps the SOFR curve from those two handles. No JSON or external curve file is used."
    $readme.Range("A13").Value2 = "Pressing F9 with unchanged inputs reuses every object handle. Only edited quotes or trade inputs create a new revision."
    $readme.Range("A14").Value2 = "Function guidance"
    Set-SectionStyle $readme.Range("A14:F14")
    $readme.Range("A15").Value2 = "Excel's Function Arguments dialog shows guidance for every parameter."
    $readme.Range("A16").Value2 = 'Use =QS.HELP("QS.CURVE.CONFIGS.ADD") or =QS.FUNCTIONS.DETAILS() for an in-sheet reference.'
    $readme.Columns.Item("A").ColumnWidth = 115
    $readme.Range("A1:A16").WrapText = $true

    $curve.Range("A1:F1").Merge()
    $curve.Range("A1").Value2 = "USD SOFR curve setup"
    Set-TitleStyle $curve.Range("A1:F1")
    $curve.Range("A3").Value2 = "Valuation date"
    $curve.Range("B3").NumberFormat = "@"
    $curve.Range("B3").Value2 = "2026-02-24"
    $curve.Range("E3").Value2 = "Initial quote store"
    $curve.Range("F3").Formula2 = '=QS.QUOTES.CREATE("SwapExample.Quotes",B3)'
    $curve.Range("A5").Value2 = "Quote identifier / calibration instrument"
    $curve.Range("B5").Value2 = "Mid"
    $curve.Range("C5").Value2 = "QuoteStore revision"
    Set-HeaderStyle $curve.Range("A5:C5")
    $curveQuotes = @(
        [pscustomobject]@{ Identifier = "FixedRateDeposit_USD_SOFR_1D"; Mid = [double]0.04500 }
        [pscustomobject]@{ Identifier = "OIS_USD_SOFR_1Y"; Mid = [double]0.04837 }
        [pscustomobject]@{ Identifier = "OIS_USD_SOFR_2Y"; Mid = [double]0.04528 }
        [pscustomobject]@{ Identifier = "OIS_USD_SOFR_3Y"; Mid = [double]0.04260 }
        [pscustomobject]@{ Identifier = "OIS_USD_SOFR_5Y"; Mid = [double]0.04077 }
        [pscustomobject]@{ Identifier = "OIS_USD_SOFR_7Y"; Mid = [double]0.04033 }
        [pscustomobject]@{ Identifier = "OIS_USD_SOFR_10Y"; Mid = [double]0.03898 }
        [pscustomobject]@{ Identifier = "OIS_USD_SOFR_30Y"; Mid = [double]0.03738 }
    )
    for ($index = 0; $index -lt $curveQuotes.Count; $index++) {
        $row = 6 + $index
        $curve.Cells.Item($row, 1).Value2 = $curveQuotes[$index].Identifier
        $rateText = $curveQuotes[$index].Mid.ToString([Globalization.CultureInfo]::InvariantCulture)
        $curve.Cells.Item($row, 2).Formula2 = "=$rateText"
        if ($index -eq 0) {
            $curve.Cells.Item($row, 3).Formula2 = "=QS.QUOTES.SET(`$F`$3,A$row,B$row,,)"
        }
        else {
            $previousRow = $row - 1
            $curve.Cells.Item($row, 3).Formula2 = "=QS.QUOTES.SET(C$previousRow,A$row,B$row,,)"
        }
    }
    $curve.Range("E5").Value2 = "Curve parameter"
    $curve.Range("F5").Value2 = "Value / handle"
    Set-HeaderStyle $curve.Range("E5:F5")
    $curve.Range("E6").Value2 = "Market index"
    $curve.Range("F6").Value2 = "SOFR"
    $curve.Range("E7").Value2 = "Day counter"
    $curve.Range("F7").Value2 = "Actual360"
    $curve.Range("E8").Value2 = "Interpolator"
    $curve.Range("F8").Value2 = "LogLinear"
    $curve.Range("E9").Value2 = "Enable extrapolation"
    $curve.Range("F9").Formula2 = "=TRUE"
    $curve.Range("E10").Value2 = "Configuration store"
    $curve.Range("F10").Formula2 = '=QS.CURVE.CONFIGS.CREATE("SwapExample.Curves")'
    $curve.Range("E11").Value2 = "Configured SOFR curve"
    $curve.Range("F11").Formula2 = "=QS.CURVE.CONFIGS.ADD(F10,F6,F7,F8,F9,A6:A13)"
    $curve.Range("A3:B3").Borders.LineStyle = 1
    $curve.Range("A5:C13").Borders.LineStyle = 1
    $curve.Range("E5:F11").Borders.LineStyle = 1
    $curve.Range("B3").Interior.Color = 0xFCE4D6
    $curve.Range("B6:B13").Interior.Color = 0xFCE4D6
    $curve.Range("F6:F9").Interior.Color = 0xFCE4D6
    $curve.Range("B6:B13").NumberFormatLocal = $percentFormat
    $curve.Columns.Item("A").ColumnWidth = 42
    $curve.Columns.Item("B").ColumnWidth = 14
    $curve.Columns.Item("C").ColumnWidth = 48
    $curve.Columns.Item("D").ColumnWidth = 3
    $curve.Columns.Item("E").ColumnWidth = 24
    $curve.Columns.Item("F").ColumnWidth = 50
    $curve.Range("A:F").WrapText = $true

    $inputs.Range("A1:D1").Merge()
    $inputs.Range("A1").Value2 = "Swap inputs"
    Set-TitleStyle $inputs.Range("A1:D1")
    $inputs.Range("A3").Value2 = "Parameter"
    $inputs.Range("B3").Value2 = "Value"
    Set-HeaderStyle $inputs.Range("A3:B3")
    $inputs.Range("B6:B7").NumberFormat = "@"
    $inputRows = @(
        [pscustomobject]@{ Parameter = "Notional"; Value = [double]10000000 }
        [pscustomobject]@{ Parameter = "Fixed coupon"; Value = [double]0.0425 }
        [pscustomobject]@{ Parameter = "Start date"; Value = "2026-02-24" }
        [pscustomobject]@{ Parameter = "Maturity date"; Value = "2031-02-24" }
        [pscustomobject]@{ Parameter = "Currency"; Value = "USD" }
        [pscustomobject]@{ Parameter = "Market index"; Value = "SOFR" }
        [pscustomobject]@{ Parameter = "Side"; Value = "Receive" }
        [pscustomobject]@{ Parameter = "Day counter"; Value = "Actual360" }
        [pscustomobject]@{ Parameter = "Coupon compounding"; Value = "Simple" }
        [pscustomobject]@{ Parameter = "Rate frequency"; Value = "Semiannual" }
        [pscustomobject]@{ Parameter = "Fixed-leg frequency"; Value = "Semiannual" }
        [pscustomobject]@{ Parameter = "Floating-leg frequency"; Value = "Quarterly" }
    )
    for ($index = 0; $index -lt $inputRows.Count; $index++) {
        $row = 4 + $index
        $inputs.Cells.Item($row, 1).Value2 = $inputRows[$index].Parameter
        if ($inputRows[$index].Value -is [double]) {
            $numericText = $inputRows[$index].Value.ToString([Globalization.CultureInfo]::InvariantCulture)
            $inputs.Cells.Item($row, 2).Formula2 = "=$numericText"
        }
        else {
            $inputs.Cells.Item($row, 2).Value2 = $inputRows[$index].Value
        }
    }
    $inputs.Range("B4:B15").Interior.Color = 0xFCE4D6
    $inputs.Range("B4").NumberFormatLocal = $numberFormat
    $inputs.Range("B5").NumberFormatLocal = $percentFormat
    $inputs.Range("A3:B15").Borders.LineStyle = 1
    $inputs.Columns.Item("A").ColumnWidth = 27
    $inputs.Columns.Item("B").ColumnWidth = 23
    $inputs.Range("D3").Value2 = "Rates are decimal values: 0.03 = 3%. Dates are ISO text (YYYY-MM-DD)."
    $inputs.Range("D3").WrapText = $true
    $inputs.Columns.Item("D").ColumnWidth = 42

    $pricing.Range("A1:D1").Merge()
    $pricing.Range("A1").Value2 = "Pricing chain"
    Set-TitleStyle $pricing.Range("A1:D1")
    $pricing.Range("A3").Value2 = "Step"
    $pricing.Range("B3").Value2 = "Result"
    $pricing.Range("C3").Value2 = "Purpose"
    Set-HeaderStyle $pricing.Range("A3:C3")
    $pricingSteps = @(
        [pscustomobject]@{ Step = "1. Quote store"; Formula = "='Curve Setup'!C13"; Purpose = "Latest revision after loading all visible market quotes" }
        [pscustomobject]@{ Step = "2. Curve configuration"; Formula = "='Curve Setup'!F11"; Purpose = "SOFR, Actual360, LogLinear, extrapolation, and quote-pillar range" }
        [pscustomobject]@{ Step = "3. Pricing context"; Formula = '=QS.CONTEXT.CREATE("SwapExample.Context",B4,B5,,,,,,,,,"USD","SOFR")'; Purpose = "Bootstraps the SOFR discount/forward curve" }
        [pscustomobject]@{ Step = "4. Swap trade"; Formula = '=QS.TRADE.SWAP("SwapExample.Swap",''Curve Setup''!B3,''Trade Inputs''!B6,''Trade Inputs''!B7,''Trade Inputs''!B8,''Trade Inputs''!B9,''Trade Inputs''!B4,''Trade Inputs''!B5,0,''Trade Inputs''!B10,''Trade Inputs''!B11,''Trade Inputs''!B12,''Trade Inputs''!B13,''Trade Inputs''!B14,''Trade Inputs''!B15)'; Purpose = "Creates a receive-fixed USD SOFR swap" }
        [pscustomobject]@{ Step = "5. Present value"; Formula = "=QS.PRICE(B7,B6)"; Purpose = "Swap NPV in USD" }
        [pscustomobject]@{ Step = "6. Fair fixed rate"; Formula = "=QS.FAIR.RATE(B7,B6)"; Purpose = "Par coupon implied by the bootstrapped curve" }
        [pscustomobject]@{ Step = "7. Discount curve"; Formula = "=QS.CURVE.GET(B6,'Trade Inputs'!B9)"; Purpose = "Reusable handle for the bootstrapped SOFR curve" }
        [pscustomobject]@{ Step = "8. Maturity discount"; Formula = "=QS.CURVE.DISCOUNT(B10,'Trade Inputs'!B7)"; Purpose = "Bootstrapped discount factor at swap maturity" }
    )
    for ($index = 0; $index -lt $pricingSteps.Count; $index++) {
        $row = 4 + $index
        $pricing.Cells.Item($row, 1).Value2 = $pricingSteps[$index].Step
        $pricing.Cells.Item($row, 2).Formula2 = $pricingSteps[$index].Formula
        $pricing.Cells.Item($row, 3).Value2 = $pricingSteps[$index].Purpose
    }
    $pricing.Range("A3:C11").Borders.LineStyle = 1
    $pricing.Range("B8").NumberFormatLocal = $numberFormat
    $pricing.Range("B9").NumberFormatLocal = $percentFormat
    $pricing.Range("B11").NumberFormatLocal = $factorFormat
    $pricing.Columns.Item("A").ColumnWidth = 24
    $pricing.Columns.Item("B").ColumnWidth = 58
    $pricing.Columns.Item("C").ColumnWidth = 40
    $pricing.Range("B4:B11").WrapText = $true

    $curveResults.Range("A1:H1").Merge()
    $curveResults.Range("A1").Value2 = "Bootstrapped SOFR discount curve"
    Set-TitleStyle $curveResults.Range("A1:H1")
    $curveResults.Range("A3").Value2 = "Curve nodes"
    Set-SectionStyle $curveResults.Range("A3:E3")
    $curveResults.Range("A4").Formula2 = '=QS.CURVE.TABLE(Pricing!B10,"Simple","Annual")'
    $curveResults.Range("C5:C200").NumberFormatLocal = $factorFormat
    $curveResults.Range("D5:E200").NumberFormatLocal = $percentFormat
    $curveResults.Columns.Item("A").ColumnWidth = 38
    $curveResults.Columns.Item("B").ColumnWidth = 16
    $curveResults.Columns.Item("C:E").ColumnWidth = 20

    $curveResults.Range("G3").Value2 = "Point queries"
    Set-SectionStyle $curveResults.Range("G3:H3")
    $curveResults.Range("G4").Value2 = "Curve handle"
    $curveResults.Range("H4").Formula2 = "=Pricing!B10"
    $curveResults.Range("G5").Value2 = "Discount date"
    $curveResults.Range("H5").Formula2 = "='Trade Inputs'!B7"
    $curveResults.Range("H5").NumberFormat = "@"
    $curveResults.Range("G6").Value2 = "Discount factor"
    $curveResults.Range("H6").Formula2 = "=QS.CURVE.DISCOUNT(Pricing!B10,H5)"
    $curveResults.Range("H6").NumberFormatLocal = $factorFormat
    $curveResults.Range("G7").Value2 = "Forward start"
    $curveResults.Range("H7").Formula2 = "='Trade Inputs'!B6"
    $curveResults.Range("H7").NumberFormat = "@"
    $curveResults.Range("G8").Value2 = "Forward end"
    $curveResults.Range("H8").Formula2 = "='Trade Inputs'!B7"
    $curveResults.Range("H8").NumberFormat = "@"
    $curveResults.Range("G9").Value2 = "Forward rate"
    $curveResults.Range("H9").Formula2 = '=QS.CURVE.FORWARD(Pricing!B10,H7,H8,"Simple","Annual")'
    $curveResults.Range("H9").NumberFormatLocal = $percentFormat
    $curveResults.Range("G4:H9").Borders.LineStyle = 1
    $curveResults.Columns.Item("G").ColumnWidth = 20
    $curveResults.Columns.Item("H").ColumnWidth = 58
    $curveResults.Range("H4").WrapText = $true

    $outputs.Range("A1:J1").Merge()
    $outputs.Range("A1").Value2 = "Swap outputs"
    Set-TitleStyle $outputs.Range("A1:J1")
    $outputs.Range("A3").Value2 = "Cashflows"
    Set-SectionStyle $outputs.Range("A3:E3")
    $outputs.Range("A4").Formula2 = "=QS.CASHFLOWS(Pricing!B7,Pricing!B6)"
    $outputs.Range("G3").Value2 = "Sensitivities"
    Set-SectionStyle $outputs.Range("G3:J3")
    $outputs.Range("G4").Formula2 = "=QS.SENSITIVITIES(Pricing!B7,Pricing!B6)"
    $outputs.Columns.Item("A:J").ColumnWidth = 18

    $help.Range("A1:H1").Merge()
    $help.Range("A1").Value2 = "Live worksheet-function help"
    Set-TitleStyle $help.Range("A1:H1")
    $help.Range("A3").Value2 = "Curve construction"
    Set-SectionStyle $help.Range("A3:B3")
    $help.Range("A4").Formula2 = '=QS.HELP("QS.CURVE.CONFIGS.ADD")'
    $help.Range("D3").Value2 = "Swap construction"
    Set-SectionStyle $help.Range("D3:E3")
    $help.Range("D4").Formula2 = '=QS.HELP("QS.TRADE.SWAP")'
    $help.Range("G3").Value2 = "Curve inspection"
    Set-SectionStyle $help.Range("G3:H3")
    $help.Range("G4").Formula2 = '=QS.HELP("QS.CURVE.TABLE")'
    $help.Columns.Item("A").ColumnWidth = 25
    $help.Columns.Item("B").ColumnWidth = 76
    $help.Columns.Item("D").ColumnWidth = 25
    $help.Columns.Item("E").ColumnWidth = 76
    $help.Columns.Item("G").ColumnWidth = 25
    $help.Columns.Item("H").ColumnWidth = 76
    $help.Range("A:H").WrapText = $true

    foreach ($sheet in @($readme, $curve, $inputs, $pricing, $curveResults, $outputs, $help)) {
        $sheet.Activate()
        $excel.ActiveWindow.DisplayGridlines = $false
    }
    $pricing.Activate()
    $pricing.Range("A4").Select() | Out-Null

    $workbook.ForceFullCalculation = $true
    $excel.Calculation = -4105 # xlCalculationAutomatic
    if (Test-Path -LiteralPath $XllPath) {
        $excel.CalculateFullRebuild()
        $initialQuotes = $curve.Range("F3").Value2
        Write-Verbose "Initial quote store => $initialQuotes ($($initialQuotes.GetType().FullName))"
        for ($row = 6; $row -le 13; $row++) {
            $quoteFormula = $curve.Cells.Item($row, 3).Formula2
            $quoteValue = $curve.Cells.Item($row, 3).Value2
            Write-Verbose "Quote row ${row}: $quoteFormula => $quoteValue ($($quoteValue.GetType().FullName))"
        }
        for ($row = 4; $row -le 11; $row++) {
            $stepName = $pricing.Cells.Item($row, 1).Value2
            $stepFormula = $pricing.Cells.Item($row, 2).Formula2
            $stepValue = $pricing.Cells.Item($row, 2).Value2
            Write-Verbose "${stepName}: $stepFormula => $stepValue ($($stepValue.GetType().FullName))"
        }
        $handleCells = @(
            [pscustomobject]@{ Name = "initial quote store"; Cell = $curve.Range("F3") }
            [pscustomobject]@{ Name = "final quote store"; Cell = $curve.Range("C13") }
            [pscustomobject]@{ Name = "configuration store"; Cell = $curve.Range("F10") }
            [pscustomobject]@{ Name = "configured curve"; Cell = $curve.Range("F11") }
            [pscustomobject]@{ Name = "pricing quote store"; Cell = $pricing.Range("B4") }
            [pscustomobject]@{ Name = "pricing curve configuration"; Cell = $pricing.Range("B5") }
            [pscustomobject]@{ Name = "pricing context"; Cell = $pricing.Range("B6") }
            [pscustomobject]@{ Name = "swap trade"; Cell = $pricing.Range("B7") }
            [pscustomobject]@{ Name = "discount curve"; Cell = $pricing.Range("B10") }
        )
        $handlesBeforeF9 = @{}
        foreach ($entry in $handleCells) {
            $handlesBeforeF9[$entry.Name] = [string]$entry.Cell.Value2
        }
        $priceBeforeF9 = [double]$pricing.Range("B8").Value2
        $excel.Calculate() # equivalent to an unchanged F9 recalculation
        foreach ($entry in $handleCells) {
            $handleAfterF9 = [string]$entry.Cell.Value2
            if ($handleAfterF9 -ne $handlesBeforeF9[$entry.Name]) {
                throw "F9 changed the $($entry.Name) handle despite unchanged inputs: $($handlesBeforeF9[$entry.Name]) -> $handleAfterF9"
            }
        }
        $priceAfterF9 = [double]$pricing.Range("B8").Value2
        if ([Math]::Abs($priceAfterF9 - $priceBeforeF9) -gt 0.00000001) {
            throw "F9 changed the swap price despite unchanged inputs: $priceBeforeF9 -> $priceAfterF9"
        }
        $price = $pricing.Range("B8").Value2
        $fairRate = $pricing.Range("B9").Value2
        $discountFactor = $pricing.Range("B11").Value2
        $curveTableReferenceDf = $curveResults.Range("C5").Value2
        $curveTableFirstForward = $curveResults.Range("E6").Value2
        $curvePointDiscount = $curveResults.Range("H6").Value2
        $curvePointForward = $curveResults.Range("H9").Value2
        if ($price -isnot [double]) {
            throw "The example workbook did not calculate a numeric swap price. Result: $price"
        }
        if ($fairRate -isnot [double]) {
            throw "The example workbook did not calculate a numeric fair rate. Result: $fairRate"
        }
        if ($discountFactor -isnot [double]) {
            throw "The example workbook did not calculate a numeric maturity discount factor. Result: $discountFactor"
        }
        if ($curveTableReferenceDf -isnot [double] -or [Math]::Abs($curveTableReferenceDf - 1.0) -gt 0.00000001) {
            throw "The curve table did not spill a unit reference-date discount factor. Result: $curveTableReferenceDf"
        }
        if ($curveTableFirstForward -isnot [double]) {
            throw "The curve table did not spill a numeric forward rate. Result: $curveTableFirstForward"
        }
        if ($curvePointDiscount -isnot [double] -or [Math]::Abs($curvePointDiscount - $discountFactor) -gt 0.00000001) {
            throw "The curve point discount does not match the pricing-sheet result. Curve: $curvePointDiscount; Pricing: $discountFactor"
        }
        if ($curvePointForward -isnot [double] -or [Math]::Abs($curvePointForward) -gt 0.5) {
            throw "The curve point forward is missing or implausible. Result: $curvePointForward"
        }
        $notional = $inputs.Range("B4").Value2
        if ([Math]::Abs($price) -gt $notional) {
            throw "The calculated swap price is implausible for the configured notional. Result: $price"
        }
        if ([Math]::Abs($fairRate) -gt 0.5) {
            throw "The calculated fair rate is implausible. Result: $fairRate"
        }
        if ($discountFactor -le 0.0 -or $discountFactor -gt 1.5) {
            throw "The calculated maturity discount factor is implausible. Result: $discountFactor"
        }
    }

    $workbook.SaveAs($savePath, 51) # xlOpenXMLWorkbook
    $saveSucceeded = $true
}
catch {
    Write-Error "$($_.Exception.Message)`n$($_.ScriptStackTrace)"
    throw
}
finally {
    if ($null -ne $workbook) {
        $workbook.Close($false)
    }
    $entry = $null
    $sheet = $null
    if ($null -ne $handleCells) {
        foreach ($handleEntry in $handleCells) {
            if ($null -ne $handleEntry.Cell -and [Runtime.InteropServices.Marshal]::IsComObject($handleEntry.Cell)) {
                [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($handleEntry.Cell)
            }
        }
    }
    $handleCells = $null
    foreach ($worksheet in @($help, $outputs, $curveResults, $pricing, $inputs, $curve, $readme)) {
        if ($null -ne $worksheet -and [Runtime.InteropServices.Marshal]::IsComObject($worksheet)) {
            [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($worksheet)
        }
    }
    $worksheet = $null
    $help = $null
    $outputs = $null
    $curveResults = $null
    $pricing = $null
    $inputs = $null
    $curve = $null
    $readme = $null
    if ($null -ne $workbook -and [Runtime.InteropServices.Marshal]::IsComObject($workbook)) {
        [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($workbook)
    }
    $workbook = $null
    if ($null -ne $excel) {
        $excel.Quit()
        if ([Runtime.InteropServices.Marshal]::IsComObject($excel)) {
            [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($excel)
        }
    }
    $excel = $null
    [GC]::Collect()
    [GC]::WaitForPendingFinalizers()
    [GC]::Collect()
    [GC]::WaitForPendingFinalizers()

    if ($savePath -ne $OutputPath) {
        if ($saveSucceeded) {
            [IO.File]::Copy($savePath, $OutputPath, $true)
        }
        for ($cleanupAttempt = 0; $cleanupAttempt -lt 20; $cleanupAttempt++) {
            if (Test-Path -LiteralPath $savePath) {
                Remove-Item -LiteralPath $savePath -Force
                break
            }
            Start-Sleep -Milliseconds 250
        }
        if (Test-Path -LiteralPath $savePath) {
            throw "Could not remove the temporary workbook: $savePath"
        }
    }
}

Write-Host "Created swap-pricing workbook: $OutputPath"
if (Test-Path -LiteralPath $XllPath) {
    Write-Host ("Validated swap NPV: {0:N2}; fair rate: {1:P6}; maturity DF: {2:N8}; forward: {3:P6}" -f $price, $fairRate, $discountFactor, $curvePointForward)
}

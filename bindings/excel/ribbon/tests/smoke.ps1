param([string]$PreviewPath)

# Windows/.NET Framework smoke test; no Excel instance or native XLL is opened.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms, System.Drawing, System.Runtime.Serialization
$ribbonRoot = Split-Path -Parent $PSScriptRoot
$assets = Get-Content -LiteralPath (Join-Path $ribbonRoot 'obj/project.assets.json') -Raw | ConvertFrom-Json
$integrationPackage = ($assets.libraries.PSObject.Properties | Where-Object Name -Like 'ExcelDna.Integration/*').Value.path
$integrationPath = $assets.packageFolders.PSObject.Properties.Name | ForEach-Object {
    Join-Path $_ "$integrationPackage/lib/net462/ExcelDna.Integration.dll"
} | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
if (-not $integrationPath) { throw 'Build the ribbon in Release before running this test.' }
[void][Reflection.Assembly]::LoadFrom($integrationPath)
$assembly = [Reflection.Assembly]::LoadFrom((Join-Path $ribbonRoot 'bin/Release/net462/QuantSupport.Ribbon.dll'))
$browserType = $assembly.GetType('QuantSupport.Ribbon.BrowserControl')
$snapshotType = $assembly.GetType('QuantSupport.Ribbon.Snapshot')
$flags = [Reflection.BindingFlags]'Instance, NonPublic'
$json = @'
{"workbook":"Preview.xlsx","error":"","objects":[{"name":"USD swap","handle":"QSObject:SwapTrade:w1~swap:1","kind":"SwapTrade","family":"Trades","revision":1,"current_inputs":true,"details":[{"label":"Notional","value":"1000000"}],"locations":[],"dependencies":[],"curves":[]},{"name":"USD market","handle":"QSObject:PricingContext:w1~market:2","kind":"PricingContext","family":"Pricing contexts","revision":2,"current_inputs":true,"details":[],"locations":[],"dependencies":[],"curves":[]},{"name":"Swap pricer","handle":"QSObject:Pricer:w1~pricer:3","kind":"Pricer","family":"Pricing setup","revision":3,"current_inputs":true,"details":[],"locations":[],"dependencies":[],"curves":[]}]}
'@
$serializer = New-Object System.Runtime.Serialization.Json.DataContractJsonSerializer($snapshotType)
$inputStream = New-Object IO.MemoryStream(,[Text.Encoding]::UTF8.GetBytes($json))
try { $snapshot = $serializer.ReadObject($inputStream) } finally { $inputStream.Dispose() }
$control = [Activator]::CreateInstance($browserType)
$form = New-Object Windows.Forms.Form
try {
    $form.ClientSize = New-Object Drawing.Size(520,760)
    $form.StartPosition = 'Manual'
    $form.Location = New-Object Drawing.Point(-32000,-32000)
    $form.ShowInTaskbar = $false
    $form.Opacity = 0
    $form.Controls.Add($control)
    $form.Show()
    $browserType.GetField('snapshot',$flags).SetValue($control,$snapshot)
    $populate = $browserType.GetMethod('Populate',$flags)
    [void]$populate.Invoke($control,@())
    $grid = $browserType.GetField('grid',$flags).GetValue($control)
    $grid.SelectAll()
    if ($grid.SelectedRows.Count -ne 3) { throw 'All object rows must be selectable.' }
    $objectCopy = [string]$grid.GetClipboardContent().GetData([Windows.Forms.DataFormats]::UnicodeText)
    if (-not $objectCopy.Contains('USD swap') -or -not $objectCopy.Contains('Swap pricer') -or -not $objectCopy.Contains('Name')) {
        throw 'Object selection must copy all values and column headings.'
    }
    $grid.CurrentCell = $grid.Rows[0].Cells[0]
    $explorer = $browserType.GetField('tradeExplorer',$flags).GetValue($control)
    $explorerType = $explorer.GetType()
    $runButton = $explorerType.GetField('run',$flags).GetValue($explorer)
    if (-not $runButton.Enabled) { throw 'A selected trade with a context and pricer must enable requests.' }
    $form.PerformLayout()
    $runPosition = $explorer.PointToClient($runButton.PointToScreen([Drawing.Point]::Empty))
    if ($runPosition.X -lt 0 -or $runPosition.X + $runButton.Width -gt $explorer.ClientSize.Width) {
        throw 'Run request must remain visible within the pane width.'
    }
    $view = $explorerType.GetField('view',$flags).GetValue($explorer)
    $target = $explorerType.GetField('target',$flags).GetValue($explorer)
    $view.SelectedIndex = 2
    if (-not $target.Enabled) { throw 'Fair rate must enable the leg/segment input.' }
    $view.SelectedIndex = 0
    if ($target.Enabled) { throw 'PV must disable the leg/segment input.' }
    $table = $explorerType.GetField('table',$flags).GetValue($explorer)
    [void]$table.Columns.Add('amount','Amount')
    [void]$table.Columns.Add('currency','Currency')
    [void]$table.Rows.Add([object[]]@('100.5','USD'))
    [void]$table.Rows.Add([object[]]@('-20','EUR'))
    $table.SelectAll()
    $tableCopy = [string]$table.GetClipboardContent().GetData([Windows.Forms.DataFormats]::UnicodeText)
    if ($table.SelectedRows.Count -ne 2 -or -not $tableCopy.Contains('100.5') -or -not $tableCopy.Contains('-20') -or -not $tableCopy.Contains('Amount')) {
        throw 'Result tables must support selecting and copying every value and heading.'
    }
    $context = $explorerType.GetField('context',$flags).GetValue($explorer)
    $context.SelectedIndex = -1
    if ($runButton.Enabled) { throw 'A trade request requires a context.' }
    $context.SelectedIndex = 0
    if (-not $runButton.Enabled) { throw 'Selecting a context must enable requests again.' }
    if ($PreviewPath) {
        $image = New-Object Drawing.Bitmap($form.Width,$form.Height)
        try {
            $form.DrawToBitmap($image,(New-Object Drawing.Rectangle(0,0,$form.Width,$form.Height)))
            $image.Save($PreviewPath,[Drawing.Imaging.ImageFormat]::Png)
        } finally { $image.Dispose() }
    }
    $snapshot.Objects[0].CurrentInputs = $false
    [void]$populate.Invoke($control,@())
    if ($runButton.Enabled) { throw 'Changed trade inputs must disable requests.' }
    $snapshot.Objects[0].CurrentInputs = $true
    $snapshot.Objects[0].Kind = 'FixedRateBondTrade'
    [void]$populate.Invoke($control,@())
    $view.SelectedIndex = 2
    if ($runButton.Enabled) { throw 'Bond fair rate must explain its absent measure in the ribbon.' }
    $snapshot.Objects.Clear()
    [void]$populate.Invoke($control,@())
    if ($runButton.Enabled) { throw 'An empty browser must disable requests.' }
    Write-Output 'Ribbon construction, multi-selection, copied values, request controls and stale-input checks passed.'
} finally { $form.Dispose() }

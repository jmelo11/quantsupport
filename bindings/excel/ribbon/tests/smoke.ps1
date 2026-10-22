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
$snapshot.Objects[1].EvaluationDate = '2026-01-15'
$instrumentJson = '{"name":"USD swap contract","handle":"QSObject:Swap:w1~swap.contract:4","kind":"Swap","family":"Instruments","revision":4,"current_inputs":true,"details":[],"locations":[],"dependencies":[],"curves":[]}'
$instrumentSerializer = New-Object System.Runtime.Serialization.Json.DataContractJsonSerializer($assembly.GetType('QuantSupport.Ribbon.BrowserObject'))
$inputStream = New-Object IO.MemoryStream(,[Text.Encoding]::UTF8.GetBytes($instrumentJson))
try { $snapshot.Objects.Add($instrumentSerializer.ReadObject($inputStream)) } finally { $inputStream.Dispose() }
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
    if (-not $grid.VirtualMode) { throw 'Large object lists must use virtual rows.' }
    if ($grid.ColumnHeadersHeightSizeMode -ne [Windows.Forms.DataGridViewColumnHeadersHeightSizeMode]::AutoSize) {
        throw 'Headers must expand to fit wrapped text at the current display DPI.'
    }
    $grid.SelectAll()
    if ($grid.SelectedRows.Count -ne 4) { throw 'All object rows must be selectable.' }
    $objectCopy = [string]$grid.GetClipboardContent().GetData([Windows.Forms.DataFormats]::UnicodeText)
    if (-not $objectCopy.Contains('USD swap') -or -not $objectCopy.Contains('Swap pricer') -or -not $objectCopy.Contains('Name')) {
        throw 'Object selection must copy all values and column headings.'
    }
    $grid.CurrentCell = $grid.Rows[0].Cells[0]
    $explorer = $browserType.GetField('tradeExplorer',$flags).GetValue($control)
    $tabs = $browserType.GetField('tabs',$flags).GetValue($control)
    if ((($tabs.TabPages | ForEach-Object Text) -join ',') -ne 'Details,Pricing') {
        throw 'A trade must show only its relevant tabs.'
    }
    $explorerType = $explorer.GetType()
    $runButton = $explorerType.GetField('run',$flags).GetValue($explorer)
    if (-not $runButton.Enabled) { throw 'A selected trade with a context and pricer must enable requests.' }
    $form.PerformLayout()
    $runPosition = $explorer.PointToClient($runButton.PointToScreen([Drawing.Point]::Empty))
    if ($runPosition.X -lt 0 -or $runPosition.X + $runButton.Width -gt $explorer.ClientSize.Width) {
        throw 'Run request must remain visible within the pane width.'
    }
    if ($runPosition.Y -lt 0 -or $runPosition.Y + $runButton.Height -gt $explorer.ClientSize.Height) {
        throw 'Run request must remain visible within the pane height.'
    }
    $view = $explorerType.GetField('view',$flags).GetValue($explorer)
    $target = $explorerType.GetField('target',$flags).GetValue($explorer)
    if (-not $view.Items.Contains('Sensitivities')) { throw 'Pricing must offer Sensitivities.' }
    $view.SelectedItem = 'Sensitivities'
    $riskRequest = $explorerType.GetMethod('BuildQuery',$flags).Invoke($explorer,@())
    if ($target.Enabled -or -not $runButton.Enabled -or $riskRequest.Request -ne 'Sensitivities') {
        throw 'Sensitivities must run without a fair-rate leg or segment target.'
    }
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
    $response = [Activator]::CreateInstance($assembly.GetType('QuantSupport.Ribbon.BrowserResponse'))
    $response.NumericColumns = New-Object 'System.Collections.Generic.List[int]'
    $response.NumericColumns.Add(0)
    $explorerType.GetField('response',$flags).SetValue($explorer,$response)
    $separator = $assembly.GetType('QuantSupport.Ribbon.BrowserDisplay').GetProperty('DecimalSeparator',[Reflection.BindingFlags]'Static, NonPublic')
    $separator.SetValue($null,',')
    $table.Rows[0].Cells[1].Value = 'USD.v1'
    $localizedCopy = [string]$table.GetClipboardContent().GetData([Windows.Forms.DataFormats]::UnicodeText)
    if (-not $localizedCopy.Contains('100,5') -or -not $localizedCopy.Contains('USD.v1')) {
        throw 'Copy must use Excel decimal conventions without changing identifiers.'
    }
    $separator.SetValue($null,'.')
    $context = $explorerType.GetField('context',$flags).GetValue($explorer)
    $context.SelectedIndex = -1
    if ($runButton.Enabled) { throw 'A trade request requires a context.' }
    $context.SelectedIndex = 0
    if (-not $runButton.Enabled) { throw 'Selecting a context must enable requests again.' }
    $snapshot.Objects[1].Curves.Add('SOFR')
    [void]$populate.Invoke($control,@())
    $grid.CurrentCell = $grid.Rows[1].Cells[0]
    if ((($tabs.TabPages | ForEach-Object Text) -join ',') -ne 'Details,Curves') {
        throw 'A context with curves must show only Details and Curves.'
    }
    $curveExplorer = $browserType.GetField('curveExplorer',$flags).GetValue($control)
    $curveMessage = $explorerType.GetField('message',$flags).GetValue($curveExplorer)
    $curveTable = $explorerType.GetField('table',$flags).GetValue($curveExplorer)
    if (-not $curveMessage.Text.Contains('click Refresh table') -or $curveTable.RowCount -ne 0) {
        throw 'Selecting a curve must wait for an explicit query, including hidden tabs.'
    }
    $snapshot.Objects[1].Handle = 'QSObject:PricingContext:w1~market:99'
    [void]$populate.Invoke($control,@())
    $selected = $browserType.GetProperty('Selected',$flags).GetValue($control)
    if ($selected.Handle -ne 'QSObject:PricingContext:w1~market:99' -or $tabs.SelectedTab.Text -ne 'Curves') {
        throw 'Refreshing after a quote correction must retain the curve object with its new handle.'
    }
    $grid.CurrentCell = $grid.Rows[0].Cells[0]
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
    $view.SelectedIndex = 0
    $grid.CurrentCell = $grid.Rows[3].Cells[0]
    if ((($tabs.TabPages | ForEach-Object Text) -join ',') -ne 'Details,Pricing' -or -not $runButton.Enabled) {
        throw 'Selecting an instrument must expose working pricing controls.'
    }
    $priceButton = $browserType.GetField('price',$flags).GetValue($control)
    if (-not $priceButton.Enabled) { throw 'Instrument selection must enable Price selected.' }
    $tabs.SelectedIndex = 0
    [void]$browserType.GetMethod('ShowPricing',$flags).Invoke($control,@())
    if ($tabs.SelectedTab.Text -ne 'Pricing') { throw 'Price selected must open Pricing.' }
    $buildQuery = $explorerType.GetMethod('BuildQuery',$flags)
    $preview = $buildQuery.Invoke($explorer,@())
    if ($preview.Handle -ne $snapshot.Objects[3].Handle -or $preview.TradeDate -ne '2026-01-15' -or $preview.Side -ne 'Receive') {
        throw 'Instrument previews must send the selected contract and visible position inputs.'
    }
    $previewDate = $explorerType.GetField('tradeDate',$flags).GetValue($explorer)
    $previewDate.Value = [datetime]'2026-01-14'
    [void]$populate.Invoke($control,@())
    if ($buildQuery.Invoke($explorer,@()).TradeDate -ne '2026-01-14') {
        throw 'Refreshing the same instrument must retain edited preview terms.'
    }
    $snapshot.Objects[3].Kind = 'FixedRateBond'
    [void]$populate.Invoke($control,@())
    if ($runButton.Enabled) { throw 'A bond preview must require an explicit anchor.' }
    $anchor = $explorerType.GetField('anchor',$flags).GetValue($explorer)
    $anchorValue = $explorerType.GetField('anchorValue',$flags).GetValue($explorer)
    $anchor.SelectedItem = 'CleanPrice'
    $anchorValue.Text = '99.25'
    if (-not $runButton.Enabled -or $buildQuery.Invoke($explorer,@()).AnchorValue -ne 99.25) {
        throw 'A valid bond anchor must enable instrument pricing.'
    }
    $grid.CurrentCell = $grid.Rows[0].Cells[0]
    $stored = $buildQuery.Invoke($explorer,@())
    if ($null -ne $stored.TradeDate -or $null -ne $stored.Side -or $null -ne $stored.AnchorValue) {
        throw 'Stored trade pricing must retain the original trade terms.'
    }
    $snapshot.Objects.Clear()
    [void]$populate.Invoke($control,@())
    if ($runButton.Enabled) { throw 'An empty browser must disable requests.' }
    Write-Output 'Ribbon construction, multi-selection, copied values, request controls and stale-input checks passed.'
} finally { $form.Dispose() }

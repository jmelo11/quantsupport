using System;
using System.Collections;
using System.Drawing;
using System.IO;
using System.Reflection;
using System.Linq;
using System.Diagnostics;
using System.Globalization;
using System.Threading.Tasks;
using System.Windows.Forms;
using ExcelDna.Integration;

public static class PanelProbe
{
    [ExcelCommand(Name = "QS.PanelProbe.RefreshLargeBrowser")]
    public static async void RefreshLargeBrowser()
    {
        var output = Path.Combine(Path.GetDirectoryName(ExcelDnaUtil.XllPath), "browser-refresh.txt");
        File.WriteAllText(output, "started\n");
        Form window = null;
        QuantSupport.Ribbon.BrowserControl browser = null;
        var closedPopups = new System.Collections.Generic.List<ToolStripDropDown>();
        try {
            dynamic app = ExcelDnaUtil.Application;
            var workbook = (string)app.ActiveWorkbook.Name;
            var handle = (string)app.ActiveWorkbook.Worksheets["Origination"].Range["F10001"].Value2;
            var watch = Stopwatch.StartNew();
            Action<string, double> record = (key, value) => File.AppendAllText(output, key + "=" + value.ToString("R", CultureInfo.InvariantCulture) + "\n");
            QuantSupport.Ribbon.NativeBrowser.ClearSnapshot();
            browser = new QuantSupport.Ribbon.BrowserControl { WindowId = Convert.ToInt64(app.ActiveWindow.Hwnd) };
            window = new Form { ClientSize = new Size(560, 760), Location = new Point(-32000, -32000), StartPosition = FormStartPosition.Manual, ShowInTaskbar = false, Opacity = 0 };
            window.Controls.Add(browser); window.Show();
            browser.SetFilter("Instruments & Trades");
            var loading = browser.RefreshObjects(true);
            Require(ReferenceEquals(loading, browser.RefreshObjects(true)), "Repeated refresh did not coalesce.");
            browser.Inspect(handle); browser.ShowPricing();
            record("open_return_ms", watch.Elapsed.TotalMilliseconds);
            await loading.ConfigureAwait(false);
            record("open_complete_ms", watch.Elapsed.TotalMilliseconds);
            Task refresh = null;
            await QuantSupport.Ribbon.NativeBrowser.OnExcel(() => {
                var snapshot = (QuantSupport.Ribbon.Snapshot)Field(browser, "snapshot");
                Require(snapshot.Workbook == workbook, "Wrong workbook applied.");
                record("objects", snapshot.Objects.Count);
                var pricing = (QuantSupport.Ribbon.ObjectPicker)Field(browser, "pricingObjects");
                Require(pricing.SelectedItem.Handle == handle, "Deferred selection was lost.");
                record("pricing_objects", pricing.Items.Count);
                var explorer = Field(browser, "tradeExplorer");
                var contexts = (QuantSupport.Ribbon.ObjectPicker)Field(explorer, "context");
                record("contexts", contexts.Items.Count);
                Require(contexts.Items.Count == 10000, "Context list was truncated.");
                foreach (var picker in new[] { pricing, contexts }) {
                    watch.Restart();
                    Call(picker, "ShowChoices");
                    record(picker == pricing ? "pricing_popup_ms" : "context_popup_ms", watch.Elapsed.TotalMilliseconds);
                    var popup = (ToolStripDropDown)Field(picker, "popup");
                    var panel = ((ToolStripControlHost)popup.Items[0]).Control;
                    var search = panel.Controls.OfType<TextBox>().Single();
                    var grid = panel.Controls.OfType<DataGridView>().Single();
                    Require(grid.VirtualMode && grid.RowCount == picker.Items.Count, "Popup omitted objects.");
                    var last = picker.Items.Single(value => value.Name == (picker == pricing ? "trade.9999" : "market.9999"));
                    search.Text = "no matching object";
                    Require(grid.RowCount == 0, "Empty search retained rows.");
                    search.Text = "";
                    Require(grid.RowCount == picker.Items.Count, "Clearing search lost objects.");
                    watch.Restart(); search.Text = last.Name;
                    record(picker == pricing ? "pricing_search_ms" : "context_search_ms", watch.Elapsed.TotalMilliseconds);
                    Require(grid.RowCount == 1, "Search did not find the last object.");
                    Call(search, "OnKeyDown", new KeyEventArgs(Keys.Enter));
                    Require(picker.SelectedItem == last, "Search selection failed.");
                    Call(picker, "ShowChoices");
                    popup = (ToolStripDropDown)Field(picker, "popup");
                    search = ((ToolStripControlHost)popup.Items[0]).Control.Controls.OfType<TextBox>().Single();
                    Call(search, "OnKeyDown", new KeyEventArgs(Keys.Escape));
                    Require(picker.SelectedItem == last && Field(picker, "popup") == null, "Escape changed selection or left the popup open.");
                    Call(picker, "ShowChoices");
                    popup = (ToolStripDropDown)Field(picker, "popup");
                    popup.Close(ToolStripDropDownCloseReason.AppClicked);
                    Require(Field(picker, "popup") == null, "Outside click left a popup open.");
                    closedPopups.Add(popup);
                }
                watch.Restart();
                refresh = browser.RefreshObjects(true);
            }).ConfigureAwait(false);
            await refresh.ConfigureAwait(false);
            record("cached_refresh_ms", watch.Elapsed.TotalMilliseconds);
            await Task.Delay(50).ConfigureAwait(false);
            await QuantSupport.Ribbon.NativeBrowser.OnExcel(() => {
                Require(closedPopups.All(value => value.IsDisposed), "Closed popup controls were retained.");
                var pricing = (QuantSupport.Ribbon.ObjectPicker)Field(browser, "pricingObjects");
                Require(pricing.SelectedItem.Name == "trade.9999", "Refresh lost object selection.");
                var explorer = Field(browser, "tradeExplorer");
                Require(((QuantSupport.Ribbon.ObjectPicker)Field(explorer, "context")).SelectedItem.Name == "market.9999", "Refresh lost context selection.");
                QuantSupport.Ribbon.NativeBrowser.ClearSnapshot();
                watch.Restart(); refresh = browser.RefreshObjects(true);
                record("changed_refresh_return_ms", watch.Elapsed.TotalMilliseconds);
                browser.InvalidateObjects();
            }).ConfigureAwait(false);
            await refresh.ConfigureAwait(false);
            await QuantSupport.Ribbon.NativeBrowser.OnExcel(() => {
                Require(Field(browser, "snapshot") == null, "An invalidated snapshot was applied late.");
                refresh = browser.RefreshObjects(true);
            }).ConfigureAwait(false);
            await refresh.ConfigureAwait(false);
            await QuantSupport.Ribbon.NativeBrowser.OnExcel(() => {
                QuantSupport.Ribbon.NativeBrowser.ClearSnapshot();
                refresh = browser.RefreshObjects(true);
                var picker = (QuantSupport.Ribbon.ObjectPicker)Field(browser, "pricingObjects");
                Call(picker, "ShowChoices");
                Require(Field(picker, "popup") != null, "Dispose did not exercise an open popup.");
                window.Dispose();
            }).ConfigureAwait(false);
            await refresh.ConfigureAwait(false);
            File.AppendAllText(output, "PASS\n");
        } catch (Exception error) { File.AppendAllText(output, "FAIL\n" + error); }
        finally { if (window != null) await QuantSupport.Ribbon.NativeBrowser.OnExcel(() => window.Dispose()); }
    }

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }

    [ExcelCommand(Name = "QS.PanelProbe.ProfileBrowser")]
    public static async void ProfileBrowser()
    {
        var output = Path.Combine(Path.GetDirectoryName(ExcelDnaUtil.XllPath), "browser-profile.txt");
        File.WriteAllText(output, "started\n");
        Form window = null;
        try {
            dynamic app = ExcelDnaUtil.Application;
            var watch = Stopwatch.StartNew();
            Action<string> elapsed = label => { File.AppendAllText(output, label + "=" + watch.Elapsed.TotalMilliseconds.ToString("R", CultureInfo.InvariantCulture) + "\n"); watch.Restart(); };
            var read = QuantSupport.Ribbon.NativeBrowser.ReadAsync((string)app.ActiveWorkbook.Name);
            elapsed("capture_ms");
            var snapshot = await read.ConfigureAwait(false);
            elapsed("decode_ms");
            File.AppendAllText(output, "objects=" + snapshot.Objects.Count + "\n");
            await QuantSupport.Ribbon.NativeBrowser.OnExcel(() => {
            var browser = new QuantSupport.Ribbon.BrowserControl();
            browser.WindowId = Convert.ToInt64(app.ActiveWindow.Hwnd);
            window = new Form { ClientSize = new Size(560, 760), Location = new Point(-32000, -32000), StartPosition = FormStartPosition.Manual, ShowInTaskbar = false, Opacity = 0 };
            window.Controls.Add(browser); window.Show();
            elapsed("create_ms");
            typeof(QuantSupport.Ribbon.BrowserControl).GetField("snapshot", Flags).SetValue(browser, snapshot);
            Call(browser, "Populate");
            elapsed("populate_ms");
            browser.SetFilter("Instruments & Trades");
            elapsed("pricing_ms");
            browser.SetFilter("Curves");
            elapsed("curves_ms");
            browser.SetFilter("All objects");
            elapsed("objects_ms");
            });
            File.AppendAllText(output, "PASS\n");
        } catch (Exception error) { File.AppendAllText(output, "FAIL\n" + error); }
        finally { if (window != null) await QuantSupport.Ribbon.NativeBrowser.OnExcel(() => window.Dispose()); }
    }
    [ExcelCommand(Name = "QS.PanelProbe.Snapshot")]
    public static void Snapshot()
    {
        dynamic app = ExcelDnaUtil.Application;
        var snapshot = QuantSupport.Ribbon.NativeBrowser.ReadAsync((string)app.ActiveWorkbook.Name).GetAwaiter().GetResult();
        using (var stream = File.Create(Path.Combine(Path.GetDirectoryName(ExcelDnaUtil.XllPath), "snapshot-live.json")))
            new System.Runtime.Serialization.Json.DataContractJsonSerializer(typeof(QuantSupport.Ribbon.Snapshot)).WriteObject(stream, snapshot);
    }
    private static readonly BindingFlags Flags = BindingFlags.Instance | BindingFlags.NonPublic;
    private static Form form;
    private static object control;
    private static object explorer;
    private static string directory;
    private static Timer timer;
    private static DateTime deadline;
    private static object Field(object value, string name) { return value.GetType().GetField(name, Flags).GetValue(value); }
    private static object Property(object value, string name) { return value.GetType().GetProperty(name).GetValue(value, null); }
    private static object Call(object value, string name, params object[] args) { return value.GetType().GetMethod(name, Flags).Invoke(value, args); }

    [ExcelCommand(Name = "QS.PanelProbe.Run")]
    public static async void Run()
    {
        directory = Path.GetDirectoryName(ExcelDnaUtil.XllPath);
        try
        {
            var assembly = AppDomain.CurrentDomain.GetAssemblies().Single(a => a.GetName().Name == "QuantSupport.Ribbon");
            dynamic app = ExcelDnaUtil.Application;
            var selections = File.ReadAllLines(Path.Combine(directory, "selection.txt"));
            var name = selections[0].Trim();
            string handle = app.ActiveWorkbook.Names.Item(name).RefersToRange.Value2;
            control = Activator.CreateInstance(assembly.GetType("QuantSupport.Ribbon.BrowserControl"));
            control.GetType().GetField("WindowId", Flags).SetValue(control, Convert.ToInt64(app.ActiveWindow.Hwnd));
            form = new Form { ClientSize = new Size(560, 760), Location = new Point(-32000, -32000), StartPosition = FormStartPosition.Manual, ShowInTaskbar = false, Opacity = 0 };
            form.Controls.Add((Control)control);
            form.Show();
            Call(control, "SetFilter", "Instruments & Trades");
            var refreshed = (Task)Call(control, "RefreshObjects", true);
            Call(control, "Inspect", handle);
            Call(control, "ShowPricing");
            await refreshed.ConfigureAwait(false);
            await QuantSupport.Ribbon.NativeBrowser.OnExcel(() => {
            explorer = Field(control, "tradeExplorer");
            var run = (Button)Field(explorer, "run");
            if (selections.Length > 1) ((ComboBox)Field(explorer, "view")).SelectedItem = selections[1].Trim();
            if (selections.Length > 3) {
                var markets = (QuantSupport.Ribbon.ObjectPicker)Field(explorer, "context");
                var pricers = (QuantSupport.Ribbon.ObjectPicker)Field(explorer, "pricer");
                markets.SelectedItem = markets.Items.OfType<QuantSupport.Ribbon.BrowserObject>().Single(item => item.Name == selections[2].Trim());
                pricers.SelectedItem = pricers.Items.OfType<QuantSupport.Ribbon.BrowserObject>().Single(item => item.Name == selections[3].Trim());
            }
            if (!run.Enabled) throw new InvalidOperationException("Pricing is disabled: " + ((Label)Field(explorer, "message")).Text);
            Call(explorer, "RunQuery");
            deadline = DateTime.UtcNow.AddSeconds(30);
            timer = new Timer { Interval = 25 };
            timer.Tick += (sender, args) => {
                if (Field(explorer, "response") == null && DateTime.UtcNow < deadline && ((Label)Field(explorer, "message")).Text.StartsWith("Running")) return;
                timer.Stop(); timer.Dispose(); timer = null;
                Finish();
            };
            timer.Start();
            });
        }
        catch (Exception error) { await QuantSupport.Ribbon.NativeBrowser.OnExcel(() => SaveError(error)); }
    }

    private static void Finish()
    {
        try
        {
            var response = Field(explorer, "response");
            if (response == null) throw new InvalidOperationException(((Label)Field(explorer, "message")).Text);
            var rows = (IList)Property(response, "Rows");
            if (rows.Count == 0) throw new InvalidOperationException("No pricing result returned.");
            var context = ((QuantSupport.Ribbon.ObjectPicker)Field(explorer, "context")).SelectedItem;
            var pricer = ((QuantSupport.Ribbon.ObjectPicker)Field(explorer, "pricer")).SelectedItem;
            var title = Property(response, "Title");
            var first = (IEnumerable)rows[0];
            var values = "";
            foreach (var value in first) values += value + " | ";
            form.PerformLayout();
            using (var bitmap = new Bitmap(form.Width, form.Height))
            {
                form.DrawToBitmap(bitmap, new Rectangle(0, 0, form.Width, form.Height));
                bitmap.Save(Path.Combine(directory, "panel-live.png"));
            }
            File.WriteAllText(Path.Combine(directory, "probe-result.txt"), "PASS\nContext: " + Property(context, "Name") + "\nPricer: " + Property(pricer, "Name") + "\nRows: " + rows.Count + "\n" + title + ": " + values);
            form.Dispose();
        }
        catch (Exception error) { SaveError(error); }
    }

    private static void SaveError(Exception error)
    {
        File.WriteAllText(Path.Combine(directory, "probe-result.txt"), "FAIL\n" + error);
        if (form != null) form.Dispose();
    }
}

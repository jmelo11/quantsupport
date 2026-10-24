using System;
using System.Collections;
using System.Drawing;
using System.IO;
using System.Reflection;
using System.Linq;
using System.Windows.Forms;
using ExcelDna.Integration;

public static class PanelProbe
{
    [ExcelCommand(Name = "QS.PanelProbe.Snapshot")]
    public static void Snapshot()
    {
        dynamic app = ExcelDnaUtil.Application;
        var snapshot = QuantSupport.Ribbon.NativeBrowser.Read((string)app.ActiveWorkbook.Name);
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
    public static void Run()
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
            Call(control, "RefreshObjects", true);
            Call(control, "Inspect", handle);
            Call(control, "ShowPricing");
            explorer = Field(control, "tradeExplorer");
            var run = (Button)Field(explorer, "run");
            if (selections.Length > 1) ((ComboBox)Field(explorer, "view")).SelectedItem = selections[1].Trim();
            if (selections.Length > 3) {
                var markets = (ComboBox)Field(explorer, "context");
                var pricers = (ComboBox)Field(explorer, "pricer");
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
        }
        catch (Exception error) { SaveError(error); }
    }

    private static void Finish()
    {
        try
        {
            var response = Field(explorer, "response");
            if (response == null) throw new InvalidOperationException(((Label)Field(explorer, "message")).Text);
            var rows = (IList)Property(response, "Rows");
            if (rows.Count == 0) throw new InvalidOperationException("No pricing result returned.");
            var context = ((ComboBox)Field(explorer, "context")).SelectedItem;
            var pricer = ((ComboBox)Field(explorer, "pricer")).SelectedItem;
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

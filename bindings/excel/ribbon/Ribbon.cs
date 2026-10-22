using System;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Windows.Forms;
using ExcelDna.Integration;
using ExcelDna.Integration.CustomUI;

namespace QuantSupport.Ribbon
{
    public sealed class AddIn : IExcelAddIn
    {
        public void AutoOpen() { }
        public void AutoClose() { BrowserPanes.CloseAll(); }
    }

    [ComVisible(true)]
    public sealed class QuantSupportRibbon : ExcelRibbon
    {
        public override string GetCustomUI(string ribbonId)
        {
            using (var stream = typeof(QuantSupportRibbon).Assembly.GetManifestResourceStream("QuantSupport.Ribbon.Ribbon.xml"))
            using (var reader = new StreamReader(stream)) return reader.ReadToEnd();
        }

        internal static void Run(Action action)
        {
            try { action(); }
            catch (Exception error) { MessageBox.Show(error.Message, "QuantSupport", MessageBoxButtons.OK, MessageBoxIcon.Information); }
        }

        public void ShowContexts(IRibbonControl control) { Run(() => BrowserPanes.Show("Pricing contexts")); }
        public void ShowInstruments(IRibbonControl control) { Run(() => BrowserPanes.Show("Instruments & Trades")); }
        public void ShowCurves(IRibbonControl control) { Run(() => BrowserPanes.Show("Curves")); }
        public void ShowAll(IRibbonControl control) { Run(() => BrowserPanes.Show("All objects")); }
        public void InspectSelection(IRibbonControl control) { Run(() => BrowserPanes.Inspect()); }
        public void RefreshBrowser(IRibbonControl control) { Run(() => BrowserPanes.Show(null)); }
        public void OpenSetup(IRibbonControl control) { Open("guide/workbook.html"); }
        public void OpenReference(IRibbonControl control) { Open("functions/index.html"); }

        private static void Open(string path)
        {
            Run(() => Process.Start(new ProcessStartInfo("https://jmelo11.github.io/quantsupport/api/excel/" + path) { UseShellExecute = true }));
        }
    }
}

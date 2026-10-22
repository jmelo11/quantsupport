using System;
using System.Collections.Generic;
using System.Linq;
using System.Windows.Forms;
using ExcelDna.Integration;
using ExcelDna.Integration.CustomUI;

namespace QuantSupport.Ribbon
{
    internal static class BrowserPanes
    {
        private sealed class Entry
        {
            internal CustomTaskPane Pane;
            internal BrowserControl Control;
        }

        // Numeric window IDs only. Holding Workbook/Window RCWs prevents clean closure.
        private static readonly Dictionary<long, Entry> Panes = new Dictionary<long, Entry>();
        private static Timer timer;

        internal static long ActiveWindowId()
        {
            dynamic app = ExcelDnaUtil.Application;
            if (app.ActiveWindow == null || app.ActiveWorkbook == null)
                throw new InvalidOperationException("Open a workbook to browse its QuantSupport objects.");
            return Convert.ToInt64(app.ActiveWindow.Hwnd);
        }

        internal static string DecimalSeparator(dynamic app)
        {
            return (bool)app.UseSystemSeparators ? Convert.ToString(app.International[3]) : (string)app.DecimalSeparator;
        }

        internal static BrowserControl Show(string filter)
        {
            var id = ActiveWindowId();
            if (!Panes.TryGetValue(id, out var entry))
            {
                dynamic app = ExcelDnaUtil.Application;
                var pane = CustomTaskPaneFactory.CreateCustomTaskPane(typeof(BrowserControl), "QuantSupport", app.ActiveWindow);
                var control = (BrowserControl)pane.ContentControl;
                control.WindowId = id;
                pane.DockPosition = MsoCTPDockPosition.msoCTPDockPositionRight;
                pane.Width = 520;
                entry = new Entry { Pane = pane, Control = control };
                Panes.Add(id, entry);
                if (timer == null)
                {
                    timer = new Timer { Interval = 2000 };
                    timer.Tick += Tick;
                    timer.Start();
                }
            }
            entry.Pane.Visible = true;
            entry.Control.SetFilter(filter);
            entry.Control.RefreshObjects(filter == null);
            return entry.Control;
        }

        internal static void Inspect()
        {
            var control = Show("All objects");
            dynamic app = ExcelDnaUtil.Application;
            var value = app.ActiveCell == null ? null : app.ActiveCell.Value2 as string;
            control.Inspect(value);
        }

        private static void Tick(object sender, EventArgs args)
        {
            try
            {
                dynamic app = ExcelDnaUtil.Application;
                if (!(bool)app.Ready || (int)app.CalculationState == 1) return;
                var open = new HashSet<long>();
                var count = (int)app.Windows.Count;
                for (var i = 1; i <= count; i++) open.Add(Convert.ToInt64(app.Windows[i].Hwnd));
                foreach (var closed in Panes.Keys.Where(id => !open.Contains(id)).ToArray()) Remove(closed);
                if (count > 0 && Panes.TryGetValue(ActiveWindowId(), out var entry) && entry.Pane.Visible)
                    entry.Control.RefreshObjects(false);
                if (Panes.Count == 0) StopTimer();
            }
            // Excel can reject automation calls while a dialog is open or it is shutting down.
            catch (System.Runtime.InteropServices.COMException) { }
            catch (InvalidOperationException) { }
        }

        private static void Remove(long id)
        {
            var entry = Panes[id];
            Panes.Remove(id);
            try { entry.Pane.Delete(); }
            catch (System.Runtime.InteropServices.COMException) { }
            finally { entry.Control.Dispose(); }
        }

        private static void StopTimer()
        {
            if (timer == null) return;
            timer.Stop();
            timer.Dispose();
            timer = null;
        }

        internal static void CloseAll()
        {
            StopTimer();
            foreach (var id in Panes.Keys.ToArray()) Remove(id);
        }

        internal static void InvalidateAll()
        {
            foreach (var entry in Panes.Values) entry.Control.InvalidateObjects();
        }
    }
}

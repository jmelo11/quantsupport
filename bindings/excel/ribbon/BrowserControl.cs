using System;
using System.Collections.Generic;
using System.Drawing;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using System.Windows.Forms;
using ExcelDna.Integration;

namespace QuantSupport.Ribbon
{
    [ComVisible(true)]
    [Guid("9713351A-27B1-4728-8D92-824E60322FD8")]
    [ClassInterface(ClassInterfaceType.AutoDispatch)]
    public sealed class BrowserControl : UserControl
    {
        internal long WindowId;
        private Snapshot snapshot;
        private string stamp;
        private readonly ComboBox filter = new ComboBox { DropDownStyle = ComboBoxStyle.DropDownList, Width = 170 };
        private readonly TextBox search = new TextBox { Width = 170 };
        private readonly Label status = new Label { Dock = DockStyle.Fill, AutoSize = true, Padding = new Padding(4) };
        private readonly DataGridView grid = new DataGridView
        {
            Dock = DockStyle.Fill, ReadOnly = true, MultiSelect = false,
            SelectionMode = DataGridViewSelectionMode.FullRowSelect, AllowUserToAddRows = false,
            AllowUserToDeleteRows = false, AllowUserToOrderColumns = true, RowHeadersVisible = false,
            AutoGenerateColumns = false, BackgroundColor = SystemColors.Window,
            AutoSizeColumnsMode = DataGridViewAutoSizeColumnsMode.DisplayedCells
        };
        private readonly TextBox details = new TextBox
        {
            Dock = DockStyle.Fill, ReadOnly = true, Multiline = true, WordWrap = false,
            ScrollBars = ScrollBars.Both, BackColor = SystemColors.Window
        };
        private readonly ComboBox sources = new ComboBox { DropDownStyle = ComboBoxStyle.DropDownList, Width = 190, DisplayMember = "Address" };
        private readonly Button go = new Button { Text = "Go to cell", AutoSize = true, Enabled = false };
        private readonly Button copy = new Button { Text = "Copy handle", AutoSize = true, Enabled = false };

        public BrowserControl()
        {
            Font = new Font("Segoe UI", 9);
            Dock = DockStyle.Fill;
            filter.Items.AddRange(new object[] { "All objects", "Pricing contexts", "Instruments & Trades", "Market & configuration", "Pricing setup", "Results" });
            filter.SelectedIndex = 0;
            var toolbar = new FlowLayoutPanel { Dock = DockStyle.Fill, AutoSize = true, Padding = new Padding(4) };
            var refresh = new Button { Text = "Refresh", AutoSize = true };
            refresh.Click += (s, e) => RefreshObjects(true);
            toolbar.Controls.Add(filter);
            toolbar.Controls.Add(new Label { Text = "Search", AutoSize = true, Margin = new Padding(4, 7, 2, 0) });
            toolbar.Controls.Add(search);
            toolbar.Controls.Add(refresh);
            foreach (var title in new[] { "Name", "Type", "Valuation date", "Snapshot (UTC)", "Quote side", "Inputs" })
                grid.Columns.Add(new DataGridViewTextBoxColumn { HeaderText = title, SortMode = DataGridViewColumnSortMode.NotSortable });
            var split = new SplitContainer { Dock = DockStyle.Fill, Orientation = Orientation.Horizontal, Size = new Size(500, 520), SplitterDistance = 285 };
            split.Panel1.Controls.Add(grid);
            split.Panel2.Controls.Add(details);
            var navigation = new FlowLayoutPanel { Dock = DockStyle.Fill, AutoSize = true, Padding = new Padding(4) };
            navigation.Controls.Add(sources);
            navigation.Controls.Add(go);
            navigation.Controls.Add(copy);
            var layout = new TableLayoutPanel { Dock = DockStyle.Fill, ColumnCount = 1, RowCount = 4 };
            layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            layout.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
            layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            layout.Controls.Add(toolbar, 0, 0);
            layout.Controls.Add(split, 0, 1);
            layout.Controls.Add(navigation, 0, 2);
            layout.Controls.Add(status, 0, 3);
            Controls.Add(layout);
            filter.SelectedIndexChanged += (s, e) => Populate();
            search.TextChanged += (s, e) => Populate();
            grid.SelectionChanged += (s, e) => ShowDetails();
            go.Click += (s, e) => QuantSupportRibbon.Run(GoToSource);
            copy.Click += (s, e) => QuantSupportRibbon.Run(() => { if (Selected != null) Clipboard.SetText(Selected.Handle); });
        }

        private BrowserObject Selected => grid.CurrentRow?.Tag as BrowserObject;

        internal void SetFilter(string value)
        {
            if (value != null) { search.Clear(); filter.SelectedItem = value; }
        }

        internal void RefreshObjects(bool force)
        {
            try
            {
                if (BrowserPanes.ActiveWindowId() != WindowId) return;
                dynamic app = ExcelDnaUtil.Application;
                if (!(bool)app.Ready) return;
                var next = NativeBrowser.Read((string)app.ActiveWorkbook.Name);
                var nextStamp = next.Workbook + "\n" + string.Join("\n", next.Objects.Select(o =>
                    o.Handle + "|" + o.CurrentInputs + "|" + string.Join("|", o.Locations.Select(l => l.SheetId + ":" + l.Row + ":" + l.Column + ":" + l.Formula))));
                snapshot = next;
                if (force || stamp != nextStamp) { stamp = nextStamp; Populate(); }
            }
            catch (Exception error)
            {
                snapshot = null;
                stamp = null;
                grid.Rows.Clear();
                details.Clear();
                sources.Items.Clear();
                go.Enabled = copy.Enabled = false;
                status.Text = error.Message;
            }
        }

        private void Populate()
        {
            var selected = Selected?.Handle;
            grid.Rows.Clear();
            if (snapshot == null) return;
            var category = (string)filter.SelectedItem;
            var query = search.Text.Trim();
            foreach (var item in snapshot.Objects)
            {
                var matchesCategory = category == "All objects" || item.Family == category ||
                    (category == "Instruments & Trades" && (item.Family == "Instruments" || item.Family == "Trades"));
                if (!matchesCategory || (query.Length > 0 &&
                    (item.Name + " " + item.Kind + " " + item.Handle).IndexOf(query, StringComparison.OrdinalIgnoreCase) < 0)) continue;
                var index = grid.Rows.Add(item.Name, item.Kind, item.EvaluationDate, item.SnapshotTimestamp, item.QuoteLevel,
                    item.CurrentInputs ? "Current" : "Recalculate");
                grid.Rows[index].Tag = item;
            }
            foreach (DataGridViewRow row in grid.Rows)
                if (((BrowserObject)row.Tag).Handle == selected) { grid.CurrentCell = row.Cells[0]; break; }
            ShowDetails();
            status.Text = snapshot.Workbook + " · " + grid.Rows.Count + " of " + snapshot.Objects.Count + " live objects" +
                (snapshot.Objects.Count == 0 ? ". Calculate the workbook to create its objects." : ". Refreshes while Excel is ready.");
        }

        private void ShowDetails()
        {
            sources.Items.Clear();
            var item = Selected;
            copy.Enabled = item != null;
            go.Enabled = item != null && item.Locations.Count > 0;
            if (item == null) { details.Clear(); return; }
            var text = new StringBuilder();
            text.AppendLine(item.Name + " (" + item.Kind + ")");
            text.AppendLine(item.Handle);
            text.AppendLine("Inputs: " + (item.CurrentInputs ? "Current" : "Changed; recalculate to rebuild this object"));
            if (!string.IsNullOrEmpty(item.EvaluationDate)) text.AppendLine("Valuation date: " + item.EvaluationDate);
            if (!string.IsNullOrEmpty(item.SnapshotTimestamp)) text.AppendLine("Snapshot (UTC): " + item.SnapshotTimestamp);
            if (!string.IsNullOrEmpty(item.QuoteLevel)) text.AppendLine("Quote side: " + item.QuoteLevel);
            foreach (var field in item.Details) text.AppendLine(field.Label + ": " + field.Value);
            text.AppendLine();
            text.AppendLine("Defining / updating cells:");
            foreach (var cell in item.Locations)
            {
                sources.Items.Add(cell);
                text.AppendLine(cell.Address);
                if (!string.IsNullOrEmpty(cell.Formula)) text.AppendLine(cell.Formula);
            }
            if (sources.Items.Count > 0) sources.SelectedIndex = 0;
            else text.AppendLine("No defining cell remains; another live formula retains this object.");
            if (item.Dependencies.Count > 0)
            {
                text.AppendLine();
                text.AppendLine("Inputs:");
                foreach (var dependency in item.Dependencies) text.AppendLine(dependency);
            }
            details.Text = text.ToString();
        }

        internal void Inspect(string handle)
        {
            search.Clear();
            if (!string.IsNullOrEmpty(handle))
                foreach (DataGridViewRow row in grid.Rows)
                    if (((BrowserObject)row.Tag).Handle == handle) { grid.CurrentCell = row.Cells[0]; return; }
            grid.ClearSelection();
            grid.CurrentCell = null;
            ShowDetails();
            status.Text = "Select a cell containing a live QuantSupport handle. If its handle is from a saved or older calculation, recalculate first.";
        }

        private void GoToSource()
        {
            var location = sources.SelectedItem as Location;
            var handle = Selected?.Handle;
            var workbook = snapshot?.Workbook;
            if (location == null || handle == null || workbook == null) return;
            ExcelAsyncUtil.QueueAsMacro(() => QuantSupportRibbon.Run(() =>
            {
                if (IsDisposed) return;
                dynamic app = ExcelDnaUtil.Application;
                if (BrowserPanes.ActiveWindowId() != WindowId || !string.Equals((string)app.ActiveWorkbook.Name, workbook, StringComparison.OrdinalIgnoreCase))
                    throw new InvalidOperationException("Activate the source workbook and refresh the browser.");
                var current = NativeBrowser.Read(workbook).Objects.FirstOrDefault(o => o.Handle == handle);
                if (current == null || !current.Locations.Any(l => l.SheetId == location.SheetId && l.Row == location.Row && l.Column == location.Column))
                    throw new InvalidOperationException("The object or its source has changed. Refresh the browser.");
                var reference = new ExcelReference(location.Row - 1, location.Row - 1, location.Column - 1, location.Column - 1, new IntPtr(unchecked((long)location.SheetId)));
                XlCall.Excel(XlCall.xlcFormulaGoto, reference);
            }));
        }
    }
}

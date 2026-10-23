using System;
using System.Collections.Generic;
using System.Drawing;
using System.Linq;
using System.Runtime.InteropServices;
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
        private ulong stamp;
        private string decimalSeparator = ".";
        private string detailsIdentity;
        private readonly List<BrowserObject> visibleObjects = new List<BrowserObject>();
        private readonly ComboBox filter = new ComboBox { DropDownStyle = ComboBoxStyle.DropDownList, Width = 170 };
        private readonly TextBox search = new TextBox { Width = 170 };
        private readonly LinkLabel status = new LinkLabel { Dock = DockStyle.Fill, AutoSize = true, Padding = new Padding(4), LinkArea = new LinkArea(0, 0) };
        private readonly DataGridView grid = new DataGridView
        {
            Dock = DockStyle.Fill, ReadOnly = true, MultiSelect = false,
            SelectionMode = DataGridViewSelectionMode.FullRowSelect, AllowUserToAddRows = false,
            AllowUserToDeleteRows = false, AllowUserToOrderColumns = true, RowHeadersVisible = false,
            AutoGenerateColumns = false, VirtualMode = true, BackgroundColor = SystemColors.Window,
            AutoSizeColumnsMode = DataGridViewAutoSizeColumnsMode.DisplayedCells
        };
        private readonly TextBox details = new TextBox
        {
            Dock = DockStyle.Fill, ReadOnly = true, Multiline = true, WordWrap = true,
            ScrollBars = ScrollBars.Vertical, BackColor = SystemColors.Window
        };
        private readonly ComboBox sources = new ComboBox { DropDownStyle = ComboBoxStyle.DropDownList, Width = 190, DisplayMember = "Address" };
        private readonly Button go = new Button { Text = "Go to cell", AutoSize = true, Enabled = false };
        private readonly Button copy = new Button { Text = "Copy handle", AutoSize = true, Enabled = false };
        private readonly Button price = new Button { Text = "Price selected", AutoSize = true, Enabled = false };
        private readonly TabControl tabs = new TabControl { Dock = DockStyle.Fill };
        private readonly TabPage detailsPage = new TabPage("Details");
        private readonly TabPage messagesPage = new TabPage("Messages");
        private readonly TextBox messages = new TextBox { Dock = DockStyle.Fill, ReadOnly = true, Multiline = true, WordWrap = true, ScrollBars = ScrollBars.Vertical, BackColor = SystemColors.Window };
        private readonly TabPage tradesPage = new TabPage("Pricing");
        private readonly TabPage curvesPage = new TabPage("Curves");
        private readonly ExplorerControl tradeExplorer;
        private readonly ExplorerControl curveExplorer;
        private bool populating;

        public BrowserControl()
        {
            Font = new Font("Segoe UI", 9);
            Dock = DockStyle.Fill;
            MaterialIcons.Apply(go, "open_in_new");
            MaterialIcons.Apply(copy, "content_copy");
            MaterialIcons.Apply(price, "calculate");
            MaterialIcons.Apply(tabs, "info", "warning", "calculate", "show_chart");
            detailsPage.ImageIndex = 0;
            messagesPage.ImageIndex = 1;
            tradesPage.ImageIndex = 2;
            curvesPage.ImageIndex = 3;
            TableClipboard.Configure(grid);
            grid.CellValueNeeded += (sender, args) =>
            {
                if (args.RowIndex < 0 || args.RowIndex >= visibleObjects.Count) return;
                var item = visibleObjects[args.RowIndex];
                switch (args.ColumnIndex)
                {
                    case 0: args.Value = item.Name; break;
                    case 1: args.Value = item.Kind; break;
                    case 2: args.Value = item.EvaluationDate; break;
                    case 3: args.Value = item.SnapshotTimestamp; break;
                    case 4: args.Value = item.QuoteLevel; break;
                    case 5: args.Value = item.CurrentInputs ? "Current" : "Recalculate"; break;
                }
            };
            filter.Items.AddRange(new object[] { "All objects", "Pricing contexts", "Instruments & Trades", "Portfolios", "Curves", "Market & configuration", "Pricing setup", "Results" });
            filter.SelectedIndex = 0;
            var toolbar = new TableLayoutPanel { Dock = DockStyle.Fill, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, ColumnCount = 3, RowCount = 2, Padding = new Padding(4), Margin = Padding.Empty };
            toolbar.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
            toolbar.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
            toolbar.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
            toolbar.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            toolbar.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            var refresh = new Button { Text = "Refresh", AutoSize = true };
            MaterialIcons.Apply(refresh, "refresh");
            refresh.Click += (s, e) => RefreshObjects(true);
            filter.Dock = search.Dock = DockStyle.Fill;
            toolbar.Controls.Add(filter, 0, 0);
            toolbar.SetColumnSpan(filter, 2);
            toolbar.Controls.Add(refresh, 2, 0);
            toolbar.Controls.Add(new Label { Text = "Search", AutoSize = true, Anchor = AnchorStyles.Left, Margin = new Padding(4, 4, 8, 4) }, 0, 1);
            toolbar.Controls.Add(search, 1, 1);
            toolbar.SetColumnSpan(search, 2);
            foreach (var title in new[] { "Name", "Type", "Valuation date", "Snapshot (UTC)", "Quote side", "Inputs" })
                grid.Columns.Add(new DataGridViewTextBoxColumn { HeaderText = title, SortMode = DataGridViewColumnSortMode.NotSortable });
            grid.Columns[0].MinimumWidth = 140;
            grid.Columns[1].Width = 110;
            grid.Columns[2].Width = 105;
            grid.Columns[3].Width = 165;
            grid.Columns[4].Width = 75;
            grid.Columns[5].Width = 90;
            var split = new SplitContainer { Dock = DockStyle.Fill, Orientation = Orientation.Horizontal, FixedPanel = FixedPanel.Panel1, Size = new Size(500, 700), SplitterDistance = 190, SplitterWidth = 6, Panel1MinSize = 80, Panel2MinSize = 180 };
            var listHeight = 190;
            var sizing = false;
            split.SplitterMoved += (sender, args) => { if (!sizing) listHeight = split.SplitterDistance; };
            split.SizeChanged += (sender, args) =>
            {
                if (split.ClientSize.Height < split.Panel1MinSize + split.Panel2MinSize + split.SplitterWidth) return;
                sizing = true;
                try { split.SplitterDistance = Math.Max(80, Math.Min(listHeight, split.ClientSize.Height - 266)); }
                finally { sizing = false; }
            };
            split.Panel1.Controls.Add(grid);
            tradeExplorer = new ExplorerControl(true, () => WindowId);
            curveExplorer = new ExplorerControl(false, () => WindowId);
            detailsPage.Controls.Add(details);
            messagesPage.Controls.Add(messages);
            tradesPage.Controls.Add(tradeExplorer);
            curvesPage.Controls.Add(curveExplorer);
            tabs.TabPages.Add(detailsPage);
            split.Panel2.Controls.Add(tabs);
            var navigation = new FlowLayoutPanel { Dock = DockStyle.Fill, AutoSize = true, Padding = new Padding(4) };
            navigation.Controls.Add(sources);
            navigation.Controls.Add(go);
            navigation.Controls.Add(copy);
            navigation.Controls.Add(price);
            var layout = new TableLayoutPanel { Dock = DockStyle.Fill, ColumnCount = 1, RowCount = 4 };
            layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
            layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            layout.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
            layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            layout.Controls.Add(toolbar, 0, 0);
            layout.Controls.Add(split, 0, 1);
            layout.Controls.Add(navigation, 0, 2);
            layout.Controls.Add(status, 0, 3);
            Controls.Add(layout);
            SizeChanged += (sender, args) =>
            {
                var width = Math.Max(1, ClientSize.Width - 8);
                toolbar.MaximumSize = navigation.MaximumSize = status.MaximumSize = new Size(width, 0);
            };
            filter.SelectedIndexChanged += (s, e) => { if (!populating) Populate(); };
            search.TextChanged += (s, e) => { if (!populating) Populate(); };
            grid.CurrentCellChanged += (s, e) => { if (!populating) ShowDetails(); };
            tabs.SelectedIndexChanged += (s, e) => BindSelectedExplorer();
            go.Click += (s, e) => QuantSupportRibbon.Run(GoToSource);
            copy.Click += (s, e) => QuantSupportRibbon.Run(() => { if (Selected != null) Clipboard.SetText(Selected.Handle); });
            price.Click += (s, e) => ShowPricing();
            status.LinkClicked += (s, e) => { if (tabs.TabPages.Contains(messagesPage)) tabs.SelectedTab = messagesPage; };
        }

        private BrowserObject Selected
        {
            get
            {
                var row = grid.CurrentCell?.RowIndex ?? -1;
                return row >= 0 && row < visibleObjects.Count ? visibleObjects[row] : null;
            }
        }

        internal void SetFilter(string value)
        {
            if (value == null || ((string)filter.SelectedItem == value && search.TextLength == 0)) return;
            populating = true;
            try { search.Clear(); filter.SelectedItem = value; }
            finally { populating = false; }
            Populate();
        }

        internal void RefreshObjects(bool force)
        {
            try
            {
                if (BrowserPanes.ActiveWindowId() != WindowId) return;
                dynamic app = ExcelDnaUtil.Application;
                if (!(bool)app.Ready || (int)app.CalculationState == 1) return;
                var workbook = (string)app.ActiveWorkbook.Name;
                var nextSeparator = BrowserPanes.DecimalSeparator(app);
                var formatChanged = decimalSeparator != nextSeparator;
                BrowserDisplay.DecimalSeparator = decimalSeparator = nextSeparator;
                var revision = NativeBrowser.Revision();
                if (!force && snapshot != null && snapshot.Workbook == workbook && stamp == revision)
                {
                    if (formatChanged) { details.Text = BrowserDisplay.Describe(Selected); tradeExplorer.RefreshFormats(); curveExplorer.RefreshFormats(); }
                    return;
                }
                snapshot = NativeBrowser.Read(workbook);
                stamp = revision;
                Populate();
            }
            catch (Exception error)
            {
                snapshot = null;
                stamp = 0;
                grid.Rows.Clear();
                visibleObjects.Clear();
                details.Clear();
                tradeExplorer.Bind(null, null);
                curveExplorer.Bind(null, null);
                sources.Items.Clear();
                go.Enabled = copy.Enabled = price.Enabled = false;
                status.ForeColor = Color.Firebrick;
                status.Text = error.Message;
            }
        }

        internal void InvalidateObjects()
        {
            snapshot = null;
            stamp = 0;
            Populate();
            status.Text = "Native objects cleared. Rebuilding open workbooks…";
        }

        private void Populate()
        {
            var selected = Selected?.Identity;
            populating = true;
            grid.SuspendLayout();
            grid.Rows.Clear();
            visibleObjects.Clear();
            if (snapshot == null) { grid.ResumeLayout(); populating = false; ShowDetails(); return; }
            var category = (string)filter.SelectedItem;
            var query = search.Text.Trim();
            foreach (var item in snapshot.Objects)
            {
                var matchesCategory = category == "All objects" || item.Family == category ||
                    (category == "Curves" && item.Curves != null && item.Curves.Count > 0) ||
                    (category == "Instruments & Trades" && (item.Family == "Instruments" || item.Family == "Trades"));
                if (!matchesCategory || (query.Length > 0 &&
                    (item.Name + " " + item.Kind + " " + item.Handle).IndexOf(query, StringComparison.OrdinalIgnoreCase) < 0)) continue;
                visibleObjects.Add(item);
            }
            grid.RowCount = visibleObjects.Count;
            // Keep the list useful for instruments, whose market-date columns are empty.
            grid.Columns[2].Visible = visibleObjects.Any(value => !string.IsNullOrEmpty(value.EvaluationDate));
            grid.Columns[3].Visible = visibleObjects.Any(value => !string.IsNullOrEmpty(value.SnapshotTimestamp));
            grid.Columns[4].Visible = visibleObjects.Any(value => !string.IsNullOrEmpty(value.QuoteLevel));
            grid.Columns[5].Visible = visibleObjects.Any(value => !value.CurrentInputs);
            var selectedIndex = visibleObjects.FindIndex(item => item.Identity == selected);
            if (selectedIndex >= 0) grid.CurrentCell = grid.Rows[selectedIndex].Cells[0];
            grid.ResumeLayout();
            populating = false;
            ShowDetails();
            var errors = snapshot.Diagnostics;
            status.ForeColor = errors != null && errors.Count > 0 ? Color.Firebrick : SystemColors.ControlText;
            status.Text = errors != null && errors.Count > 0
                ? errors.Count + (errors.Count == 1 ? " worksheet error. " : " worksheet errors. ") + "Open Messages for the library details."
                : snapshot.Workbook + " · " + grid.Rows.Count + " of " + snapshot.Objects.Count + " objects";
            status.LinkArea = new LinkArea(0, errors != null && errors.Count > 0 ? status.Text.Length : 0);


        }

        private void ShowDetails()
        {
            sources.Items.Clear();
            var item = Selected;
            tradeExplorer.CancelRequests();
            curveExplorer.CancelRequests();
            copy.Enabled = item != null;
            price.Enabled = BrowserDisplay.CanPrice(item);
            go.Enabled = item != null && item.Locations.Count > 0;
            foreach (var cell in item?.Locations ?? new List<Location>()) sources.Items.Add(cell);
            if (sources.Items.Count > 0) sources.SelectedIndex = 0;
            details.Text = BrowserDisplay.Describe(item);
            var previousTab = tabs.SelectedTab;
            populating = true;
            tabs.TabPages.Clear();
            tabs.TabPages.Add(detailsPage);
            var diagnostics = snapshot?.Diagnostics ?? new List<Diagnostic>();
            messages.Text = string.Join(Environment.NewLine + Environment.NewLine, diagnostics.Select(error => new Location { Sheet = error.Sheet, Row = error.Row, Column = error.Column }.Address + Environment.NewLine + error.Message));
            if (diagnostics.Count > 0) tabs.TabPages.Add(messagesPage);
            if (BrowserDisplay.CanPrice(item)) tabs.TabPages.Add(tradesPage);
            if (item?.Curves != null && item.Curves.Count > 0) tabs.TabPages.Add(curvesPage);
            var preferred = item == null && diagnostics.Count > 0 ? messagesPage : BrowserDisplay.CanPrice(item) ? tradesPage : tabs.TabPages.Contains(curvesPage) ? curvesPage : detailsPage;
            tabs.SelectedTab = item != null && detailsIdentity == item.Identity && tabs.TabPages.Contains(previousTab) ? previousTab : preferred;
            detailsIdentity = item?.Identity;
            populating = false;
            if (item == null) { tradeExplorer.Bind(null, null); curveExplorer.Bind(null, null); }
            else BindSelectedExplorer();
        }

        private void BindSelectedExplorer()
        {
            if (populating) return;
            if (tabs.SelectedTab == tradesPage) tradeExplorer.Bind(snapshot, Selected);
            if (tabs.SelectedTab == curvesPage) curveExplorer.Bind(snapshot, Selected);
        }

        internal void ShowPricing()
        {
            if (!BrowserDisplay.CanPrice(Selected))
            {
                status.Text = "Select an instrument or trade in the list to open Pricing.";
                return;
            }
            tabs.SelectedTab = tradesPage;
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing)
            {
                populating = true;
                detailsPage.Dispose();
                messagesPage.Dispose();
                tradesPage.Dispose();
                curvesPage.Dispose();
            }
            base.Dispose(disposing);
        }

        internal void Inspect(string handle)
        {
            search.Clear();
            var index = visibleObjects.FindIndex(item => item.Handle == handle);
            if (!string.IsNullOrEmpty(handle) && index >= 0) { grid.CurrentCell = grid.Rows[index].Cells[0]; return; }
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

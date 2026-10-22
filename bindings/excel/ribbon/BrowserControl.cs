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
        private readonly Label status = new Label { Dock = DockStyle.Fill, AutoSize = true, Padding = new Padding(4) };
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
            Dock = DockStyle.Fill, ReadOnly = true, Multiline = true, WordWrap = false,
            ScrollBars = ScrollBars.Both, BackColor = SystemColors.Window
        };
        private readonly ComboBox sources = new ComboBox { DropDownStyle = ComboBoxStyle.DropDownList, Width = 190, DisplayMember = "Address" };
        private readonly Button go = new Button { Text = "Go to cell", AutoSize = true, Enabled = false };
        private readonly Button copy = new Button { Text = "Copy handle", AutoSize = true, Enabled = false };
        private readonly Button price = new Button { Text = "Price selected", AutoSize = true, Enabled = false };
        private readonly TabControl tabs = new TabControl { Dock = DockStyle.Fill };
        private readonly TabPage detailsPage = new TabPage("Details");
        private readonly TabPage tradesPage = new TabPage("Pricing");
        private readonly TabPage curvesPage = new TabPage("Curves");
        private readonly ExplorerControl tradeExplorer;
        private readonly ExplorerControl curveExplorer;
        private bool populating;

        public BrowserControl()
        {
            Font = new Font("Segoe UI", 9);
            Dock = DockStyle.Fill;
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
            var toolbar = new FlowLayoutPanel { Dock = DockStyle.Fill, AutoSize = true, Padding = new Padding(4) };
            var refresh = new Button { Text = "Refresh", AutoSize = true };
            refresh.Click += (s, e) => RefreshObjects(true);
            toolbar.Controls.Add(filter);
            toolbar.Controls.Add(new Label { Text = "Search", AutoSize = true, Margin = new Padding(4, 7, 2, 0) });
            toolbar.Controls.Add(search);
            toolbar.Controls.Add(refresh);
            foreach (var title in new[] { "Name", "Type", "Valuation date", "Snapshot (UTC)", "Quote side", "Inputs" })
                grid.Columns.Add(new DataGridViewTextBoxColumn { HeaderText = title, SortMode = DataGridViewColumnSortMode.NotSortable });
            var split = new SplitContainer { Dock = DockStyle.Fill, Orientation = Orientation.Horizontal, Size = new Size(500, 700), SplitterDistance = 210 };
            split.Panel1.Controls.Add(grid);
            tradeExplorer = new ExplorerControl(true, () => WindowId);
            curveExplorer = new ExplorerControl(false, () => WindowId);
            detailsPage.Controls.Add(details);
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
            grid.SelectionChanged += (s, e) => { if (!populating) ShowDetails(); };
            tabs.SelectedIndexChanged += (s, e) => BindSelectedExplorer();
            go.Click += (s, e) => QuantSupportRibbon.Run(GoToSource);
            copy.Click += (s, e) => QuantSupportRibbon.Run(() => { if (Selected != null) Clipboard.SetText(Selected.Handle); });
            price.Click += (s, e) => ShowPricing();
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
            var selectedIndex = visibleObjects.FindIndex(item => item.Identity == selected);
            if (selectedIndex >= 0) grid.CurrentCell = grid.Rows[selectedIndex].Cells[0];
            grid.ResumeLayout();
            populating = false;
            ShowDetails();
            status.Text = snapshot.Workbook + " · " + grid.Rows.Count + " of " + snapshot.Objects.Count + " live objects" +
                (snapshot.Objects.Count == 0 ? ". Calculate the workbook to create its objects." : ". Updates when native objects change.");
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
            if (BrowserDisplay.CanPrice(item)) tabs.TabPages.Add(tradesPage);
            if (item?.Curves != null && item.Curves.Count > 0) tabs.TabPages.Add(curvesPage);
            var preferred = BrowserDisplay.CanPrice(item) ? tradesPage : tabs.TabPages.Contains(curvesPage) ? curvesPage : detailsPage;
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

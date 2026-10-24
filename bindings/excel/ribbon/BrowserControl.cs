using System;
using System.Collections.Generic;
using System.Drawing;
using System.Linq;
using System.Threading.Tasks;
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
        private readonly Button showCurves = new Button { Text = "View curves", AutoSize = true, Enabled = false };
        private readonly TabControl tabs = new TabControl { Dock = DockStyle.Fill };
        private readonly TabPage objectsPage = new TabPage("Objects");
        private readonly TabPage messagesPage = new TabPage("Messages");
        private readonly TextBox messages = new TextBox { Dock = DockStyle.Fill, ReadOnly = true, Multiline = true, WordWrap = true, ScrollBars = ScrollBars.Vertical, BackColor = SystemColors.Window };
        private readonly TabPage tradesPage = new TabPage("Pricing");
        private readonly TabPage curvesPage = new TabPage("Curves");
        private readonly ObjectPicker pricingObjects = ObjectChoice();
        private readonly ObjectPicker curveObjects = ObjectChoice();
        private readonly ExplorerControl tradeExplorer;
        private readonly ExplorerControl curveExplorer;
        private bool populating;
        private bool pricingDirty = true;
        private bool curvesDirty = true;
        private bool loading;
        private string loadingWorkbook;
        private long refreshGeneration;
        private Task refreshTask = Task.CompletedTask;
        private bool pendingSelection, pendingPricing;
        private string pendingHandle;

        private static ObjectPicker ObjectChoice() => new ObjectPicker(true) { Dock = DockStyle.Fill };

        public BrowserControl()
        {
            Font = new Font("Segoe UI", 9);
            Dock = DockStyle.Fill;
            MaterialIcons.Apply(go, "open_in_new");
            MaterialIcons.Apply(copy, "content_copy");
            MaterialIcons.Apply(price, "calculate");
            MaterialIcons.Apply(showCurves, "show_chart");
            MaterialIcons.Apply(tabs, "info", "warning", "calculate", "show_chart");
            objectsPage.ImageIndex = 0;
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
            var refresh = RefreshButton();
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
            var split = new SplitContainer { Dock = DockStyle.Fill, Orientation = Orientation.Horizontal, Size = new Size(500, 600), SplitterDistance = 350, SplitterWidth = 6, Panel1MinSize = 80, Panel2MinSize = 80 };
            split.Panel1.Controls.Add(grid);
            split.Panel2.Controls.Add(details);
            var objectLayout = new TableLayoutPanel { Dock = DockStyle.Fill, ColumnCount = 1, RowCount = 3, Margin = Padding.Empty };
            objectLayout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
            objectLayout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            objectLayout.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
            objectLayout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            objectLayout.Controls.Add(toolbar, 0, 0);
            objectLayout.Controls.Add(split, 0, 1);
            var objectActions = new FlowLayoutPanel { Dock = DockStyle.Fill, AutoSize = true, Margin = Padding.Empty };
            objectActions.Controls.Add(price);
            objectActions.Controls.Add(showCurves);
            objectLayout.Controls.Add(objectActions, 0, 2);
            objectsPage.Controls.Add(objectLayout);
            tradeExplorer = new ExplorerControl(true, () => WindowId);
            curveExplorer = new ExplorerControl(false, () => WindowId);
            messagesPage.Controls.Add(messages);
            AddWorkspace(tradesPage, "Instrument / trade", pricingObjects, tradeExplorer);
            AddWorkspace(curvesPage, "Market / curve", curveObjects, curveExplorer);
            tabs.TabPages.AddRange(new[] { objectsPage, tradesPage, curvesPage });
            var navigation = new TableLayoutPanel { Dock = DockStyle.Fill, AutoSize = true, ColumnCount = 3, RowCount = 1, Padding = new Padding(4), Margin = Padding.Empty };
            navigation.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
            navigation.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
            navigation.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
            sources.Dock = DockStyle.Fill;
            navigation.Controls.Add(sources, 0, 0);
            navigation.Controls.Add(go, 1, 0);
            navigation.Controls.Add(copy, 2, 0);
            var layout = new TableLayoutPanel { Dock = DockStyle.Fill, ColumnCount = 1, RowCount = 3 };
            layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
            layout.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
            layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            layout.Controls.Add(tabs, 0, 0);
            layout.Controls.Add(navigation, 0, 1);
            layout.Controls.Add(status, 0, 2);
            Controls.Add(layout);
            SizeChanged += (sender, args) =>
            {
                var width = Math.Max(1, ClientSize.Width - 8);
                toolbar.MaximumSize = navigation.MaximumSize = status.MaximumSize = new Size(width, 0);
            };
            filter.SelectedIndexChanged += (s, e) => { if (!populating) PopulateList(false); };
            search.TextChanged += (s, e) => { if (!populating) PopulateList(false); };
            grid.CurrentCellChanged += (s, e) => { if (!populating) ShowDetails(); };
            tabs.SelectedIndexChanged += (s, e) => { BindSelectedExplorer(); UpdateNavigation(); };
            pricingObjects.SelectedIndexChanged += (s, e) => WorkspaceSelectionChanged(true);
            curveObjects.SelectedIndexChanged += (s, e) => WorkspaceSelectionChanged(false);
            go.Click += (s, e) => QuantSupportRibbon.Run(GoToSource);
            copy.Click += (s, e) => QuantSupportRibbon.Run(() => { if (Selected != null) Clipboard.SetText(Selected.Handle); });
            price.Click += (s, e) => ShowPricing();
            showCurves.Click += (s, e) => ShowCurves();
            status.LinkClicked += (s, e) => { if (tabs.TabPages.Contains(messagesPage)) tabs.SelectedTab = messagesPage; };
        }

        private Button RefreshButton()
        {
            var button = new Button { Text = "Refresh", AutoSize = true };
            MaterialIcons.Apply(button, "refresh");
            button.Click += (s, e) => RefreshObjects(true);
            return button;
        }

        private void AddWorkspace(TabPage page, string label, ObjectPicker choice, ExplorerControl explorer)
        {
            var header = new TableLayoutPanel { Dock = DockStyle.Fill, AutoSize = true, ColumnCount = 2, RowCount = 2, Padding = new Padding(4), Margin = Padding.Empty };
            header.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
            header.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
            header.Controls.Add(new Label { Text = label, AutoSize = true, Anchor = AnchorStyles.Left }, 0, 0);
            header.Controls.Add(choice, 0, 1);
            header.Controls.Add(RefreshButton(), 1, 1);
            choice.AccessibleName = label;
            var layout = new TableLayoutPanel { Dock = DockStyle.Fill, ColumnCount = 1, RowCount = 2, Margin = Padding.Empty };
            layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
            layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            layout.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
            layout.Controls.Add(header, 0, 0);
            layout.Controls.Add(explorer, 0, 1);
            page.Controls.Add(layout);
        }

        private BrowserObject GridSelection
        {
            get
            {
                var row = grid.CurrentCell?.RowIndex ?? -1;
                return row >= 0 && row < visibleObjects.Count ? visibleObjects[row] : null;
            }
        }

        private BrowserObject Selected
        {
            get
            {
                if (tabs.SelectedTab == tradesPage) return pricingObjects.SelectedItem as BrowserObject;
                if (tabs.SelectedTab == curvesPage) return curveObjects.SelectedItem as BrowserObject;
                return tabs.SelectedTab == objectsPage ? GridSelection : null;
            }
        }

        internal void SetFilter(string value)
        {
            if (value == null) return;
            tabs.SelectedTab = value == "Curves" ? curvesPage : value == "Instruments & Trades" ? tradesPage : objectsPage;
            populating = true;
            try { search.Clear(); filter.SelectedItem = value; }
            finally { populating = false; }
            PopulateList(false);
        }

        internal Task RefreshObjects(bool force)
        {
            try
            {
                if (BrowserPanes.ActiveWindowId() != WindowId) return Task.CompletedTask;
                dynamic app = ExcelDnaUtil.Application;
                if (!(bool)app.Ready || (int)app.CalculationState == 1) return Task.CompletedTask;
                var workbook = (string)app.ActiveWorkbook.Name;
                var nextSeparator = BrowserPanes.DecimalSeparator(app);
                var formatChanged = decimalSeparator != nextSeparator;
                BrowserDisplay.DecimalSeparator = decimalSeparator = nextSeparator;
                if (loading && loadingWorkbook == workbook) return refreshTask;
                var revision = NativeBrowser.Revision();
                if (!force && snapshot != null && snapshot.Workbook == workbook && stamp == revision)
                {
                    if (formatChanged) { details.Text = BrowserDisplay.Describe(GridSelection); tradeExplorer.RefreshFormats(); curveExplorer.RefreshFormats(); }
                    return Task.CompletedTask;
                }
                loading = true; loadingWorkbook = workbook;
                status.ForeColor = SystemColors.ControlText;
                status.Text = "Loading workbook objects…";
                var generation = ++refreshGeneration;
                return refreshTask = ApplySnapshot(NativeBrowser.ReadAsync(workbook), workbook, revision, generation);
            }
            catch (Exception error)
            {
                ShowLoadError(error);
                return Task.CompletedTask;
            }
        }

        private async Task ApplySnapshot(Task<Snapshot> read, string workbook, ulong revision, long generation)
        {
            try {
                var next = await read.ConfigureAwait(false);
                if (IsDisposed) return;
                await NativeBrowser.OnExcel(() => {
                    if (IsDisposed || generation != refreshGeneration) return;
                    loading = false;
                    dynamic app = ExcelDnaUtil.Application;
                    if (BrowserPanes.ActiveWindowId() != WindowId || !string.Equals((string)app.ActiveWorkbook.Name, workbook, StringComparison.OrdinalIgnoreCase)) return;
                    snapshot = next; stamp = revision;
                    Populate();
                    if (pendingSelection) { pendingSelection = false; Inspect(pendingHandle); }
                    if (pendingPricing) { pendingPricing = false; ShowPricing(); }
                }).ConfigureAwait(false);
            } catch (Exception error) {
                if (IsDisposed) return;
                await NativeBrowser.OnExcel(() => { if (!IsDisposed && generation == refreshGeneration) ShowLoadError(error); }).ConfigureAwait(false);
            }
        }

        private void ShowLoadError(Exception error)
        {
            loading = false; snapshot = null; stamp = 0;
            Populate();
            status.ForeColor = Color.Firebrick;
            status.Text = error.Message;
        }

        internal void InvalidateObjects()
        {
            refreshGeneration++; loading = false;
            snapshot = null;
            stamp = 0;
            Populate();
            status.Text = "Native objects cleared. Rebuilding open workbooks…";
        }

        private void Populate() { PopulateList(true); }

        private void PopulateList(bool snapshotChanged)
        {
            var selected = GridSelection?.Identity;
            populating = true;
            grid.SuspendLayout();
            grid.Rows.Clear();
            visibleObjects.Clear();
            var objects = snapshot?.Objects ?? new List<BrowserObject>();
            if (snapshotChanged)
            {
                PopulateChoices(pricingObjects, objects.Where(BrowserDisplay.CanPrice));
                PopulateChoices(curveObjects, objects.Where(value => value.Curves?.Count > 0));
                tradeExplorer.CancelRequests();
                curveExplorer.CancelRequests();
                pricingDirty = curvesDirty = true;
            }
            var category = (string)filter.SelectedItem;
            var query = search.Text.Trim();
            foreach (var item in objects)
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
            var errors = snapshot?.Diagnostics ?? new List<Diagnostic>();
            messages.Text = string.Join(Environment.NewLine + Environment.NewLine, errors.Select(error => new Location { Sheet = error.Sheet, Row = error.Row, Column = error.Column }.Address + Environment.NewLine + error.Message));
            if (errors.Count > 0 && !tabs.TabPages.Contains(messagesPage)) tabs.TabPages.Add(messagesPage);
            else if (errors.Count == 0) tabs.TabPages.Remove(messagesPage);
            populating = false;
            ShowDetails();
            status.ForeColor = errors.Count > 0 ? Color.Firebrick : SystemColors.ControlText;
            status.Text = errors.Count > 0
                ? errors.Count + (errors.Count == 1 ? " worksheet error. " : " worksheet errors. ") + "Open Messages for the library details."
                : snapshot == null ? "No objects loaded. Refresh after recalculating the workbook."
                : snapshot.Workbook + " · " + objects.Count + " objects";
            status.LinkArea = new LinkArea(0, errors.Count > 0 ? status.Text.Length : 0);
        }

        private static void PopulateChoices(ObjectPicker choice, IEnumerable<BrowserObject> objects)
        {
            choice.SetObjects(objects);
        }

        private void ShowDetails()
        {
            var item = GridSelection;
            details.Text = BrowserDisplay.Describe(item);
            if (tabs.SelectedTab == objectsPage)
            {
                if (BrowserDisplay.CanPrice(item)) pricingObjects.SelectedItem = item;
                if (item?.Curves?.Count > 0) curveObjects.SelectedItem = item;
            }
            price.Enabled = BrowserDisplay.CanPrice(item);
            showCurves.Enabled = item?.Curves?.Count > 0;
            BindSelectedExplorer();
            UpdateNavigation();
        }

        private void UpdateNavigation()
        {
            if (populating) return;
            var previous = (sources.SelectedItem as Location)?.Address;
            sources.Items.Clear();
            var item = Selected;
            copy.Enabled = item != null;
            go.Enabled = item?.Locations?.Count > 0;
            foreach (var cell in item?.Locations ?? new List<Location>()) sources.Items.Add(cell);
            var match = sources.Items.OfType<Location>().FirstOrDefault(cell => cell.Address == previous);
            if (match != null) sources.SelectedItem = match;
            else if (sources.Items.Count > 0) sources.SelectedIndex = 0;
        }

        private void WorkspaceSelectionChanged(bool pricing)
        {
            if (populating) return;
            if (pricing) { pricingDirty = true; tradeExplorer.CancelRequests(); }
            else { curvesDirty = true; curveExplorer.CancelRequests(); }
            BindSelectedExplorer();
            UpdateNavigation();
        }

        private void BindSelectedExplorer()
        {
            if (populating) return;
            if (tabs.SelectedTab == tradesPage && pricingDirty) { tradeExplorer.Bind(snapshot, Selected); pricingDirty = false; }
            if (tabs.SelectedTab == curvesPage && curvesDirty) { curveExplorer.Bind(snapshot, Selected); curvesDirty = false; }
        }

        internal void ShowPricing()
        {
            if (loading || snapshot == null) { pendingPricing = true; return; }
            if (tabs.SelectedTab == objectsPage && BrowserDisplay.CanPrice(GridSelection)) pricingObjects.SelectedItem = GridSelection;
            if (pricingObjects.SelectedItem == null)
            {
                status.Text = "Select an instrument or trade in the list to open Pricing.";
                return;
            }
            tabs.SelectedTab = tradesPage;
            BindSelectedExplorer();
        }

        internal void ShowCurves()
        {
            if (tabs.SelectedTab == objectsPage && GridSelection?.Curves?.Count > 0) curveObjects.SelectedItem = GridSelection;
            tabs.SelectedTab = curvesPage;
            BindSelectedExplorer();
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing)
            {
                refreshGeneration++;
                populating = true;
                objectsPage.Dispose();
                messagesPage.Dispose();
                tradesPage.Dispose();
                curvesPage.Dispose();
            }
            base.Dispose(disposing);
        }

        internal void Inspect(string handle)
        {
            tabs.SelectedTab = objectsPage;
            if (loading || snapshot == null) { pendingSelection = true; pendingHandle = handle; return; }
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
            ExcelAsyncUtil.QueueAsMacro(async () =>
            {
                try {
                    if (IsDisposed) return;
                    dynamic app = ExcelDnaUtil.Application;
                    if (BrowserPanes.ActiveWindowId() != WindowId || !string.Equals((string)app.ActiveWorkbook.Name, workbook, StringComparison.OrdinalIgnoreCase))
                        throw new InvalidOperationException("Activate the source workbook and refresh the browser.");
                    var latest = await NativeBrowser.ReadAsync(workbook).ConfigureAwait(false);
                    await NativeBrowser.OnExcel(() => {
                        if (IsDisposed) return;
                        dynamic active = ExcelDnaUtil.Application;
                        if (BrowserPanes.ActiveWindowId() != WindowId || !string.Equals((string)active.ActiveWorkbook.Name, workbook, StringComparison.OrdinalIgnoreCase))
                            throw new InvalidOperationException("Activate the source workbook and refresh the browser.");
                        var current = latest.Objects.FirstOrDefault(o => o.Handle == handle);
                        if (current == null || !current.Locations.Any(l => l.SheetId == location.SheetId && l.Row == location.Row && l.Column == location.Column))
                            throw new InvalidOperationException("The object or its source has changed. Refresh the browser.");
                        var reference = new ExcelReference(location.Row - 1, location.Row - 1, location.Column - 1, location.Column - 1, new IntPtr(unchecked((long)location.SheetId)));
                        XlCall.Excel(XlCall.xlcFormulaGoto, reference);
                    }).ConfigureAwait(false);
                } catch (Exception error) {
                    await NativeBrowser.OnExcel(() => { if (!IsDisposed) QuantSupportRibbon.Run(() => { throw error; }); }).ConfigureAwait(false);
                }
            });
        }
    }
}

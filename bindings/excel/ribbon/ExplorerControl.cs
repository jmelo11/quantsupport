using System;
using System.Collections.Generic;
using System.Drawing;
using System.Globalization;
using System.Linq;
using System.Text;
using System.Windows.Forms;
using ExcelDna.Integration;

namespace QuantSupport.Ribbon
{
    // Only owned snapshots and numeric window identities survive callbacks.
    internal sealed class ExplorerControl : UserControl
    {
        private readonly bool trades;
        private readonly Func<long> windowId;
        private readonly ComboBox context = Choice();
        private readonly ComboBox pricer = Choice();
        private readonly ComboBox curve = Choice();
        private readonly ComboBox view = Choice();
        private readonly DateTimePicker tradeDate = new DateTimePicker { Format = DateTimePickerFormat.Custom, CustomFormat = "yyyy-MM-dd", Width = 120 };
        private readonly ComboBox side = Choice();
        private readonly ComboBox anchor = Choice();
        private readonly TextBox anchorValue = new TextBox { Width = 120 };
        private readonly List<Control> positionControls = new List<Control>();
        private readonly List<Control> bondControls = new List<Control>();
        private readonly NumericUpDown target = new NumericUpDown { Minimum = 0, Maximum = uint.MaxValue, Width = 75 };
        private readonly Label targetLabel = new Label { Text = "Leg / segment (0-based)", AutoSize = true, Padding = new Padding(0, 6, 0, 0) };
        private readonly Button run = new Button { Text = "Run request", AutoSize = true };
        private readonly Label message = new Label { AutoSize = true, Dock = DockStyle.Fill, Padding = new Padding(4), MaximumSize = new Size(700, 0) };
        private readonly DataGridView table = new DataGridView
        {
            Dock = DockStyle.Fill, ReadOnly = true, MultiSelect = false,
            SelectionMode = DataGridViewSelectionMode.FullRowSelect, AllowUserToAddRows = false,
            AllowUserToDeleteRows = false, RowHeadersVisible = false,
            BackgroundColor = SystemColors.Window, AutoSizeColumnsMode = DataGridViewAutoSizeColumnsMode.DisplayedCells
        };
        private readonly TextBox parameters = new TextBox
        {
            Dock = DockStyle.Fill, ReadOnly = true, Multiline = true,
            ScrollBars = ScrollBars.Both, WordWrap = false, BackColor = SystemColors.Window
        };
        private Snapshot snapshot;
        private BrowserObject item;
        private BrowserResponse response;
        private bool binding;
        private int generation;
        private string positionDefaultsKey;

        private static ComboBox Choice() => new ComboBox { DropDownStyle = ComboBoxStyle.DropDownList, Width = 210, DisplayMember = "DisplayName", DropDownWidth = 420 };

        internal ExplorerControl(bool trades, Func<long> windowId)
        {
            this.trades = trades;
            this.windowId = windowId;
            Dock = DockStyle.Fill;
            TableClipboard.Configure(table);
            table.CellFormatting += (sender, args) =>
            {
                if (response?.NumericColumns == null || !response.NumericColumns.Contains(args.ColumnIndex)) return;
                args.Value = BrowserDisplay.Number(Convert.ToString(args.Value));
                args.CellStyle.Alignment = DataGridViewContentAlignment.MiddleRight;
                args.FormattingApplied = true;
            };
            var toolbar = new FlowLayoutPanel { Dock = DockStyle.Fill, AutoSize = true, Padding = new Padding(4), WrapContents = true };
            run.Text = trades ? "Run request" : "Refresh table";
            toolbar.Controls.Add(run);
            Action<string, Control> add = (label, control) => {
                toolbar.Controls.Add(new Label { Text = label, AutoSize = true, Padding = new Padding(0, 6, 0, 0) });
                toolbar.Controls.Add(control);
            };
            add(trades ? "Pricing context" : "Context (for curve inputs)", context);
            if (trades) add("Pricer", pricer);
            else add("Curve", curve);
            add(trades ? "Request" : "View", view);
            view.Items.AddRange(trades ? new object[] { "PV", "Cashflows", "Fair rate" } : new object[] { "Curve table", "Instruments & inputs" });
            view.SelectedIndex = 0;
            if (trades) { toolbar.Controls.Add(targetLabel); toolbar.Controls.Add(target); }
            if (trades)
            {
                side.Items.AddRange(new object[] { "Receive", "Pay" });
                side.SelectedIndex = 0;
                anchor.Items.AddRange(new object[] { "CleanPrice", "DirtyPrice", "ZSpread" });
                Action<string, Control, List<Control>> position = (label, control, group) =>
                {
                    var heading = new Label { Text = label, AutoSize = true, Padding = new Padding(0, 6, 0, 0) };
                    toolbar.Controls.Add(heading);
                    toolbar.Controls.Add(control);
                    group.Add(heading);
                    group.Add(control);
                };
                position("Preview trade date", tradeDate, positionControls);
                position("Side", side, positionControls);
                position("Bond anchor", anchor, bondControls);
                position("Anchor value", anchorValue, bondControls);
            }
            var split = new SplitContainer { Dock = DockStyle.Fill, Orientation = Orientation.Horizontal, Size = new Size(500, 300), SplitterDistance = 190 };
            split.Panel1.Controls.Add(table);
            split.Panel2.Controls.Add(parameters);
            split.Panel2Collapsed = trades;
            var layout = new TableLayoutPanel { Dock = DockStyle.Fill, ColumnCount = 1, RowCount = 3 };
            layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
            layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            layout.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
            layout.Controls.Add(toolbar, 0, 0);
            layout.Controls.Add(message, 0, 1);
            layout.Controls.Add(split, 0, 2);
            Controls.Add(layout);
            SizeChanged += (sender, args) =>
            {
                var width = Math.Max(1, ClientSize.Width - 8);
                toolbar.MaximumSize = new Size(width, 0);
                message.MaximumSize = new Size(width, 0);
            };
            run.Click += (sender, args) => RunQuery();
            context.SelectedIndexChanged += (sender, args) => { UpdatePositionDefaults(); OptionsChanged(); };
            pricer.SelectedIndexChanged += (sender, args) => OptionsChanged();
            curve.SelectedIndexChanged += (sender, args) => OptionsChanged();
            view.SelectedIndexChanged += (sender, args) => OptionsChanged();
            target.ValueChanged += (sender, args) => OptionsChanged();
            tradeDate.ValueChanged += (sender, args) => OptionsChanged();
            side.SelectedIndexChanged += (sender, args) => OptionsChanged();
            anchor.SelectedIndexChanged += (sender, args) => OptionsChanged();
            anchorValue.TextChanged += (sender, args) => OptionsChanged();
            table.SelectionChanged += (sender, args) => ShowParameters();
            Bind(null, null);
        }

        private static void Objects(ComboBox choice, IEnumerable<BrowserObject> objects, bool optional)
        {
            var previous = (choice.SelectedItem as BrowserObject)?.Name;
            choice.Items.Clear();
            if (optional) choice.Items.Add("(Use selected object)");
            foreach (var value in objects) choice.Items.Add(value);
            var match = choice.Items.OfType<BrowserObject>().FirstOrDefault(value => value.Name == previous);
            if (match != null) choice.SelectedItem = match;
            else if (choice.Items.Count > 0) choice.SelectedIndex = 0;
        }

        internal void Bind(Snapshot next, BrowserObject selected)
        {
            binding = true;
            snapshot = next;
            item = selected;
            var objects = next?.Objects ?? new List<BrowserObject>();
            Objects(context, objects.Where(value => value.Kind == "PricingContext"), !trades);
            Objects(pricer, objects.Where(value => value.Kind == "Pricer"), false);
            var previousCurve = curve.SelectedItem as string;
            curve.Items.Clear();
            foreach (var name in selected?.Curves ?? new List<string>()) curve.Items.Add(name);
            if (previousCurve != null && curve.Items.Contains(previousCurve)) curve.SelectedItem = previousCurve;
            else if (curve.Items.Count > 0) curve.SelectedIndex = 0;
            // A context owns both its nodes and calibration inputs.
            context.Enabled = selected?.Kind != "PricingContext";
            binding = false;
            UpdatePositionDefaults();
            OptionsChanged();
        }

        private bool Preview => trades && item?.Family == "Instruments";
        private bool BondPreview => Preview && item.Kind == "FixedRateBond";

        private void UpdatePositionDefaults()
        {
            if (binding || !Preview) return;
            var selectedContext = context.SelectedItem as BrowserObject;
            var key = item.Identity + "\0" + selectedContext?.Identity + "\0" + selectedContext?.EvaluationDate;
            if (key == positionDefaultsKey) return;
            binding = true;
            try
            {
                if (DateTime.TryParseExact(selectedContext?.EvaluationDate, "yyyy-MM-dd", CultureInfo.InvariantCulture, DateTimeStyles.None, out var date))
                    tradeDate.Value = date;
                positionDefaultsKey = key;
            }
            finally { binding = false; }
        }

        private void ClearResult()
        {
            generation++;
            response = null;
            table.Rows.Clear();
            table.Columns.Clear();
            parameters.Clear();
        }

        internal void CancelRequests() { generation++; }

        private void OptionsChanged()
        {
            if (binding) return;
            ClearResult();
            foreach (var control in positionControls) control.Visible = Preview;
            foreach (var control in bondControls) control.Visible = BondPreview;
            target.Enabled = targetLabel.Enabled = trades && view.SelectedIndex == 2;
            var supported = item != null && (trades ? BrowserDisplay.CanPrice(item) : curve.Items.Count > 0);
            var bondFairRate = trades && (item?.Kind == "FixedRateBondTrade" || item?.Kind == "FixedRateBond") && view.SelectedIndex == 2;
            var anchorError = "";
            if (BondPreview)
            {
                if (anchor.SelectedItem == null) anchorError = "Choose the bond's CleanPrice, DirtyPrice or ZSpread anchor and enter its value.";
                else try { BrowserDisplay.ReadNumber(anchorValue.Text); } catch (Exception error) { anchorError = error.Message; }
            }
            run.Enabled = supported && item.CurrentInputs && !bondFairRate && anchorError.Length == 0 && (!trades || (context.SelectedItem is BrowserObject && pricer.SelectedItem is BrowserObject));
            if (!supported) message.Text = trades ? "Select an instrument or trade to request PV, cashflows or a fair rate." : "Select a pricing context, curve, curve configuration or constructed elements store.";
            else if (!item.CurrentInputs) message.Text = "Inputs changed. Recalculate the workbook, then refresh before requesting results.";
            else if (bondFairRate) message.Text = "BondPricer does not define FairRate. Request YieldToMaturity or ZSpread through QS.EVALUATE for the bond's price or spread anchor.";
            else if (anchorError.Length > 0) message.Text = anchorError;
            else if (!run.Enabled) message.Text = "Create a pricing context and an explicit pricer in the workbook, then refresh.";
            else if (Preview) message.Text = "Instrument preview uses its notional and the trade date and side shown above. Choose a context, pricer and request, then click Run request.";
            else if (trades) message.Text = "Choose a context, pricer and request, then click Run request. Fair rates use a zero-based leg or loan segment.";
            else message.Text = "Choose a curve and view, then click Refresh table.";
        }

        private void RunQuery()
        {
            if (!run.Enabled || snapshot == null || item == null) return;
            ClearResult();
            BrowserQuery request;
            try { request = BuildQuery(); }
            catch (Exception error) { message.Text = error.Message; return; }
            var token = generation;
            var origin = item.DisplayName + (trades ? "; " + (context.SelectedItem as BrowserObject)?.DisplayName + "; " + (pricer.SelectedItem as BrowserObject)?.DisplayName : "") +
                (Preview ? "; preview " + request.TradeDate + ", " + request.Side : "");
            message.Text = "Running " + (trades ? (string)view.SelectedItem : request.Curve) + "…";
            ExcelAsyncUtil.QueueAsMacro(() =>
            {
                if (IsDisposed || token != generation) return;
                try
                {
                    dynamic app = ExcelDnaUtil.Application;
                    if (BrowserPanes.ActiveWindowId() != windowId() || !string.Equals((string)app.ActiveWorkbook.Name, request.Workbook, StringComparison.OrdinalIgnoreCase))
                        throw new InvalidOperationException("Activate the source workbook and refresh before running this request.");
                    if (!(bool)app.Ready) throw new InvalidOperationException("Wait until Excel is ready, then retry.");
                    // A queued request follows the same object to its newest revision.
                    // Dependency validation remains native and rejects genuinely stale inputs.
                    var latest = NativeBrowser.Read(request.Workbook);
                    var selected = BrowserDisplay.Resolve(latest, item);
                    if (selected == null) throw new InvalidOperationException("This object was removed. Refresh the browser and select its replacement.");
                    request.Handle = selected.Handle;
                    if (!string.IsNullOrEmpty(request.Context))
                        request.Context = BrowserDisplay.Resolve(latest, context.SelectedItem as BrowserObject)?.Handle ?? request.Context;
                    if (!string.IsNullOrEmpty(request.Pricer))
                        request.Pricer = BrowserDisplay.Resolve(latest, pricer.SelectedItem as BrowserObject)?.Handle ?? request.Pricer;
                    BrowserDisplay.DecimalSeparator = BrowserPanes.DecimalSeparator(app);
                    var result = NativeBrowser.Query(request);
                    if (IsDisposed || token != generation) return;
                    response = result;
                    foreach (var column in result.Columns) table.Columns.Add(new DataGridViewTextBoxColumn { HeaderText = column, SortMode = DataGridViewColumnSortMode.NotSortable });
                    foreach (var row in result.Rows) table.Rows.Add(row.Cast<object>().ToArray());
                    message.Text = origin + Environment.NewLine + result.Title + ": " + result.Rows.Count + " rows. " + result.Message;
                    ShowParameters();
                }
                catch (Exception error) { message.Text = error.Message; }
            });
        }

        private BrowserQuery BuildQuery()
        {
            return new BrowserQuery
            {
                Workbook = snapshot.Workbook, Handle = item.Handle,
                View = trades ? "evaluate" : view.SelectedIndex == 0 ? "nodes" : "instruments",
                Curve = curve.SelectedItem as string ?? "",
                Context = item.Kind == "PricingContext" ? "" : (context.SelectedItem as BrowserObject)?.Handle ?? "",
                Pricer = (pricer.SelectedItem as BrowserObject)?.Handle ?? "",
                Request = view.SelectedIndex == 0 ? "Value" : view.SelectedIndex == 1 ? "Cashflows" : "FairRate",
                Target = (uint)target.Value,
                TradeDate = Preview ? tradeDate.Value.ToString("yyyy-MM-dd", CultureInfo.InvariantCulture) : null,
                Side = Preview ? side.SelectedItem as string : null,
                PricingAnchor = BondPreview ? anchor.SelectedItem as string : null,
                AnchorValue = BondPreview ? (double?)BrowserDisplay.ReadNumber(anchorValue.Text) : null
            };
        }

        private void ShowParameters()
        {
            parameters.Clear();
            var row = table.CurrentRow?.Index ?? -1;
            if (response?.Parameters == null || row < 0 || row >= response.Parameters.Count) return;
            var text = new StringBuilder("Selected instrument / input parameters" + Environment.NewLine);
            foreach (var field in response.Parameters[row]) text.AppendLine(field.Label + ": " + BrowserDisplay.Value(field));
            parameters.Text = text.ToString();
        }

        internal void RefreshFormats() { table.Invalidate(); ShowParameters(); }
    }
}

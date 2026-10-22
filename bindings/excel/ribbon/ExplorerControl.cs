using System;
using System.Collections.Generic;
using System.Drawing;
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

        private static ComboBox Choice() => new ComboBox { DropDownStyle = ComboBoxStyle.DropDownList, Width = 210, DisplayMember = "DisplayName", DropDownWidth = 420 };

        internal ExplorerControl(bool trades, Func<long> windowId)
        {
            this.trades = trades;
            this.windowId = windowId;
            Dock = DockStyle.Fill;
            TableClipboard.Configure(table);
            var toolbar = new FlowLayoutPanel { Dock = DockStyle.Fill, AutoSize = true, Padding = new Padding(4), WrapContents = true };
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
            run.Text = trades ? "Run request" : "Refresh table";
            toolbar.Controls.Add(run);
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
            context.SelectedIndexChanged += (sender, args) => OptionsChanged();
            pricer.SelectedIndexChanged += (sender, args) => OptionsChanged();
            curve.SelectedIndexChanged += (sender, args) => OptionsChanged();
            view.SelectedIndexChanged += (sender, args) => OptionsChanged();
            target.ValueChanged += (sender, args) => OptionsChanged();
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
            OptionsChanged();
        }

        private void ClearResult()
        {
            generation++;
            response = null;
            table.Rows.Clear();
            table.Columns.Clear();
            parameters.Clear();
        }

        private void OptionsChanged()
        {
            if (binding) return;
            ClearResult();
            target.Enabled = targetLabel.Enabled = trades && view.SelectedIndex == 2;
            var supported = item != null && (trades ? item.Family == "Trades" : curve.Items.Count > 0);
            var bondFairRate = trades && item?.Kind == "FixedRateBondTrade" && view.SelectedIndex == 2;
            run.Enabled = supported && item.CurrentInputs && !bondFairRate && (!trades || (context.SelectedItem is BrowserObject && pricer.SelectedItem is BrowserObject));
            if (!supported) message.Text = trades ? "Select a trade to request PV, cashflows or a fair rate." : "Select a pricing context, curve, curve configuration or constructed elements store.";
            else if (!item.CurrentInputs) message.Text = "Inputs changed. Recalculate the workbook, then refresh before requesting results.";
            else if (bondFairRate) message.Text = "BondPricer does not define FairRate. Request YieldToMaturity or ZSpread through QS.EVALUATE for the bond's price or spread anchor.";
            else if (!run.Enabled) message.Text = "Create a pricing context and an explicit pricer in the workbook, then refresh.";
            else if (trades) message.Text = "Choose a context, pricer and request, then click Run request. Fair rates use a zero-based leg or loan segment.";
            else RunQuery();
        }

        private void RunQuery()
        {
            if (!run.Enabled || snapshot == null || item == null) return;
            ClearResult();
            var request = new BrowserQuery
            {
                Workbook = snapshot.Workbook, Handle = item.Handle,
                View = trades ? "evaluate" : view.SelectedIndex == 0 ? "nodes" : "instruments",
                Curve = curve.SelectedItem as string ?? "",
                Context = item.Kind == "PricingContext" ? "" : (context.SelectedItem as BrowserObject)?.Handle ?? "",
                Pricer = (pricer.SelectedItem as BrowserObject)?.Handle ?? "",
                Request = view.SelectedIndex == 0 ? "Value" : view.SelectedIndex == 1 ? "Cashflows" : "FairRate",
                Target = (uint)target.Value
            };
            var token = generation;
            var origin = item.DisplayName + (trades ? "; " + (context.SelectedItem as BrowserObject)?.DisplayName + "; " + (pricer.SelectedItem as BrowserObject)?.DisplayName : "");
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

        private void ShowParameters()
        {
            parameters.Clear();
            var row = table.CurrentRow?.Index ?? -1;
            if (response?.Parameters == null || row < 0 || row >= response.Parameters.Count) return;
            var text = new StringBuilder("Selected instrument / input parameters" + Environment.NewLine);
            foreach (var field in response.Parameters[row]) text.AppendLine(field.Label + ": " + field.Value);
            parameters.Text = text.ToString();
        }
    }
}

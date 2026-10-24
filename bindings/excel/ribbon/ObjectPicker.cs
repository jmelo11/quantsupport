using System;
using System.Collections.Generic;
using System.Drawing;
using System.Linq;
using System.Windows.Forms;

namespace QuantSupport.Ribbon
{
    // Keep the complete list in managed memory. The popup paints visible rows
    // on demand instead of inserting every object into a native ComboBox.
    internal sealed class ObjectPicker : UserControl
    {
        private readonly TextBox display = new TextBox { ReadOnly = true, Dock = DockStyle.Fill, TabStop = false, BackColor = SystemColors.Window };
        private readonly Button open = new Button { Text = "\u25be", Dock = DockStyle.Right, Width = 25, TabStop = false };
        private readonly bool showKind;
        private BrowserObject[] items = new BrowserObject[0];
        private int selectedIndex = -1;
        private ToolStripDropDown popup;

        internal ObjectPicker(bool showKind = false)
        {
            this.showKind = showKind;
            Height = display.PreferredHeight;
            MinimumSize = new Size(40, Height);
            Controls.Add(display);
            Controls.Add(open);
            display.Click += (s, e) => ShowChoices();
            open.Click += (s, e) => ShowChoices();
            display.KeyDown += HandleKeys;
            KeyDown += HandleKeys;
            GotFocus += (s, e) => display.Focus();
            display.TabStop = true;
            TabStop = false;
        }

        public event EventHandler SelectedIndexChanged;
        public IReadOnlyList<BrowserObject> Items => items;
        public BrowserObject SelectedItem
        {
            get => selectedIndex < 0 ? null : items[selectedIndex];
            set => SelectedIndex = value == null ? -1 : Array.IndexOf(items, value);
        }
        public int SelectedIndex
        {
            get => selectedIndex;
            set
            {
                if (value < -1 || value >= items.Length) throw new ArgumentOutOfRangeException(nameof(value));
                if (value == selectedIndex) return;
                selectedIndex = value;
                display.Text = value < 0 ? "" : Label(items[value]);
                SelectedIndexChanged?.Invoke(this, EventArgs.Empty);
            }
        }

        private string Label(BrowserObject item) => item == null ? "(Use selected object)" : showKind ? item.DisplayName : item.Name;

        internal void SetObjects(IEnumerable<BrowserObject> objects, bool optional = false)
        {
            CloseChoices();
            var previous = SelectedItem?.Identity;
            items = (optional ? new BrowserObject[] { null }.Concat(objects) : objects).ToArray();
            selectedIndex = -1;
            display.Clear();
            var match = previous == null ? -1 : Array.FindIndex(items, value => value?.Identity == previous);
            SelectedIndex = match >= 0 ? match : items.Length > 0 ? 0 : -1;
            Enabled = items.Length > 0;
        }

        private void HandleKeys(object sender, KeyEventArgs e)
        {
            if (e.KeyCode == Keys.F4 || e.KeyCode == Keys.Space || e.KeyCode == Keys.Down && e.Alt)
            {
                ShowChoices(); e.Handled = e.SuppressKeyPress = true;
            }
            else if (e.KeyCode == Keys.Down || e.KeyCode == Keys.Up)
            {
                SelectedIndex = Math.Max(-1, Math.Min(items.Length - 1, selectedIndex + (e.KeyCode == Keys.Down ? 1 : -1)));
                e.Handled = e.SuppressKeyPress = true;
            }
        }

        private void ShowChoices()
        {
            if (!Enabled || popup != null) return;
            var choices = items;
            var visible = Enumerable.Range(0, choices.Length).ToArray();
            var search = new TextBox { Dock = DockStyle.Top, AccessibleName = "Search objects by name, type or valuation date" };
            var heading = new Label { Text = "Search name, type or valuation date", Dock = DockStyle.Top, AutoSize = true };
            var grid = new DataGridView {
                Dock = DockStyle.Fill, ReadOnly = true, VirtualMode = true,
                MultiSelect = false, SelectionMode = DataGridViewSelectionMode.FullRowSelect,
                AllowUserToAddRows = false, AllowUserToDeleteRows = false,
                RowHeadersVisible = false, AutoSizeColumnsMode = DataGridViewAutoSizeColumnsMode.None,
                BackgroundColor = SystemColors.Window
            };
            grid.Columns.Add(new DataGridViewTextBoxColumn { HeaderText = "Name", AutoSizeMode = DataGridViewAutoSizeColumnMode.Fill, SortMode = DataGridViewColumnSortMode.NotSortable });
            grid.Columns.Add(new DataGridViewTextBoxColumn { HeaderText = "Type", Width = 135, SortMode = DataGridViewColumnSortMode.NotSortable });
            grid.Columns.Add(new DataGridViewTextBoxColumn { HeaderText = "Valuation date", Width = 110, SortMode = DataGridViewColumnSortMode.NotSortable });
            grid.CellValueNeeded += (s, e) => {
                if (e.RowIndex < 0 || e.RowIndex >= visible.Length) return;
                var item = choices[visible[e.RowIndex]];
                e.Value = e.ColumnIndex == 0 ? item?.Name ?? "(Use selected object)" : e.ColumnIndex == 1 ? item?.Kind : item?.EvaluationDate;
            };
            grid.RowCount = visible.Length;
            if (selectedIndex >= 0) grid.CurrentCell = grid.Rows[selectedIndex].Cells[0];
            search.TextChanged += (s, e) => {
                var query = search.Text.Trim();
                visible = Enumerable.Range(0, choices.Length).Where(index => {
                    var item = choices[index];
                    return query.Length == 0 || ((item?.Name ?? "") + " " + item?.Kind + " " + item?.EvaluationDate).IndexOf(query, StringComparison.OrdinalIgnoreCase) >= 0;
                }).ToArray();
                // RowCount shrinking removes rows individually. Clear in bulk
                // so a narrow search remains fast even in a very large list.
                grid.Rows.Clear();
                grid.RowCount = visible.Length;
                grid.Invalidate();
                if (visible.Length > 0) grid.CurrentCell = grid.Rows[0].Cells[0];
            };
            Action accept = () => {
                var row = grid.CurrentCell?.RowIndex ?? -1;
                if (row >= 0 && row < visible.Length) SelectedIndex = visible[row];
                CloseChoices();
                display.Focus();
            };
            grid.CellClick += (s, e) => { if (e.RowIndex >= 0) accept(); };
            KeyEventHandler navigate = (s, e) => {
                if (e.KeyCode == Keys.Enter) { accept(); e.Handled = e.SuppressKeyPress = true; }
                else if (e.KeyCode == Keys.Escape) { CloseChoices(); display.Focus(); e.Handled = e.SuppressKeyPress = true; }
                else if (s == search && (e.KeyCode == Keys.Down || e.KeyCode == Keys.Up) && visible.Length > 0) {
                    var row = Math.Max(0, Math.Min(visible.Length - 1, (grid.CurrentCell?.RowIndex ?? -1) + (e.KeyCode == Keys.Down ? 1 : -1)));
                    grid.CurrentCell = grid.Rows[row].Cells[0];
                    e.Handled = e.SuppressKeyPress = true;
                }
            };
            search.KeyDown += navigate;
            grid.KeyDown += navigate;
            var panel = new Panel { Size = new Size(Math.Max(500, Width), 360), Padding = new Padding(4) };
            panel.Controls.Add(grid);
            panel.Controls.Add(search);
            panel.Controls.Add(heading);
            var host = new ToolStripControlHost(panel) { AutoSize = false, Size = panel.Size, Margin = Padding.Empty, Padding = Padding.Empty };
            var menu = new ToolStripDropDown { Padding = Padding.Empty, AutoClose = true };
            menu.Items.Add(host);
            menu.Closed += (s, e) => {
                if (popup != menu) return; // CloseChoices disposes explicitly.
                popup = null;
                // Close still reparents its window after raising Closed.
                // An outside click needs deferred cleanup, including when the
                // owner window has already destroyed the popup's native handle.
                var cleanup = new Timer { Interval = 1 };
                cleanup.Tick += (sender, args) => { cleanup.Stop(); cleanup.Dispose(); menu.Dispose(); };
                cleanup.Start();
            };
            popup = menu;
            menu.Show(this, new Point(0, Height));
            search.Focus();
        }

        private void CloseChoices()
        {
            var menu = popup;
            popup = null;
            if (menu == null) return;
            menu.Close();
            menu.Dispose();
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing) { CloseChoices(); items = new BrowserObject[0]; selectedIndex = -1; }
            base.Dispose(disposing);
        }
    }
}

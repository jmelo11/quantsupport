using System;
using System.Drawing;
using System.Windows.Forms;

namespace QuantSupport.Ribbon
{
    internal static class TableClipboard
    {
        internal static void Configure(DataGridView table)
        {
            table.MultiSelect = true;
            table.BorderStyle = BorderStyle.None;
            table.CellBorderStyle = DataGridViewCellBorderStyle.SingleHorizontal;
            table.GridColor = Color.FromArgb(230, 234, 238);
            table.EnableHeadersVisualStyles = false;
            table.ColumnHeadersDefaultCellStyle.BackColor = Color.FromArgb(238, 243, 247);
            table.ColumnHeadersDefaultCellStyle.ForeColor = Color.FromArgb(35, 54, 70);
            table.ColumnHeadersDefaultCellStyle.Padding = new Padding(6, 5, 6, 5);
            table.DefaultCellStyle.Padding = new Padding(5, 2, 5, 2);
            table.AlternatingRowsDefaultCellStyle.BackColor = Color.FromArgb(248, 250, 252);
            table.RowTemplate.Height = 27;
            table.AutoSizeColumnsMode = DataGridViewAutoSizeColumnsMode.None;
            table.AutoSizeRowsMode = DataGridViewAutoSizeRowsMode.None;
            table.ColumnHeadersVisible = true;
            table.ColumnHeadersHeightSizeMode = DataGridViewColumnHeadersHeightSizeMode.AutoSize;
            table.ColumnHeadersDefaultCellStyle.WrapMode = DataGridViewTriState.True;
            table.ColumnAdded += (sender, args) => {
                args.Column.MinimumWidth = 95;
                args.Column.Width = 135;
                args.Column.Resizable = DataGridViewTriState.True;
                if (args.Column.Index == 0) { args.Column.AutoSizeMode = DataGridViewAutoSizeColumnMode.Fill; args.Column.MinimumWidth = 175; }
            };
            table.ClipboardCopyMode = DataGridViewClipboardCopyMode.EnableAlwaysIncludeHeaderText;
            Action copy = () => QuantSupportRibbon.Run(() =>
            {
                var content = table.GetClipboardContent();
                if (content != null) Clipboard.SetDataObject(content, true);
            });
            var menu = new ContextMenuStrip();
            menu.Items.Add("Select all (Ctrl+A)", null, (sender, args) => table.SelectAll());
            menu.Items.Add("Copy values (Ctrl+C)", null, (sender, args) => copy());
            menu.Items.Add("Copy entire table", null, (sender, args) => { table.SelectAll(); copy(); });
            table.ContextMenuStrip = menu;
            table.Disposed += (sender, args) => menu.Dispose();
            table.KeyDown += (sender, args) =>
            {
                if (!args.Control) return;
                if (args.KeyCode == Keys.A) { table.SelectAll(); args.Handled = args.SuppressKeyPress = true; }
                if (args.KeyCode == Keys.C) { copy(); args.Handled = args.SuppressKeyPress = true; }
            };
        }
    }
}

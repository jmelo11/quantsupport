using System;
using System.Windows.Forms;

namespace QuantSupport.Ribbon
{
    internal static class TableClipboard
    {
        internal static void Configure(DataGridView table)
        {
            table.MultiSelect = true;
            table.ColumnHeadersVisible = true;
            table.ColumnHeadersHeightSizeMode = DataGridViewColumnHeadersHeightSizeMode.AutoSize;
            table.ColumnHeadersDefaultCellStyle.WrapMode = DataGridViewTriState.True;
            table.ColumnAdded += (sender, args) => args.Column.MinimumWidth = 85;
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

using System;
using System.Globalization;
using System.Linq;
using System.Text;

namespace QuantSupport.Ribbon
{
    // Native values remain invariant; only explicitly numeric fields are localized.
    internal static class BrowserDisplay
    {
        internal static string DecimalSeparator { get; set; } = ".";

        internal static string Number(string value) => (value ?? "").Replace(".", DecimalSeparator);
        internal static string Value(Detail field) => field.Numeric ? Number(field.Value) : field.Value;

        internal static bool CanPrice(BrowserObject item) => item?.Family == "Trades" || item?.Family == "Instruments";

        internal static double ReadNumber(string value)
        {
            var format = new NumberFormatInfo { NumberDecimalSeparator = DecimalSeparator };
            if (!double.TryParse(value, NumberStyles.Float, format, out var number) || double.IsNaN(number) || double.IsInfinity(number))
                throw new InvalidOperationException("Enter a finite anchor value using Excel's decimal separator (" + DecimalSeparator + ").");
            return number;
        }

        internal static BrowserObject Resolve(Snapshot snapshot, BrowserObject previous)
        {
            return previous == null ? null : snapshot?.Objects?.FirstOrDefault(item => item.Identity == previous.Identity);
        }

        internal static string Describe(BrowserObject item)
        {
            if (item == null) return "";
            var text = new StringBuilder();
            text.AppendLine(item.Name + " (" + item.Kind + ")");
            text.AppendLine(item.Handle);
            text.AppendLine("Inputs: " + (item.CurrentInputs ? "Current" : "Changed; recalculate to rebuild this object"));
            if (!string.IsNullOrEmpty(item.EvaluationDate)) text.AppendLine("Valuation date: " + item.EvaluationDate);
            if (!string.IsNullOrEmpty(item.SnapshotTimestamp)) text.AppendLine("Snapshot (UTC): " + item.SnapshotTimestamp);
            if (!string.IsNullOrEmpty(item.QuoteLevel)) text.AppendLine("Quote side: " + item.QuoteLevel);
            foreach (var field in item.Details) text.AppendLine(field.Label + ": " + Value(field));
            text.AppendLine();
            text.AppendLine("Defining / updating cells:");
            foreach (var cell in item.Locations)
            {
                text.AppendLine(cell.Address);
                if (!string.IsNullOrEmpty(cell.Formula)) text.AppendLine(cell.Formula);
            }
            if (item.Locations.Count == 0) text.AppendLine("No defining cell remains; another live formula retains this object.");
            if (item.Dependencies.Count > 0)
            {
                text.AppendLine();
                text.AppendLine("Inputs:");
                foreach (var dependency in item.Dependencies) text.AppendLine(dependency);
            }
            return text.ToString();
        }
    }
}

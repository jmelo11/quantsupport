using System;
using System.Collections.Generic;
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

        internal static bool IsPricingContext(BrowserObject item) => item?.Kind == "PricingContext" && item.Curves?.Count > 0;

        // Use an existing worksheet valuation when it identifies one unambiguous setup.
        internal static BrowserObject[] PricingDefaults(Snapshot snapshot, BrowserObject selected)
        {
            if (snapshot?.Objects == null || selected == null) return new BrowserObject[0];
            var objects = snapshot.Objects.Where(value => value.Handle != null).ToDictionary(value => value.Handle);
            var setups = new List<BrowserObject[]>();
            foreach (var result in snapshot.Objects.Where(value => value.Kind == "EvaluationResults" && value.CurrentInputs))
            {
                var dependencies = new HashSet<string>();
                var pending = new Stack<string>(result.Dependencies ?? new List<string>());
                while (pending.Count > 0)
                {
                    var handle = pending.Pop();
                    if (!dependencies.Add(handle) || !objects.TryGetValue(handle, out var dependency)) continue;
                    foreach (var input in dependency.Dependencies ?? new List<string>()) pending.Push(input);
                }
                if (!dependencies.Contains(selected.Handle)) continue;
                var contexts = snapshot.Objects.Where(value => IsPricingContext(value) && value.CurrentInputs && dependencies.Contains(value.Handle)).ToArray();
                var pricers = snapshot.Objects.Where(value => value.Kind == "Pricer" && value.CurrentInputs && dependencies.Contains(value.Handle)).ToArray();
                if (contexts.Length == 1 && pricers.Length == 1) setups.Add(new[] { contexts[0], pricers[0] });
            }
            var distinct = setups.GroupBy(pair => pair[0].Identity + "\0" + pair[1].Identity).ToArray();
            return distinct.Length == 1 ? distinct[0].First() : new BrowserObject[0];
        }

        internal static string PricingRequest(string choice)
        {
            switch (choice)
            {
                case "PV": return "Value";
                case "Cashflows": return "Cashflows";
                case "Fair rate": return "FairRate";
                case "Sensitivities": return "Sensitivities";
                default: throw new InvalidOperationException("Choose a pricing request.");
            }
        }

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
            text.AppendLine("Inputs: " + (item.CurrentInputs ? "Current" : "Changed; recalculate to rebuild this object"));
            if (!string.IsNullOrEmpty(item.EvaluationDate)) text.AppendLine("Valuation date: " + item.EvaluationDate);
            if (!string.IsNullOrEmpty(item.SnapshotTimestamp)) text.AppendLine("Snapshot (UTC): " + item.SnapshotTimestamp);
            if (!string.IsNullOrEmpty(item.QuoteLevel)) text.AppendLine("Quote side: " + item.QuoteLevel);
            foreach (var field in item.Details) text.AppendLine(field.Label + ": " + Value(field));
            return text.ToString();
        }
    }
}

using System;
using System.IO;
using System.Runtime.Serialization.Json;
using QuantSupport.Ribbon;

internal static class Program
{
    private static void Check(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }

    private static void Main()
    {
        using (var input = File.OpenRead(Path.Combine(AppContext.BaseDirectory, "snapshot.json")))
        {
            var snapshot = (Snapshot)new DataContractJsonSerializer(typeof(Snapshot)).ReadObject(input);
            Check(snapshot.Workbook == "Café.xlsx" && snapshot.Error == "", "Unicode workbook contract");
            Check(snapshot.Objects.Count == 1, "Object array contract");
            var context = snapshot.Objects[0];
            Check(context.Name == "café \"market\"" && context.Handle.EndsWith(":1"), "Escaped name and handle contract");
            Check(context.CurrentInputs && context.Revision == 1, "Boolean and revision contract");
            Check(context.EvaluationDate == "2026-01-15" && context.SnapshotTimestamp == "2026-01-16T00:30:00+00:00" && context.QuoteLevel == "Ask", "Market date contract");
            Check(context.Details.Count == 6 && context.Dependencies.Count == 0, "Detail and dependency arrays");
            Check(context.Curves.Count == 0, "Curve selector array");
            var source = context.Locations[0];
            Check(source.SheetId == 42 && source.Address == "Marché!A1", "Source coordinates contract");
            Check(source.Formula.Contains("\"\"market\"\""), "Formula quoting contract");
            var previous = new BrowserObject { Kind = context.Kind, Name = context.Name, Handle = context.Handle };
            context.Handle = context.Handle.Substring(0, context.Handle.LastIndexOf(':')) + ":2";
            Check(BrowserDisplay.Resolve(snapshot, previous) == context, "Curve selection follows a rebuilt revision");
            Check(BrowserDisplay.Resolve(snapshot, new BrowserObject { Name = context.Name, Kind = "QuoteStore" }) == null, "Same name in a different family must not resolve");
        }
        Check(new Location { Sheet = "Sheet1", Column = 16384, Row = 1048576 }.Address == "Sheet1!XFD1048576", "Excel edge coordinates");
        using (var input = File.OpenRead(Path.Combine(AppContext.BaseDirectory, "query-response.json")))
        {
            var response = (BrowserResponse)new DataContractJsonSerializer(typeof(BrowserResponse)).ReadObject(input);
            Check(response.Error == "" && response.Columns.Count == 20 && response.Rows[0].Count == 20, "Query response contract");
            Check(response.Rows[0][1] == "Coupon café" && response.Rows[0][2] == "-1234.5" && response.Rows[0][4] == "", "Unicode, amounts and blank cells");
            Check(response.Curves.Count == 0 && response.Parameters.Count == 0, "Query metadata arrays");
            Check(response.NumericColumns.Contains(2) && !response.NumericColumns.Contains(0), "Numeric metadata must distinguish dates from amounts");
        }
        using (var input = File.OpenRead(Path.Combine(AppContext.BaseDirectory, "query.json")))
        {
            var serializer = new DataContractJsonSerializer(typeof(BrowserQuery));
            var query = (BrowserQuery)serializer.ReadObject(input);
            Check(query.Workbook == "Café.xlsx" && query.Request == "FairRate" && query.Target == 1, "Query request contract");
            using (var output = new MemoryStream())
            {
                serializer.WriteObject(output, query);
                output.Position = 0;
                var copy = (BrowserQuery)serializer.ReadObject(output);
                Check(copy.Context == query.Context && copy.Target == 1 && copy.Handle == query.Handle, "Query serialization round trip");
            }
        }
        BrowserDisplay.DecimalSeparator = ",";
        Check(BrowserDisplay.Number("-1234.5678901234567") == "-1234,5678901234567", "Excel comma decimals preserve precision");
        Check(BrowserDisplay.Number("1.25e-10") == "1,25e-10", "Scientific notation uses the Excel decimal separator");
        Check(BrowserDisplay.Value(new Detail { Value = "Swap_EUR_EURIBOR3m_1Y_0.04", Numeric = false }) == "Swap_EUR_EURIBOR3m_1Y_0.04", "Quote identifiers must remain unchanged");
        Check(BrowserDisplay.Value(new Detail { Value = "2026-10-22", Numeric = false }) == "2026-10-22", "Dates must remain unchanged");
        Check(BrowserDisplay.Value(new Detail { Value = "0.035", Numeric = true }) == "0,035", "Numeric parameter display follows Excel");
        BrowserDisplay.DecimalSeparator = ".";
        Check(BrowserDisplay.Number("0.035") == "0.035", "Dot-decimal Excel settings remain supported");
        Console.WriteLine("Browser contracts, revision replacement, numeric localization and source coordinates passed.");
    }
}

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
        Check(BrowserDisplay.CanPrice(new BrowserObject { Family = "Instruments", Kind = "Swap" }), "Instrument selection must expose pricing");
        Check(BrowserDisplay.CanPrice(new BrowserObject { Family = "Trades", Kind = "SwapTrade" }), "Trade selection must expose pricing");
        Check(!BrowserDisplay.CanPrice(new BrowserObject { Family = "Pricing contexts" }) && !BrowserDisplay.CanPrice(null), "Unrelated selections must hide pricing");
        Check(BrowserDisplay.PricingRequest("Sensitivities") == "Sensitivities", "Sensitivities must reach the native request unchanged");
        Check(BrowserDisplay.PricingRequest("Fair rate") == "FairRate" && BrowserDisplay.PricingRequest("PV") == "Value" && BrowserDisplay.PricingRequest("Cashflows") == "Cashflows", "Existing pricing request mappings");
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
                var json = System.Text.Encoding.UTF8.GetString(output.ToArray());
                Check(!json.Contains("\"trade_date\"") && !json.Contains("\"side\"") && !json.Contains("\"anchor_value\""), "Stored trade requests must omit preview fields on the wire");
                output.Position = 0;
                var copy = (BrowserQuery)serializer.ReadObject(output);
                Check(copy.Context == query.Context && copy.Target == 1 && copy.Handle == query.Handle, "Query serialization round trip");
                Check(copy.TradeDate == null && copy.Side == null && copy.AnchorValue == null, "Stored trade requests must not override position terms");
            }
        }
        using (var input = File.OpenRead(Path.Combine(AppContext.BaseDirectory, "query-instrument.json")))
        {
            var serializer = new DataContractJsonSerializer(typeof(BrowserQuery));
            var query = (BrowserQuery)serializer.ReadObject(input);
            Check(query.TradeDate == "2026-01-15" && query.Side == "Receive", "Instrument preview position inputs");
            query.PricingAnchor = "CleanPrice";
            query.AnchorValue = 99.25;
            using (var output = new MemoryStream())
            {
                serializer.WriteObject(output, query);
                using (var json = System.Text.Json.JsonDocument.Parse(output.ToArray()))
                    Check(json.RootElement.GetProperty("anchor_value").GetDouble() == 99.25, "Native query requires an ordinary JSON numeric anchor");
                output.Position = 0;
                var copy = (BrowserQuery)serializer.ReadObject(output);
                Check(copy.AnchorValue == 99.25 && copy.PricingAnchor == "CleanPrice", "Bond anchor remains numeric across the ABI");
            }
        }
        BrowserDisplay.DecimalSeparator = ",";
        using (var input = File.OpenRead(Path.Combine(AppContext.BaseDirectory, "sensitivities-response.json")))
        {
            var response = (BrowserResponse)new DataContractJsonSerializer(typeof(BrowserResponse)).ReadObject(input);
            Check(response.Title == "Sensitivities" && response.Rows.Count == 2, "Sensitivity response contract");
            Check(response.NumericColumns.Count == 1 && response.NumericColumns[0] == 1, "Only sensitivity values are numeric");
            Check(response.Rows[1][0] == "FX:USD/EUR.café" && BrowserDisplay.Number(response.Rows[0][1]) == "-1234,5", "Localized sensitivity values preserve risk-factor labels");
            Check(response.Rows[1][1] == "0.00001", "Raw sensitivity values must not be scaled");
        }
        Check(BrowserDisplay.Number("-1234.5678901234567") == "-1234,5678901234567", "Excel comma decimals preserve precision");
        Check(BrowserDisplay.Number("1.25e-10") == "1,25e-10", "Scientific notation uses the Excel decimal separator");
        Check(BrowserDisplay.ReadNumber("99,25") == 99.25, "Bond anchors accept Excel comma decimals");
        foreach (var invalid in new[] { "NaN", "Infinity", "99.25", "" })
        {
            var rejected = false;
            try { BrowserDisplay.ReadNumber(invalid); } catch (InvalidOperationException) { rejected = true; }
            Check(rejected, "Reject invalid or ambiguous anchor values: " + invalid);
        }
        Check(BrowserDisplay.Value(new Detail { Value = "Swap_EUR_EURIBOR3m_1Y_0.04", Numeric = false }) == "Swap_EUR_EURIBOR3m_1Y_0.04", "Quote identifiers must remain unchanged");
        Check(BrowserDisplay.Value(new Detail { Value = "2026-10-22", Numeric = false }) == "2026-10-22", "Dates must remain unchanged");
        Check(BrowserDisplay.Value(new Detail { Value = "0.035", Numeric = true }) == "0,035", "Numeric parameter display follows Excel");
        BrowserDisplay.DecimalSeparator = ".";
        Check(BrowserDisplay.Number("0.035") == "0.035", "Dot-decimal Excel settings remain supported");
        Check(BrowserDisplay.ReadNumber("99.25") == 99.25, "Bond anchors accept Excel dot decimals");
        Console.WriteLine("Browser contracts, revision replacement, numeric localization and source coordinates passed.");
    }
}

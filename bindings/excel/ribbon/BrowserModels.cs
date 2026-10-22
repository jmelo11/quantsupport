using System.Collections.Generic;
using System.Runtime.Serialization;

namespace QuantSupport.Ribbon
{
    [DataContract]
    internal sealed class Snapshot
    {
        [DataMember(Name = "workbook")] public string Workbook { get; set; }
        [DataMember(Name = "objects")] public List<BrowserObject> Objects { get; set; }
        [DataMember(Name = "error")] public string Error { get; set; }
    }

    [DataContract]
    internal sealed class BrowserObject
    {
        [DataMember(Name = "name")] public string Name { get; set; }
        [DataMember(Name = "handle")] public string Handle { get; set; }
        [DataMember(Name = "kind")] public string Kind { get; set; }
        [DataMember(Name = "family")] public string Family { get; set; }
        [DataMember(Name = "revision")] public ulong Revision { get; set; }
        [DataMember(Name = "current_inputs")] public bool CurrentInputs { get; set; }
        [DataMember(Name = "evaluation_date")] public string EvaluationDate { get; set; }
        [DataMember(Name = "snapshot_timestamp")] public string SnapshotTimestamp { get; set; }
        [DataMember(Name = "quote_level")] public string QuoteLevel { get; set; }
        [DataMember(Name = "details")] public List<Detail> Details { get; set; }
        [DataMember(Name = "locations")] public List<Location> Locations { get; set; }
        [DataMember(Name = "dependencies")] public List<string> Dependencies { get; set; }
        [DataMember(Name = "curves")] public List<string> Curves { get; set; }

        public string DisplayName => Name + " (" + Kind + ")";
    }

    [DataContract]
    internal sealed class Location
    {
        [DataMember(Name = "sheet_id")] public ulong SheetId { get; set; }
        [DataMember(Name = "sheet")] public string Sheet { get; set; }
        [DataMember(Name = "row")] public int Row { get; set; }
        [DataMember(Name = "column")] public int Column { get; set; }
        [DataMember(Name = "formula")] public string Formula { get; set; }

        public string Address
        {
            get
            {
                var column = Column;
                var name = "";
                while (column > 0) { column--; name = (char)('A' + column % 26) + name; column /= 26; }
                return Sheet + "!" + name + Row;
            }
        }
    }

    [DataContract]
    internal sealed class Detail
    {
        [DataMember(Name = "label")] public string Label { get; set; }
        [DataMember(Name = "value")] public string Value { get; set; }
    }

    [DataContract]
    internal sealed class BrowserQuery
    {
        [DataMember(Name = "workbook")] public string Workbook { get; set; } = "";
        [DataMember(Name = "handle")] public string Handle { get; set; } = "";
        [DataMember(Name = "view")] public string View { get; set; } = "";
        [DataMember(Name = "curve")] public string Curve { get; set; } = "";
        [DataMember(Name = "context")] public string Context { get; set; } = "";
        [DataMember(Name = "pricer")] public string Pricer { get; set; } = "";
        [DataMember(Name = "request")] public string Request { get; set; } = "";
        [DataMember(Name = "target")] public uint Target { get; set; }
    }

    [DataContract]
    internal sealed class BrowserResponse
    {
        [DataMember(Name = "title")] public string Title { get; set; }
        [DataMember(Name = "message")] public string Message { get; set; }
        [DataMember(Name = "curves")] public List<string> Curves { get; set; }
        [DataMember(Name = "columns")] public List<string> Columns { get; set; }
        [DataMember(Name = "rows")] public List<List<string>> Rows { get; set; }
        [DataMember(Name = "parameters")] public List<List<Detail>> Parameters { get; set; }
        [DataMember(Name = "error")] public string Error { get; set; }
    }
}

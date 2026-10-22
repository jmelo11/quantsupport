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
}

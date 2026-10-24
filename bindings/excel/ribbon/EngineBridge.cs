using System;
using System.Collections.Generic;
using System.Collections.Concurrent;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Runtime.Serialization;
using System.Runtime.Serialization.Json;
using System.Text;
using System.Threading;
using ExcelDna.Integration;
using Microsoft.Win32.SafeHandles;

namespace QuantSupport.Ribbon
{
    [DataContract]
    internal sealed class EnginePulse
    {
        [DataMember(Name = "changed")] public List<string> Changed { get; set; }
        [DataMember(Name = "pending")] public int Pending { get; set; }
        [DataMember(Name = "pending_delivery")] public int PendingDelivery { get; set; }
        [DataMember(Name = "settled")] public bool Settled { get; set; }
        [DataMember(Name = "submitted")] public long Submitted { get; set; }
        [DataMember(Name = "completed")] public long Completed { get; set; }
        [DataMember(Name = "error")] public string Error { get; set; }
    }
    [DataContract]
    internal sealed class DeliveryTarget
    {
        [DataMember(Name = "sheet")] public ulong Sheet { get; set; }
        [DataMember(Name = "row")] public int Row { get; set; }
        [DataMember(Name = "column")] public int Column { get; set; }
    }

    // One signal and one bounded dispatcher for every worksheet and pane.
    public static class EngineBridge
    {
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] private static extern IntPtr GetModuleHandle(string name);
        [DllImport("kernel32.dll", CharSet = CharSet.Ansi)] private static extern IntPtr GetProcAddress(IntPtr module, string name);
        [UnmanagedFunctionPointer(CallingConvention.StdCall)] private delegate uint Version();
        [UnmanagedFunctionPointer(CallingConvention.StdCall)] private delegate UIntPtr Event();
        [UnmanagedFunctionPointer(CallingConvention.StdCall)] private delegate IntPtr Read();
        [UnmanagedFunctionPointer(CallingConvention.StdCall)] private delegate void Free(IntPtr value);
        [UnmanagedFunctionPointer(CallingConvention.StdCall)] private delegate void Timing(double milliseconds);
        [UnmanagedFunctionPointer(CallingConvention.StdCall)] private delegate int Attach(uint version);
        [UnmanagedFunctionPointer(CallingConvention.StdCall, CharSet = CharSet.Unicode)] private delegate void ReleaseTopic([MarshalAs(UnmanagedType.LPWStr)] string topic, UIntPtr length);
        private static Read pulse, metrics, targets;
        private static Free free;
        private static Timing recordDelivery;
        private static ReleaseTopic unsubscribe;
        private static Attach attach;
        private static readonly ConcurrentQueue<string> disconnected = new ConcurrentQueue<string>();
        private static readonly ConcurrentQueue<Action> pages = new ConcurrentQueue<Action>();
        private static EventWaitHandle signal;
        private static RegisteredWaitHandle wait;
        private static System.Windows.Forms.Timer retry;
        private static int queued;
        private static bool stopped;
        private static bool refreshRequested;
        private static string dispatchError = "";
        private static DateTime nextDelivery;
        private static readonly Dictionary<string, Topic> topics = new Dictionary<string, Topic>();

        private static T Export<T>(IntPtr module, string name) where T : class
        {
            var address = GetProcAddress(module, name);
            if (address == IntPtr.Zero) throw new InvalidOperationException("Load matching QuantSupport native and ribbon XLLs from the same release.");
            return (T)(object)Marshal.GetDelegateForFunctionPointer(address, typeof(T));
        }
        internal static void Start()
        {
            stopped = false;
            retry = new System.Windows.Forms.Timer { Interval = 16 };
            retry.Tick += (sender, args) => {
                retry.Stop();
                if (pulse == null) { Connect(); if (pulse == null) retry.Start(); }
                else ExcelAsyncUtil.QueueAsMacro(Dispatch);
            };
            Connect();
            if (pulse == null) retry.Start();
        }
        private static void Connect()
        {
            var module = GetModuleHandle(Path.Combine(Path.GetDirectoryName(ExcelDnaUtil.XllPath), "quantsupport.xll"));
            if (module == IntPtr.Zero) return;
            if (Export<Version>(module, "qs_engine_api_version")() != 1) throw new InvalidOperationException("QuantSupport engine ABI mismatch.");
            pulse = Export<Read>(module, "qs_engine_pulse"); metrics = Export<Read>(module, "qs_engine_metrics"); free = Export<Free>(module, "qs_engine_free");
            targets = Export<Read>(module, "qs_engine_refresh_targets");
            recordDelivery = Export<Timing>(module, "qs_engine_record_delivery");
            unsubscribe = Export<ReleaseTopic>(module, "qs_engine_unsubscribe"); attach = Export<Attach>(module, "qs_engine_attach");
            if (attach(1) != 1) throw new InvalidOperationException("Connect the pricing bridge on Excel's main thread.");
            var handle = Export<Event>(module, "qs_engine_event")();
            if (handle == UIntPtr.Zero) { pulse = null; return; }
            signal = new EventWaitHandle(false, EventResetMode.AutoReset);
            signal.SafeWaitHandle = new SafeWaitHandle((IntPtr)handle.ToUInt64(), false);
            wait = ThreadPool.RegisterWaitForSingleObject(signal, (state, timeout) => Schedule(), null, Timeout.Infinite, false);
            retry.Stop(); Schedule();
        }
        private static void Schedule()
        {
            if (stopped || Interlocked.Exchange(ref queued, 1) != 0) return;
            // QueueAsMacro drains its queued actions together. Repeated native
            // completions must yield through a UI timer between dispatches,
            // otherwise a large batch can monopolize that macro queue.
            ExcelAsyncUtil.QueueAsMacro(() => { if (!stopped) retry.Start(); });
        }
        internal static void QueuePage(Action page) { pages.Enqueue(page); Schedule(); }
        private static string ReadJson(Read read)
        {
            var buffer = read();
            try {
                var length = 0; while (Marshal.ReadByte(buffer, length) != 0) length++;
                var bytes = new byte[length]; Marshal.Copy(buffer, bytes, 0, length);
                return Encoding.UTF8.GetString(bytes);
            } finally { free(buffer); }
        }
        private static void Dispatch()
        {
            try { DispatchCore(); }
            catch (Exception error) {
                dispatchError = error.ToString();
                retry.Stop();
            }
        }
        [ExcelFunction(Name = "QS._ENGINE.DIAGNOSTIC", IsHidden = true, IsExceptionSafe = true)]
        public static string Diagnostic() { return dispatchError; }
        private static void DispatchCore()
        {
            Interlocked.Exchange(ref queued, 0);
            if (stopped || pulse == null) return;
            dynamic app = ExcelDnaUtil.Application;
            if (!(bool)app.Ready || (int)app.CalculationState == 1) { retry.Start(); return; }
            retry.Stop();
            for (var i = 0; i < 256 && disconnected.TryDequeue(out var key); i++)
                if (!topics.ContainsKey(key)) unsubscribe(key, new UIntPtr((uint)key.Length));
            EnginePulse result;
            using (var stream = new MemoryStream(Encoding.UTF8.GetBytes(ReadJson(pulse))))
                result = (EnginePulse)new DataContractJsonSerializer(typeof(EnginePulse)).ReadObject(stream);
            if (!string.IsNullOrEmpty(result.Error)) { dispatchError = result.Error; return; }
            foreach (var key in result.Changed) if (topics.TryGetValue(key, out var topic)) topic.Update();
            Completed?.Invoke();
            if (pages.TryDequeue(out var page)) page();
            if (!pages.IsEmpty) Schedule();
            if (!disconnected.IsEmpty) Schedule();
            // Delivery belongs to an already requested calculation, including
            // dependent stages in Manual mode. A late completion must not lose
            // its refresh merely because a previous dispatcher was idle.
            if (refreshRequested || result.Pending > 0 || result.PendingDelivery > 0) {
                if (DateTime.UtcNow < nextDelivery) { retry.Start(); return; }
                List<DeliveryTarget> cells;
                var delivery = Stopwatch.StartNew();
                using (var stream = new MemoryStream(Encoding.UTF8.GetBytes(ReadJson(targets))))
                    cells = (List<DeliveryTarget>)new DataContractJsonSerializer(typeof(List<DeliveryTarget>)).ReadObject(stream);
                foreach (var group in cells.GroupBy(c => c.Sheet)) {
                    var first = group.First();
                    var reference = new ExcelReference(first.Row, first.Row, first.Column, first.Column, (IntPtr)(long)first.Sheet);
                    var qualified = ((string)XlCall.Excel(XlCall.xlSheetNm, reference)).Trim('\'').Replace("''", "'");
                    var open = qualified.LastIndexOf('['); var close = qualified.LastIndexOf(']');
                    dynamic sheet = app.Workbooks[qualified.Substring(open + 1, close - open - 1)].Worksheets[qualified.Substring(close + 1)];
                    foreach (var address in DeliveryRanges(group)) {
                        dynamic range = sheet.Range[address];
                        range.Dirty(); range.Calculate();
                    }
                }
                nextDelivery = DateTime.UtcNow.AddMilliseconds(16);
                if (cells.Count > 0) recordDelivery(delivery.Elapsed.TotalMilliseconds);
                if (result.Settled && result.Pending == 0 && result.PendingDelivery == 0 && cells.Count == 0) refreshRequested = false;
                else if (cells.Count > 0) retry.Start();
            }
        }
        private static IEnumerable<string> DeliveryRanges(IEnumerable<DeliveryTarget> cells)
        {
            var batch = "";
            foreach (var group in cells.GroupBy(c => c.Column).OrderBy(g => g.Key)) {
                var rows = group.Select(c => c.Row).Distinct().OrderBy(r => r).ToArray();
                for (var i = 0; i < rows.Length; i++) {
                    var first = rows[i]; var last = first;
                    while (i + 1 < rows.Length && rows[i + 1] == last + 1) last = rows[++i];
                    var address = Column(group.Key) + (first + 1);
                    if (last != first) address += ":" + Column(group.Key) + (last + 1);
                    // Excel rejects long comma-separated Range addresses.
                    if (batch.Length + address.Length + 1 > 220) { yield return batch; batch = ""; }
                    batch += (batch.Length == 0 ? "" : ",") + address;
                }
            }
            if (batch.Length > 0) yield return batch;
        }
        private static string Column(int column)
        {
            var result = "";
            for (var value = column + 1; value > 0; value = (value - 1) / 26) result = (char)('A' + (value - 1) % 26) + result;
            return result;
        }
        [ExcelCommand(Name = "QS._ENGINE.REFRESH")]
        public static void RefreshCommand() { Refresh(); }
        [ExcelCommand(Name = "QS._ENGINE.DELIVER")]
        public static void DeliverCommand()
        {
            ExcelAsyncUtil.QueueAsMacro(() => { dispatchError = ""; refreshRequested = true; Schedule(); });
        }
        internal static void Refresh()
        {
            ExcelAsyncUtil.QueueAsMacro(() => {
                dispatchError = ""; refreshRequested = true;
                dynamic app = ExcelDnaUtil.Application;
                app.CalculateFull();
                Schedule();
            });
        }
        internal static event Action Completed;
        internal static string Metrics() => pulse == null ? "{\"error\":\"engine bridge unavailable\"}" : ReadJson(metrics);

        [ExcelFunction(Name = "QS._ENGINE.SUBSCRIBE", IsHidden = true, IsExceptionSafe = true)]
        public static object Subscribe(string key)
        {
            var token = ExcelAsyncUtil.Observe("QS.Engine", key, () => {
                var topic = new Topic(key); topics[key] = topic; return topic;
            });
            // The enclosing native UDF owns pending/error presentation. A new
            // RTD topic initially returns #N/A. A negative token distinguishes
            // that state from an initialized subscription for delivery tracking.
            return token is double || token is int ? token : -1.0;
        }
        private sealed class Topic : IExcelObservable, IDisposable
        {
            private readonly string key;
            private IExcelObserver observer;
            private long revision;
            public Topic(string key) { this.key = key; }
            public IDisposable Subscribe(IExcelObserver value) { observer = value; value.OnNext(0.0); return this; }
            public void Update() { observer?.OnNext((double)++revision); }
            public void Dispose()
            {
                observer = null;
                if (topics.TryGetValue(key, out var current) && ReferenceEquals(current, this)) {
                    topics.Remove(key); disconnected.Enqueue(key); Schedule();
                }
            }
        }
        internal static void Stop()
        {
            stopped = true; attach?.Invoke(0); retry?.Stop(); retry?.Dispose(); wait?.Unregister(null); signal?.Dispose();
            topics.Clear(); pulse = null;
        }
    }
}

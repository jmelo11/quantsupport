using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Runtime.Serialization.Json;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using System.Runtime.Serialization;
using ExcelDna.Integration;

namespace QuantSupport.Ribbon
{
    internal static class NativeBrowser
    {
        private static string cachedWorkbook;
        private static ulong cachedRevision;
        private static Task<Snapshot> cachedSnapshot;
        [DllImport("msvcrt.dll", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
        private static extern UIntPtr strlen(IntPtr text);
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        private static extern IntPtr GetModuleHandle(string name);
        [DllImport("kernel32.dll", CharSet = CharSet.Ansi, ExactSpelling = true, SetLastError = true)]
        private static extern IntPtr GetProcAddress(IntPtr module, string name);
        [UnmanagedFunctionPointer(CallingConvention.StdCall)]
        private delegate uint ApiVersion();
        [UnmanagedFunctionPointer(CallingConvention.StdCall, CharSet = CharSet.Unicode)]
        private delegate IntPtr GetSnapshot([MarshalAs(UnmanagedType.LPWStr)] string workbook, UIntPtr length);
        [UnmanagedFunctionPointer(CallingConvention.StdCall)]
        private delegate void FreeSnapshot(IntPtr value);
        [UnmanagedFunctionPointer(CallingConvention.StdCall, CharSet = CharSet.Unicode)]
        private delegate IntPtr GetQuery([MarshalAs(UnmanagedType.LPWStr)] string request, UIntPtr length);
        [UnmanagedFunctionPointer(CallingConvention.StdCall)]
        private delegate ulong GetRevision();
        [UnmanagedFunctionPointer(CallingConvention.StdCall)]
        private delegate IntPtr ResetMemory();
        [UnmanagedFunctionPointer(CallingConvention.StdCall)] private delegate IntPtr TakePage(ulong job, UIntPtr offset, UIntPtr limit);
        [UnmanagedFunctionPointer(CallingConvention.StdCall)] private delegate void CancelJob(ulong job);

        [DataContract]
        private sealed class Ticket
        {
            [DataMember(Name = "job")] public ulong Job { get; set; }
            [DataMember(Name = "pending")] public bool Pending { get; set; }
            [DataMember(Name = "result")] public string Result { get; set; }
            [DataMember(Name = "error")] public string Error { get; set; }
            [DataMember(Name = "total_rows")] public uint TotalRows { get; set; }
            [DataMember(Name = "revision")] public ulong Revision { get; set; }
        }
        internal static Task<BrowserResponse> QueryAsync(BrowserQuery request, CancellationToken cancellation)
        {
            var json = Serialize(request);
            var submitted = Invoke<Ticket>(module => Export<GetQuery>(module, "qs_engine_submit")(json, new UIntPtr((uint)json.Length)));
            if (!string.IsNullOrEmpty(submitted.Error)) throw new InvalidOperationException(submitted.Error);
            var completion = new TaskCompletionSource<BrowserResponse>(TaskCreationOptions.RunContinuationsAsynchronously);
            BrowserResponse response = null;
            uint offset = 0;
            ulong revision = 0;
            bool reading = false, finished = false;
            CancellationTokenRegistration registration = default(CancellationTokenRegistration);
            Action ready = null, page = null;
            Action cleanup = () => {
                if (finished) return;
                finished = true;
                EngineBridge.Completed -= ready;
                registration.Dispose();
                Export<CancelJob>(Module(), "qs_engine_cancel")(submitted.Job);
            };
            page = () => {
                if (finished) return;
                try {
                    var ticket = Invoke<Ticket>(module => Export<TakePage>(module, "qs_engine_take_page")(submitted.Job, new UIntPtr(offset), new UIntPtr(256)));
                    if (ticket.Pending) { response = null; offset = 0; reading = false; return; }
                    if (!string.IsNullOrEmpty(ticket.Error)) throw new InvalidOperationException(ticket.Error);
                    using (var stream = new MemoryStream(Encoding.UTF8.GetBytes(ticket.Result))) {
                        var chunk = (BrowserResponse)new DataContractJsonSerializer(typeof(BrowserResponse)).ReadObject(stream);
                        if (!string.IsNullOrEmpty(chunk.Error)) throw new InvalidOperationException(chunk.Error);
                        if (response == null) { response = chunk; revision = ticket.Revision; }
                        else {
                            if (ticket.Revision != revision) throw new InvalidOperationException("Inputs changed while reading the table. Refresh the request.");
                            response.Rows.AddRange(chunk.Rows); response.Parameters.AddRange(chunk.Parameters);
                        }
                        offset += (uint)chunk.Rows.Count;
                        if (offset < ticket.TotalRows) { EngineBridge.QueuePage(() => page()); return; }
                        cleanup(); completion.TrySetResult(response);
                    }
                } catch (Exception error) { cleanup(); completion.TrySetException(error); }
            };
            ready = () => { if (!reading && !finished) { reading = true; page(); } };
            EngineBridge.Completed += ready;
            registration = cancellation.Register(() => ExcelAsyncUtil.QueueAsMacro(() => { cleanup(); completion.TrySetCanceled(); }));
            return completion.Task;
        }

        private static T Export<T>(IntPtr module, string name) where T : class
        {
            var address = GetProcAddress(module, name);
            if (address == IntPtr.Zero)
                throw new InvalidOperationException("The ribbon and native add-in must come from the same release. Extract and load the complete ZIP.");
            return (T)(object)Marshal.GetDelegateForFunctionPointer(address, typeof(T));
        }

        internal static Task<Snapshot> ReadAsync(string workbook)
        {
            var revision = Revision();
            if (cachedSnapshot != null && cachedWorkbook == workbook && cachedRevision == revision && !cachedSnapshot.IsFaulted && !cachedSnapshot.IsCanceled) return cachedSnapshot;
            // Registry capture stays on its owner thread. Only detached UTF-8
            // data and managed model construction run on the thread pool.
            var bytes = Capture(module => Export<GetSnapshot>(module, "qs_browser_snapshot")(workbook, new UIntPtr((uint)workbook.Length)));
            cachedWorkbook = workbook; cachedRevision = revision;
            return cachedSnapshot = Task.Run(() => Decode<Snapshot>(bytes));
        }

        internal static void ClearSnapshot() { cachedWorkbook = null; cachedSnapshot = null; }

        internal static Task OnExcel(Action action)
        {
            var done = new TaskCompletionSource<bool>(TaskCreationOptions.RunContinuationsAsynchronously);
            try {
                ExcelAsyncUtil.QueueAsMacro(() => {
                    try { action(); done.TrySetResult(true); }
                    catch (Exception error) { done.TrySetException(error); }
                });
            } catch (Exception error) { done.TrySetException(error); }
            return done.Task;
        }

        internal static ulong Revision()
        {
            var revision = Export<GetRevision>(Module(), "qs_browser_revision")();
            if (revision == 0) throw new InvalidOperationException("Read objects on Excel's main thread.");
            return revision;
        }

        internal static void Reset()
        {
            ClearSnapshot();
            var result = Invoke<ResetResponse>(module => Export<ResetMemory>(module, "qs_browser_reset")());
            if (!string.IsNullOrEmpty(result.Error)) throw new InvalidOperationException(result.Error);
        }

        internal static BrowserResponse Query(BrowserQuery request)
        {
            var json = Serialize(request);
            var response = Invoke<BrowserResponse>(module => Export<GetQuery>(module, "qs_browser_query")(json, new UIntPtr((uint)json.Length)));
            if (!string.IsNullOrEmpty(response.Error)) throw new InvalidOperationException(response.Error);
            return response;
        }

        internal static string Serialize<T>(T value)
        {
            using (var stream = new MemoryStream())
            {
                new DataContractJsonSerializer(typeof(T)).WriteObject(stream, value);
                return Encoding.UTF8.GetString(stream.ToArray());
            }
        }
        private static IntPtr Module()
        {
            // Never LoadLibrary: a second copy would have an independent registry.
            var path = Path.Combine(Path.GetDirectoryName(ExcelDnaUtil.XllPath), "quantsupport.xll");
            var module = GetModuleHandle(path);
            if (module == IntPtr.Zero)
                throw new InvalidOperationException("Load quantsupport.xll from the same folder as QuantSupport.Ribbon64.xll, then refresh.");
            if (Export<ApiVersion>(module, "qs_browser_api_version")() != 4)
                throw new InvalidOperationException("This ribbon requires a matching native QuantSupport release.");
            return module;
        }

        private static T Invoke<T>(Func<IntPtr, IntPtr> read)
        {
            return Decode<T>(Capture(read));
        }

        private static T Decode<T>(byte[] bytes)
        {
            using (var stream = new MemoryStream(bytes)) {
                var result = (T)new DataContractJsonSerializer(typeof(T)).ReadObject(stream);
                if (result is Snapshot snapshot && !string.IsNullOrEmpty(snapshot.Error)) throw new InvalidOperationException(snapshot.Error);
                return result;
            }
        }

        private static byte[] Capture(Func<IntPtr, IntPtr> read)
        {
            var module = Module();
            var release = Export<FreeSnapshot>(module, "qs_browser_free");
            var buffer = read(module);
            if (buffer == IntPtr.Zero) throw new InvalidOperationException("The native object browser returned no data.");
            try
            {
                var length = checked((int)strlen(buffer).ToUInt64());
                var bytes = new byte[length];
                Marshal.Copy(buffer, bytes, 0, length);
                return bytes;
            }
            finally { release(buffer); }
        }
    }
}

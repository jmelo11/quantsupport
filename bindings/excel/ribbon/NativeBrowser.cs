using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Runtime.Serialization.Json;
using System.Text;
using ExcelDna.Integration;

namespace QuantSupport.Ribbon
{
    internal static class NativeBrowser
    {
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

        private static T Export<T>(IntPtr module, string name) where T : class
        {
            var address = GetProcAddress(module, name);
            if (address == IntPtr.Zero)
                throw new InvalidOperationException("The ribbon and native add-in must come from the same release. Extract and load the complete ZIP.");
            return (T)(object)Marshal.GetDelegateForFunctionPointer(address, typeof(T));
        }

        // Never LoadLibrary: a second copy would have an empty, independent object registry.
        internal static Snapshot Read(string workbook)
        {
            return Invoke<Snapshot>(module => Export<GetSnapshot>(module, "qs_browser_snapshot")(workbook, new UIntPtr((uint)workbook.Length)));
        }

        internal static ulong Revision()
        {
            var revision = Export<GetRevision>(Module(), "qs_browser_revision")();
            if (revision == 0) throw new InvalidOperationException("Read objects on Excel's main thread.");
            return revision;
        }

        internal static void Reset()
        {
            var result = Invoke<ResetResponse>(module => Export<ResetMemory>(module, "qs_browser_reset")());
            if (!string.IsNullOrEmpty(result.Error)) throw new InvalidOperationException(result.Error);
        }

        internal static BrowserResponse Query(BrowserQuery request)
        {
            string json;
            using (var stream = new MemoryStream())
            {
                new DataContractJsonSerializer(typeof(BrowserQuery)).WriteObject(stream, request);
                json = Encoding.UTF8.GetString(stream.ToArray());
            }
            var response = Invoke<BrowserResponse>(module => Export<GetQuery>(module, "qs_browser_query")(json, new UIntPtr((uint)json.Length)));
            if (!string.IsNullOrEmpty(response.Error)) throw new InvalidOperationException(response.Error);
            return response;
        }

        private static IntPtr Module()
        {
            var path = Path.Combine(Path.GetDirectoryName(ExcelDnaUtil.XllPath), "quantsupport.xll");
            var module = GetModuleHandle(path);
            if (module == IntPtr.Zero)
                throw new InvalidOperationException("Load quantsupport.xll from the same folder as QuantSupport.Ribbon64.xll, then refresh.");
            if (Export<ApiVersion>(module, "qs_browser_api_version")() != 3)
                throw new InvalidOperationException("This ribbon requires a matching native QuantSupport release.");
            return module;
        }

        private static T Invoke<T>(Func<IntPtr, IntPtr> read)
        {
            var module = Module();
            var release = Export<FreeSnapshot>(module, "qs_browser_free");
            var buffer = read(module);
            if (buffer == IntPtr.Zero) throw new InvalidOperationException("The native object browser returned no data.");
            try
            {
                var length = 0;
                while (Marshal.ReadByte(buffer, length) != 0) length++;
                var bytes = new byte[length];
                Marshal.Copy(buffer, bytes, 0, length);
                using (var stream = new MemoryStream(bytes))
                {
                    var result = (T)new DataContractJsonSerializer(typeof(T)).ReadObject(stream);
                    if (result is Snapshot snapshot && !string.IsNullOrEmpty(snapshot.Error)) throw new InvalidOperationException(snapshot.Error);
                    return result;
                }
            }
            finally { release(buffer); }
        }
    }
}

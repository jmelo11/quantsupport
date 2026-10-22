using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Runtime.Serialization.Json;
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
            var path = Path.Combine(Path.GetDirectoryName(ExcelDnaUtil.XllPath), "quantsupport.xll");
            var module = GetModuleHandle(path);
            if (module == IntPtr.Zero)
                throw new InvalidOperationException("Load quantsupport.xll from the same folder as QuantSupport.Ribbon64.xll, then refresh.");
            if (Export<ApiVersion>(module, "qs_browser_api_version")() != 1)
                throw new InvalidOperationException("This ribbon requires a matching native QuantSupport release.");
            var release = Export<FreeSnapshot>(module, "qs_browser_free");
            var buffer = Export<GetSnapshot>(module, "qs_browser_snapshot")(workbook, new UIntPtr((uint)workbook.Length));
            if (buffer == IntPtr.Zero) throw new InvalidOperationException("The native object browser returned no data.");
            try
            {
                var length = 0;
                while (Marshal.ReadByte(buffer, length) != 0) length++;
                var bytes = new byte[length];
                Marshal.Copy(buffer, bytes, 0, length);
                using (var stream = new MemoryStream(bytes))
                {
                    var snapshot = (Snapshot)new DataContractJsonSerializer(typeof(Snapshot)).ReadObject(stream);
                    if (!string.IsNullOrEmpty(snapshot.Error)) throw new InvalidOperationException(snapshot.Error);
                    return snapshot;
                }
            }
            finally { release(buffer); }
        }
    }
}

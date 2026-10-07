namespace AgainstRomeModifier.Tests
{
    public class WinmmExportTests
    {
        [Fact]
        public void ProxyDef_ShouldContainAllSystemWinmmExports()
        {
            var systemWinmmPath = @"C:\Windows\SysWOW64\winmm.dll";
            Assert.True(File.Exists(systemWinmmPath), "System winmm.dll not found");

            var systemExports = GetExports(systemWinmmPath);
            Assert.NotEmpty(systemExports);

            // Read the generated .def file
            var defPath = Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "../../../../../native/argm-trace/argm_trace.def"));
            Assert.True(File.Exists(defPath), "argm_trace.def not found at " + defPath);

            var defLines = File.ReadAllLines(defPath);
            var proxyExports = new HashSet<string>();
            foreach (var line in defLines)
            {
                var trimmed = line.Trim();
                if (string.IsNullOrEmpty(trimmed) || trimmed.StartsWith(';') || trimmed == "LIBRARY winmm" || trimmed == "EXPORTS")
                    continue;

                // Example: mciExecute = Thunk_mciExecute @3
                // Or: Thunk_Ordinal2 @2 NONAME
                var parts = trimmed.Split(new[] { ' ', '=' }, StringSplitOptions.RemoveEmptyEntries);
                if (parts.Length > 0)
                {
                    if (parts[0].StartsWith("Thunk_Ordinal", StringComparison.Ordinal))
                    {
                        // It's a NONAME export, extract ordinal from @...
                        var ordinalStr = parts.FirstOrDefault(p => p.StartsWith('@'));
                        if (ordinalStr != null && int.TryParse(ordinalStr.AsSpan(1), out int ord))
                        {
                            proxyExports.Add($"Ordinal{ord}");
                        }
                    }
                    else
                    {
                        // It's a named export
                        proxyExports.Add(parts[0]);
                    }
                }
            }

            var missing = new List<string>();
            foreach (var exp in systemExports)
            {
                var key = exp.Name == "NONAME" || exp.Name == null ? $"Ordinal{exp.Ordinal}" : exp.Name;
                if (!proxyExports.Contains(key!))
                {
                    missing.Add($"Missing: {key} (@{exp.Ordinal})");
                }
            }

            Assert.Empty(missing);
        }

        /// <summary>The prebuilt proxy embedded into the modifier must be rebuilt whenever the .def changes.</summary>
        [Fact]
        public void PrebuiltProxyBinary_ShouldExportAllSystemWinmmExports()
        {
            var systemWinmmPath = @"C:\Windows\SysWOW64\winmm.dll";
            Assert.True(File.Exists(systemWinmmPath), "System winmm.dll not found");
            var binaryPath = Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "../../../../../ThirdParty/argm-trace/winmm.dll"));
            Assert.True(File.Exists(binaryPath), "Prebuilt proxy not found at " + binaryPath);
            static string Key(ExportInfo exp) => exp.Name == "NONAME" || exp.Name == null ? $"Ordinal{exp.Ordinal}" : exp.Name;
            var proxy = GetExports(binaryPath).Select(Key).ToHashSet();
            Assert.DoesNotContain(GetExports(systemWinmmPath).Select(Key), key => !proxy.Contains(key));
        }

        private sealed class ExportInfo
        {
            public int Ordinal { get; set; }
            public string? Name { get; set; }
        }

        private static List<ExportInfo> GetExports(string path)
        {
            var exports = new List<ExportInfo>();
            using var fs = new FileStream(path, FileMode.Open, FileAccess.Read);
            using var br = new BinaryReader(fs);

            // Read DOS Header
            fs.Position = 0x3C;
            var e_lfanew = br.ReadInt32();

            // Read PE Signature
            fs.Position = e_lfanew;
            var signature = br.ReadUInt32();
            if (signature != 0x00004550) // "PE\0\0"
                throw new InvalidOperationException("Not a PE file");

            fs.Position = e_lfanew + 4 + 2;
            var numSections = br.ReadUInt16();

            fs.Position = e_lfanew + 4 + 16;
            var sizeOfOptionalHeader = br.ReadUInt16();

            fs.Position = e_lfanew + 24;
            var magic = br.ReadUInt16();
            
            uint exportRva = 0;
            if (magic == 0x10b)
            {
                fs.Position = e_lfanew + 24 + 96;
                exportRva = br.ReadUInt32();
            }
            else if (magic == 0x20b)
            {
                fs.Position = e_lfanew + 24 + 112;
                exportRva = br.ReadUInt32();
            }

            if (exportRva == 0) return exports;

            var sectionsOffset = e_lfanew + 24 + sizeOfOptionalHeader;
            var sections = new List<(uint va, uint vsize, uint ptr)>();
            for (int i = 0; i < numSections; i++)
            {
                fs.Position = sectionsOffset + i * 40 + 8;
                var vsize = br.ReadUInt32();
                var va = br.ReadUInt32();
                var rawSize = br.ReadUInt32();
                var ptr = br.ReadUInt32();
                sections.Add((va, vsize, ptr));
            }

            long RvaToOffset(uint rva)
            {
                foreach (var sec in sections)
                {
                    if (rva >= sec.va && rva < sec.va + sec.vsize)
                        return rva - sec.va + sec.ptr;
                }
                return 0;
            }

            string ReadNullTerminatedString(long offset)
            {
                var oldPos = fs.Position;
                fs.Position = offset;
                var chars = new List<byte>();
                byte b;
                while ((b = br.ReadByte()) != 0)
                {
                    chars.Add(b);
                }
                fs.Position = oldPos;
                return System.Text.Encoding.ASCII.GetString(chars.ToArray());
            }

            var exportOffset = RvaToOffset(exportRva);
            fs.Position = exportOffset + 16;
            var ordinalBase = br.ReadInt32();
            var numFuncs = br.ReadInt32();
            var numNames = br.ReadInt32();
            var funcRva = br.ReadUInt32();
            var nameRva = br.ReadUInt32();
            var ordRva = br.ReadUInt32();

            var funcOffset = RvaToOffset(funcRva);
            var nameOffset = RvaToOffset(nameRva);
            var ordOffset = RvaToOffset(ordRva);

            var nameToOrd = new Dictionary<int, string>();
            for (int i = 0; i < numNames; i++)
            {
                fs.Position = nameOffset + i * 4;
                var nRva = br.ReadUInt32();
                var nOffset = RvaToOffset(nRva);
                var name = ReadNullTerminatedString(nOffset);

                fs.Position = ordOffset + i * 2;
                var ordinalIndex = br.ReadUInt16();
                nameToOrd[ordinalIndex] = name;
            }

            for (int i = 0; i < numFuncs; i++)
            {
                fs.Position = funcOffset + i * 4;
                var funcAddrRva = br.ReadUInt32();
                if (funcAddrRva == 0) continue;

                var ordinal = ordinalBase + i;
                if (nameToOrd.TryGetValue(i, out var name))
                {
                    exports.Add(new ExportInfo { Ordinal = ordinal, Name = name });
                }
                else
                {
                    exports.Add(new ExportInfo { Ordinal = ordinal, Name = "NONAME" });
                }
            }

            return exports;
        }
    }
}

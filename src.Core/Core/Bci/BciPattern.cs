using System;
using System.Collections.Generic;

namespace AgainstRomeModifier
{
    public static class BciPattern
    {
        public static void WriteBciInt32(byte[] buffer, int offset, int expectedValue, int value, string patchName)
        {
            Core.Patches.VerifiedBinaryWriter.WriteInt32(buffer, offset, expectedValue, value, patchName);
        }

        public static int FindBciWordPattern(byte[] decompressedBci, int?[] pattern, int startOffset = 0)
        {
            int patternBytes = pattern.Length * 4;
            for (int offset = startOffset; offset <= decompressedBci.Length - patternBytes; offset += 4)
            {
                bool match = true;
                for (int i = 0; i < pattern.Length; i++)
                {
                    int? expected = pattern[i];
                    if (expected.HasValue && BitConverter.ToInt32(decompressedBci, offset + (i * 4)) != expected.Value)
                    {
                        match = false;
                        break;
                    }
                }
                if (match) return offset;
            }
            return -1;
        }

        public static List<int> FindAllBciWordPatternSites(byte[] decompressedBci, int?[] signature)
        {
            var sites = new List<int>();
            for (int off = FindBciWordPattern(decompressedBci, signature);
                 off >= 0;
                 off = FindBciWordPattern(decompressedBci, signature, off + 4))
            {
                sites.Add(off);
            }
            return sites;
        }
    }
}

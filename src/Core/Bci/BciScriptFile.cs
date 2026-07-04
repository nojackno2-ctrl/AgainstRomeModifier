using System;
using System.IO;

namespace AgainstRomeModifier
{
    public class BciScriptFile
    {
        public string FilePath { get; }
        public byte[] RawBytes { get; private set; }
        public byte[] DecompressedBytes { get; private set; }
        public bool IsModified { get; set; }

        public BciScriptFile(string filePath)
        {
            FilePath = filePath;
            RawBytes = File.ReadAllBytes(filePath);
            DecompressedBytes = GameLZSS.DecompressPfil(RawBytes);
            IsModified = false;
        }

        public void UpdateDecompressedBytes(byte[] decompressedBytes)
        {
            DecompressedBytes = decompressedBytes;
            IsModified = true;
        }

        public byte[] GetCompressedBytes()
        {
            if (IsModified)
            {
                RawBytes = GameLZSS.CompressPfil(DecompressedBytes, RawBytes);
                IsModified = false;
            }
            return RawBytes;
        }
    }
}

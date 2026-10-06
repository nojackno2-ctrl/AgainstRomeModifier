using System.Text;

namespace AgainstRomeModifier.Tests;

/// <summary>
/// 診斷工具：設定 ARM_DUMP_SOURCE（地圖目錄）與 ARM_DUMP_TARGET（輸出目錄）時，把所有 PFIL 文字檔解壓成 CP1251 純文字以便比對。
/// 只讀取來源目錄；未設定環境變數時不做任何事。
/// </summary>
public sealed class MapFileDumpTool
{
    [Fact]
    public void Dump_pfil_text_files_when_requested()
    {
        string? source = Environment.GetEnvironmentVariable("ARM_DUMP_SOURCE");
        string? target = Environment.GetEnvironmentVariable("ARM_DUMP_TARGET");
        if (string.IsNullOrWhiteSpace(source) || string.IsNullOrWhiteSpace(target)) return;
        Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);
        Encoding encoding = Encoding.GetEncoding(1251);
        foreach (string path in Directory.GetFiles(source, "*", SearchOption.AllDirectories))
        {
            byte[] bytes = File.ReadAllBytes(path);
            if (bytes.Length < 64 || bytes[0] != 'P' || bytes[1] != 'F' || bytes[2] != 'I' || bytes[3] != 'L') continue;
            string output = Path.Combine(target, Path.GetRelativePath(source, path) + ".txt");
            Directory.CreateDirectory(Path.GetDirectoryName(output)!);
            File.WriteAllText(output, encoding.GetString(GameLZSS.DecompressPfil(bytes)), Encoding.UTF8);
            File.WriteAllBytes(output + ".header", bytes.AsSpan(0, 64).ToArray());
        }
    }
}

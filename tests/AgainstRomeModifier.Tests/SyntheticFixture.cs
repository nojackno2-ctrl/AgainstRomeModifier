using System.Text;

namespace AgainstRomeModifier.Tests;

internal static class SyntheticFixture {
    static SyntheticFixture() => Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);
    internal static Encoding GameEncoding => Encoding.GetEncoding(1251);

    internal static byte[] Pfil(string text) {
        byte[] header = new byte[64];
        header[0] = (byte)'P'; header[1] = (byte)'F'; header[2] = (byte)'I'; header[3] = (byte)'L';
        return GameLZSS.CompressPfil(GameEncoding.GetBytes(text), header);
    }

    internal static string Text(byte[] pfil) => GameEncoding.GetString(GameLZSS.DecompressPfil(pfil));
}

using System.Buffers.Binary;
using System.Text;

namespace AgainstRomeModifier.Scripting;

/// <summary>
/// IPR 腳本映像（BCI0，PFIL 解壓後）的讀寫。佈局（2026-10-07 以 ENDL_000／TUTOR_00 的 ak_level.bci 逐位元組核對）：
/// <code>
/// 0x00 "BCI0"  0x04 version(1)  0x08 codeLen  0x0C symLen(0)  0x10 constLen  0x14 constCount  0x18 varLen  0x1C varCount
/// 0x20 "CODE"  0x24 code[codeLen]（4 位元組字組；跳躍運算元為相對位移）
/// "SYMB" sym[symLen]  "CONS" constBlob[constLen]（NUL 結尾字串：外部函式名與字串常數）
/// "CIDX" constCount × int32（各常數在 blob 中的位移）  "VAR " var[varLen]  "VIDX" varCount × int32
/// dtorAddr int32  mainAddr int32（皆為 code 內的位元組位移）
/// </code>
/// </summary>
public sealed class BciImage
{
    private static readonly Encoding Latin = Encoding.Latin1;

    private BciImage(byte[] code, byte[] symbols, byte[] constBlob, List<int> constOffsets, byte[] variables, int[] variableOffsets, int dtorAddress, int mainAddress)
    {
        Code = code; Symbols = symbols; ConstBlob = constBlob; _constOffsets = constOffsets; Variables = variables; VariableOffsets = variableOffsets;
        DtorAddress = dtorAddress; MainAddress = mainAddress;
    }

    private readonly List<int> _constOffsets;
    public byte[] Code { get; private set; }
    public byte[] Symbols { get; }
    public byte[] ConstBlob { get; private set; }
    public IReadOnlyList<int> ConstOffsets => _constOffsets;
    public byte[] Variables { get; }
    public int[] VariableOffsets { get; }
    public int DtorAddress { get; set; }
    public int MainAddress { get; set; }

    /// <summary>沒有來源地圖初始化或自動生成內容的場景節拍；等待單位沿用原生 ak_level。</summary>
    public static BciImage CreateIdleLevel()
    {
        // Destructor follows the original main-frame cleanup and exit sequence.
        // Main owns one empty frame and repeatedly waits ten native ticks. 131 consumes an integer, not a double.
        int[] words = [95, 75, 66, 0, 87, 130, 74, 94, 73, 0, 66, 10, 131, 112, -20];
        var code = new byte[words.Length * 4];
        for (int index = 0; index < words.Length; index++) BinaryPrimitives.WriteInt32LittleEndian(code.AsSpan(index * 4), words[index]);
        return new BciImage(code, [], [], [], [], [], 0, 24);
    }

    public static BciImage Parse(byte[] data)
    {
        if (data.Length < 0x24 || !data.AsSpan(0, 4).SequenceEqual("BCI0"u8)) throw new InvalidDataException("不是 BCI0 腳本映像。");
        if (Int(data, 4) != 1) throw new InvalidDataException("不支援的 BCI 版本。");
        int codeLen = Int(data, 8), symLen = Int(data, 12), constLen = Int(data, 16), constCount = Int(data, 20), varLen = Int(data, 24), varCount = Int(data, 28);
        int offset = 0x20;
        Expect(data, ref offset, "CODE"); byte[] code = Take(data, ref offset, codeLen);
        Expect(data, ref offset, "SYMB"); byte[] symbols = Take(data, ref offset, symLen);
        Expect(data, ref offset, "CONS"); byte[] blob = Take(data, ref offset, constLen);
        Expect(data, ref offset, "CIDX"); var constOffsets = new List<int>(constCount);
        for (int index = 0; index < constCount; index++) { constOffsets.Add(Int(data, offset)); offset += 4; }
        Expect(data, ref offset, "VAR "); byte[] variables = Take(data, ref offset, varLen);
        Expect(data, ref offset, "VIDX"); var variableOffsets = new int[varCount];
        for (int index = 0; index < varCount; index++) { variableOffsets[index] = Int(data, offset); offset += 4; }
        if (offset + 8 != data.Length) throw new InvalidDataException($"BCI 映像結尾不符（剩餘 {data.Length - offset} 位元組）。");
        int dtor = Int(data, offset), main = Int(data, offset + 4);
        if (codeLen % 4 != 0 || main < 0 || main >= codeLen || dtor < 0 || dtor >= codeLen) throw new InvalidDataException("BCI 進入點超出程式碼範圍。");
        return new BciImage(code, symbols, blob, constOffsets, variables, variableOffsets, dtor, main);
    }

    public byte[] Serialize()
    {
        using var stream = new MemoryStream();
        void W(int value) { Span<byte> buffer = stackalloc byte[4]; BinaryPrimitives.WriteInt32LittleEndian(buffer, value); stream.Write(buffer); }
        stream.Write("BCI0"u8); W(1); W(Code.Length); W(Symbols.Length); W(ConstBlob.Length); W(_constOffsets.Count); W(Variables.Length); W(VariableOffsets.Length);
        stream.Write("CODE"u8); stream.Write(Code);
        stream.Write("SYMB"u8); stream.Write(Symbols);
        stream.Write("CONS"u8); stream.Write(ConstBlob);
        stream.Write("CIDX"u8); foreach (int value in _constOffsets) W(value);
        stream.Write("VAR "u8); stream.Write(Variables);
        stream.Write("VIDX"u8); foreach (int value in VariableOffsets) W(value);
        W(DtorAddress); W(MainAddress);
        return stream.ToArray();
    }

    /// <summary>常數表中的字串（外部函式名與字串常數共用同一個表）。</summary>
    public string Constant(int index)
    {
        int start = _constOffsets[index], end = Array.IndexOf(ConstBlob, (byte)0, start);
        return Latin.GetString(ConstBlob, start, (end < 0 ? ConstBlob.Length : end) - start);
    }

    /// <summary>新增一個常數字串並回傳其索引（不重用既有項目，與原編譯器每次引用各自登錄的作法一致）。</summary>
    public int AddConstant(string value)
        => AddConstantBytes(value, Latin);

    public int AddGameConstant(string value)
        => AddConstantBytes(value, Maps.MapTextEncoding.Game);

    private int AddConstantBytes(string value, Encoding encoding)
    {
        if (value.Contains('\0')) throw new ArgumentException("常數不可含 NUL。", nameof(value));
        byte[] bytes = encoding.GetBytes(value + "\0");
        int offset = ConstBlob.Length;
        ConstBlob = [.. ConstBlob, .. bytes];
        _constOffsets.Add(offset);
        return _constOffsets.Count - 1;
    }

    /// <summary>把一段程式碼附加到程式碼段尾端並回傳其起始位元組位移。</summary>
    public int AppendCode(ReadOnlySpan<byte> code)
    {
        if (code.Length % 4 != 0) throw new ArgumentException("程式碼長度必須是 4 的倍數。", nameof(code));
        int start = Code.Length;
        Code = [.. Code, .. code.ToArray()];
        return start;
    }

    private static int Int(byte[] data, int offset) => BinaryPrimitives.ReadInt32LittleEndian(data.AsSpan(offset));
    private static void Expect(byte[] data, ref int offset, string tag)
    {
        if (offset + 4 > data.Length || Latin.GetString(data, offset, 4) != tag) throw new InvalidDataException($"BCI 映像在 0x{offset:x} 缺少 \"{tag}\" 區段。");
        offset += 4;
    }
    private static byte[] Take(byte[] data, ref int offset, int length)
    {
        if (length < 0 || offset + length > data.Length) throw new InvalidDataException("BCI 區段長度超出檔案。");
        byte[] result = data.AsSpan(offset, length).ToArray(); offset += length; return result;
    }
}

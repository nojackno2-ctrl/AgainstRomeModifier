using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using AgainstRomeModifier;

namespace AgainstRomeMapEditor.Modules.Soundscape;

/// <summary>
/// 音效區域二進位儲存檔案（DATA/sound.dat）與關卡腳本合約序列化器。
/// 支援原生 SNDZ 二進位結構、PFIL 容器壓縮、無損 JSON 交換格式與 IPR 腳本片段匯出。
/// </summary>
public static class SoundscapeBinaryStorage
{
    private static readonly byte[] MagicSndz = "SNDZ"u8.ToArray();
    private const ushort CurrentVersion = 1;

    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true,
        Converters = { new JsonStringEnumConverter() }
    };

    /// <summary>
    /// 將音效快照序列化為二進位位元組陣列（SNDZ 二進位格式）。
    /// </summary>
    public static byte[] SerializeToBinary(SoundscapeSnapshot snapshot)
    {
        ArgumentNullException.ThrowIfNull(snapshot);

        using var ms = new MemoryStream();
        using var writer = new BinaryWriter(ms, Encoding.UTF8, leaveOpen: true);

        // 1. Header (64 bytes 固定標頭)
        writer.Write(MagicSndz);                   // 4 bytes: 'SNDZ'
        writer.Write(CurrentVersion);              // 2 bytes: Version = 1
        writer.Write((ushort)snapshot.Zones.Count);// 2 bytes: ZoneCount
        writer.Write(snapshot.GlobalAmbienceVolume);// 4 bytes: GlobalVolume
        WriteFixedString(writer, snapshot.GlobalAmbienceId, 32); // 32 bytes: GlobalAmbienceId
        writer.Write(new byte[20]);                // 20 bytes: 保留欄位 Padding

        // 2. Zone Records
        foreach (var zone in snapshot.Zones)
        {
            writer.Write(zone.Id.ToByteArray());   // 16 bytes: Guid
            WriteFixedString(writer, zone.Name, 32);           // 32 bytes: Name
            WriteFixedString(writer, zone.SoundDefId, 32);     // 32 bytes: SoundDefId
            writer.Write((byte)zone.ShapeType);               // 1 byte: ShapeType
            writer.Write((byte)zone.AttenuationCurve);        // 1 byte: AttenuationCurve
            writer.Write((byte)zone.Category);                // 1 byte: Category
            writer.Write((byte)0);                            // 1 byte: Reserved/Padding
            writer.Write((uint)zone.Attributes);              // 4 bytes: Attributes
            writer.Write(zone.Priority);                      // 2 bytes: Priority
            writer.Write((ushort)0);                          // 2 bytes: Reserved/Padding
            writer.Write(zone.BaseVolume);                    // 4 bytes: BaseVolume
            writer.Write(zone.InnerRadius);                   // 4 bytes: InnerRadius
            writer.Write(zone.OuterRadius);                   // 4 bytes: OuterRadius
            writer.Write(zone.CenterWorldX);                  // 4 bytes: CenterX
            writer.Write(zone.CenterWorldY);                  // 4 bytes: CenterY
            writer.Write(zone.CenterWorldZ);                  // 4 bytes: CenterZ
            writer.Write(zone.ParamA);                        // 4 bytes: ParamA
            writer.Write(zone.ParamB);                        // 4 bytes: ParamB
            writer.Write(zone.ParamAngleDeg);                 // 4 bytes: ParamAngleDeg

            // 頂點陣列（針對 ConvexPolygon）
            ushort vertexCount = (ushort)(zone.ShapeType == SoundscapeShapeType.ConvexPolygon ? zone.PolygonVertices.Count : 0);
            writer.Write(vertexCount);                        // 2 bytes: VertexCount

            if (vertexCount > 0)
            {
                foreach (var v in zone.PolygonVertices)
                {
                    writer.Write(v.X);                        // 4 bytes: Vertex X
                    writer.Write(v.Z);                        // 4 bytes: Vertex Z
                }
            }
        }

        writer.Flush();
        return ms.ToArray();
    }

    /// <summary>
    /// 自二進位位元組陣列解析 SoundscapeSnapshot。
    /// 若包含 PFIL 壓縮標頭，將自動調用 GameLZSS 解壓縮。
    /// </summary>
    public static SoundscapeSnapshot DeserializeFromBinary(ReadOnlySpan<byte> data)
    {
        if (data.Length < 64)
            throw new InvalidDataException("音效資料長度不足 64 位元組標頭。");

        byte[] rawBytes;
        if (data.Length >= 4 && data[0] == 'P' && data[1] == 'F' && data[2] == 'I' && data[3] == 'L')
        {
            rawBytes = GameLZSS.DecompressPfil(data.ToArray());
        }
        else
        {
            rawBytes = data.ToArray();
        }

        using var ms = new MemoryStream(rawBytes);
        using var reader = new BinaryReader(ms, Encoding.UTF8, leaveOpen: true);

        // 1. Header 驗證
        byte[] magic = reader.ReadBytes(4);
        if (!magic.AsSpan().SequenceEqual(MagicSndz))
            throw new InvalidDataException("非有效的 SNDZ 音效二進位檔案魔數。");

        ushort version = reader.ReadUInt16();
        if (version != CurrentVersion)
            throw new NotSupportedException($"不支援的 SNDZ 版本: {version} (當前預期: {CurrentVersion})");

        ushort zoneCount = reader.ReadUInt16();
        float globalVol = reader.ReadSingle();
        string globalAmbId = ReadFixedString(reader, 32);
        reader.ReadBytes(20); // 略過 20 bytes 保留欄位

        var zones = new List<SoundscapeZone>(zoneCount);

        // 2. Zone Records 解析
        for (int i = 0; i < zoneCount; i++)
        {
            byte[] guidBytes = reader.ReadBytes(16);
            var id = new Guid(guidBytes);
            string name = ReadFixedString(reader, 32);
            string soundDefId = ReadFixedString(reader, 32);
            var shapeType = (SoundscapeShapeType)reader.ReadByte();
            var curve = (AttenuationCurveKind)reader.ReadByte();
            var category = (AudioCategory)reader.ReadByte();
            reader.ReadByte(); // Reserved
            var attributes = (SoundscapeZoneAttributes)reader.ReadUInt32();
            ushort priority = reader.ReadUInt16();
            reader.ReadUInt16(); // Reserved
            float baseVol = reader.ReadSingle();
            float innerR = reader.ReadSingle();
            float outerR = reader.ReadSingle();
            float cx = reader.ReadSingle();
            float cy = reader.ReadSingle();
            float cz = reader.ReadSingle();
            float paramA = reader.ReadSingle();
            float paramB = reader.ReadSingle();
            float paramAngle = reader.ReadSingle();
            ushort vertCount = reader.ReadUInt16();

            var zone = new SoundscapeZone
            {
                Id = id,
                Name = name,
                SoundDefId = soundDefId,
                ShapeType = shapeType,
                AttenuationCurve = curve,
                Category = category,
                Attributes = attributes,
                Priority = priority,
                BaseVolume = baseVol,
                InnerRadius = innerR,
                OuterRadius = outerR,
                CenterWorldX = cx,
                CenterWorldY = cy,
                CenterWorldZ = cz,
                ParamA = paramA,
                ParamB = paramB,
                ParamAngleDeg = paramAngle
            };

            for (int vi = 0; vi < vertCount; vi++)
            {
                float vx = reader.ReadSingle();
                float vz = reader.ReadSingle();
                zone.PolygonVertices.Add(new SoundscapeVertex(vx, vz));
            }

            zones.Add(zone);
        }

        return new SoundscapeSnapshot(globalAmbId, globalVol, zones);
    }

    /// <summary>
    /// 寫入到 DATA/sound.dat 檔案（可選 PFIL LZSS 壓縮）。
    /// </summary>
    public static void SaveToSoundDat(string filePath, SoundscapeSnapshot snapshot, bool compressPfil = false, byte[]? origHeader = null)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(filePath);
        ArgumentNullException.ThrowIfNull(snapshot);

        byte[] raw = SerializeToBinary(snapshot);
        byte[] finalBytes = compressPfil ? GameLZSS.CompressPfil(raw, origHeader ?? new byte[64]) : raw;

        string? dir = Path.GetDirectoryName(filePath);
        if (!string.IsNullOrEmpty(dir))
            Directory.CreateDirectory(dir);

        File.WriteAllBytes(filePath, finalBytes);
    }

    /// <summary>
    /// 從 DATA/sound.dat 讀取快照。
    /// </summary>
    public static SoundscapeSnapshot LoadFromSoundDat(string filePath)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(filePath);

        if (!File.Exists(filePath))
            throw new FileNotFoundException("找不到音效區域檔案", filePath);

        byte[] data = File.ReadAllBytes(filePath);
        return DeserializeFromBinary(data);
    }

    /// <summary>
    /// 匯出為無損 JSON 字串（供版本控制、除錯或跨工具交換）。
    /// </summary>
    public static string ExportToJson(SoundscapeSnapshot snapshot)
    {
        ArgumentNullException.ThrowIfNull(snapshot);
        return JsonSerializer.Serialize(snapshot, JsonOptions);
    }

    /// <summary>
    /// 自 JSON 字串反序列化快照。
    /// </summary>
    public static SoundscapeSnapshot ImportFromJson(string json)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(json);
        return JsonSerializer.Deserialize<SoundscapeSnapshot>(json, JsonOptions)
            ?? throw new InvalidDataException("無法從 JSON 反序列化 SoundscapeSnapshot。");
    }

    /// <summary>
    /// 匯出為原版 IPR 腳本（ak_level.bci 相容）觸發輔助片段。
    /// 可直接嵌入地圖腳本以在舊版引擎無二進位 sound.dat 讀取器時以腳本條件觸發。
    /// </summary>
    public static string ExportToScript(SoundscapeSnapshot snapshot)
    {
        ArgumentNullException.ThrowIfNull(snapshot);

        var sb = new StringBuilder();
        sb.AppendLine("// ==========================================================================");
        sb.AppendLine("// Against Rome - Generated Soundscape Script Trigger Contract");
        sb.AppendLine($"// Global Ambience: {snapshot.GlobalAmbienceId} (Vol: {snapshot.GlobalAmbienceVolume:F2})");
        sb.AppendLine($"// Total Zones: {snapshot.Zones.Count}");
        sb.AppendLine("// ==========================================================================");
        sb.AppendLine();
        sb.AppendLine("void InitMapSoundscapes()");
        sb.AppendLine("{");

        int idx = 0;
        foreach (var z in snapshot.Zones)
        {
            if (!z.Attributes.HasFlag(SoundscapeZoneAttributes.IsActive))
                continue;

            sb.AppendLine($"    // Zone #{++idx}: {z.Name} ({z.SoundDefId})");
            sb.AppendLine($"    // Type: {z.ShapeType}, Center: ({z.CenterWorldX:F1}, {z.CenterWorldZ:F1}), InnerR: {z.InnerRadius:F1}, OuterR: {z.OuterRadius:F1}");

            if (z.ShapeType == SoundscapeShapeType.Point)
            {
                sb.AppendLine($"    // Point Emitter: s_playVoiceSampleMP(\"{z.SoundDefId}\", {(int)z.CenterWorldX}, {(int)z.CenterWorldZ});");
            }
            else
            {
                sb.AppendLine($"    // Area Trigger: if (s_posInSightDist(listener, {(int)z.CenterWorldX}, {(int)z.CenterWorldZ}, {(int)z.OuterRadius})) {{");
                sb.AppendLine($"    //     s_setVoiceSubGroup(\"{z.SoundDefId}\", {(int)(z.BaseVolume * 100)});");
                sb.AppendLine("    // }");
            }
            sb.AppendLine();
        }

        sb.AppendLine("}");
        return sb.ToString();
    }

    #region 私有輔助函式

    private static void WriteFixedString(BinaryWriter writer, string text, int fixedBytes)
    {
        byte[] bytes = Encoding.UTF8.GetBytes(text ?? string.Empty);
        byte[] buffer = new byte[fixedBytes];
        Array.Copy(bytes, buffer, Math.Min(bytes.Length, fixedBytes));
        writer.Write(buffer);
    }

    private static string ReadFixedString(BinaryReader reader, int fixedBytes)
    {
        byte[] buffer = reader.ReadBytes(fixedBytes);
        int nullIdx = Array.IndexOf(buffer, (byte)0);
        int len = nullIdx >= 0 ? nullIdx : fixedBytes;
        return Encoding.UTF8.GetString(buffer, 0, len);
    }

    #endregion
}

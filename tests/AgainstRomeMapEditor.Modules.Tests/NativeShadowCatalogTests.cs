using System.Buffers.Binary;
using AgainstRomeMapEditor.NativeAssets;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class NativeShadowCatalogTests
{
    private const string SampleShadowNames = """
        [ShadowNames]
        0000,small_round_shadow.bmp
        0001,schuetze_shadow.bmp
        0255,shadowmap_haupthaus_1.bmp
        0283,shadowmap_wohnhaus_1.bmp
        """;

    private const string SampleObjdef = """
        [ObjectDefaults]
        ;idx ,activ,alen ,rotsp,moves,alrid,anadd,disps,alrml,handt,mltyp,shidx,shsiz,shtyp,aptix,shacx,shacz,palty,selsi,lpmax,lpbar,mpmax,mpbar,movsf,sirad,timfl,fowse,ptime,resb1,resb2,resb3,resb4,resb5,resb6,resr1,resr2,resr3,resr4,resr5,resr6,resob,maxwo,maxre,maxol,selec,   afram,aafrn,aafrw,minde,fldth,wirad,cmrad,-------------name-------------,  sex,typus
        0, 1, 1, 0, 0.0, 0, -1, 1, -1, 0, -1, 0, 25, 1, -1, 0, 0, 0, 0, 10, 0, 0, 0, 0.0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -1, 0, 0, 0, 0, 0, -1, -1, 1, 1, 0, 0, SmallRock, 0, 0
        1, 1, 1, 0, 0.0, 1, -1, 1, -1, 0, -1, 1, 30, 1, -1, 5, -3, 0, 0, 10, 0, 0, 0, 0.0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -1, 0, 0, 0, 0, 0, -1, -1, 1, 1, 0, 0, ArcherUnit, 0, 0
        37, 1, 1, 0, 0.0, -1, -1, 1, -1, 0, -1, 283, 138, 0, 1, -48, 21, 0, 0, 10, 0, 0, 0, 0.0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -1, 0, 0, 0, 0, 0, -1, -1, 1, 1, 0, 0, BauGerWoh00_Wohnhaus, 0, 0
        232, 1, 1, 0, 0.0, -1, -1, 1, -1, 0, -1, 255, 277, 0, 9, -50, 35, 0, 0, 10, 0, 0, 0, 0.0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -1, 0, 0, 0, 0, 0, -1, -1, 1, 1, 0, 0, BauGerHau00_Haupthaus, 0, 0
        999, 1, 1, 0, 0.0, -1, -1, 1, -1, 0, -1, -1, 0, 0, -1, 0, 0, 0, 0, 10, 0, 0, 0, 0.0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -1, 0, 0, 0, 0, 0, -1, -1, 1, 1, 0, 0, NoShadowObject, 0, 0
        """;

    [Fact]
    public void ParseShadowNames_ParsesValidEntries()
    {
        var names = NativeShadowCatalog.ParseShadowNames(SampleShadowNames);
        Assert.Equal(4, names.Count);
        Assert.Equal("small_round_shadow.bmp", names[0]);
        Assert.Equal("schuetze_shadow.bmp", names[1]);
        Assert.Equal("shadowmap_haupthaus_1.bmp", names[255]);
        Assert.Equal("shadowmap_wohnhaus_1.bmp", names[283]);
    }

    [Fact]
    public void ParseObjdefShadows_FiltersNegativeShadowsAndResolvesMappings()
    {
        var names = NativeShadowCatalog.ParseShadowNames(SampleShadowNames);
        var defs = NativeShadowCatalog.ParseObjdefShadows(SampleObjdef, names);

        Assert.Equal(4, defs.Count);
        Assert.False(defs.ContainsKey("NoShadowObject"));

        var hau = defs["BauGerHau00_Haupthaus"];
        Assert.Equal(255, hau.ShadowId);
        Assert.Equal(277, hau.ShadowSize);
        Assert.Equal(NativeShadowType.Box, hau.ShadowType);
        Assert.Equal(-50, hau.CorrectionX);
        Assert.Equal(35, hau.CorrectionZ);
        Assert.Equal("shadowmap_haupthaus_1.bmp", hau.TextureFileName);

        var archer = defs["ArcherUnit"];
        Assert.Equal(1, archer.ShadowId);
        Assert.Equal(30, archer.ShadowSize);
        Assert.Equal(NativeShadowType.Circle, archer.ShadowType);
        Assert.Equal(5, archer.CorrectionX);
        Assert.Equal(-3, archer.CorrectionZ);
        Assert.Equal("schuetze_shadow.bmp", archer.TextureFileName);
    }

    [Fact]
    public void Catalog_ReturnsNull_ForUnknownOrNoShadowObjects()
    {
        var catalog = CreateSyntheticCatalog();
        Assert.Null(catalog.GetShadow("NonExistentObject"));
        Assert.Null(catalog.GetShadow("NoShadowObject"));
    }

    [Fact]
    public void Catalog_ResolvesShadowDocument_AndCalculatesGeometry()
    {
        var catalog = CreateSyntheticCatalog();
        var shadow = catalog.GetShadow("BauGerWoh00_Wohnhaus");
        Assert.NotNull(shadow);
        Assert.Equal("BauGerWoh00_Wohnhaus", shadow.ObjectName);
        Assert.Equal(138, shadow.Definition.ShadowSize);
        Assert.Equal(NativeShadowType.Box, shadow.Definition.ShadowType);

        // Test World Quad calculation
        // Anchor at (11392, 10112), shacx=-48, shacz=21
        // Center = (11344, 10133), size=138
        var quad = shadow.ComputeWorldQuad(11392, 10112);
        Assert.Equal(11344, quad.CenterX);
        Assert.Equal(10133, quad.CenterZ);
        Assert.Equal(138, quad.HalfExtent);
        Assert.Equal(11344 - 138, quad.CornerNW_X);
        Assert.Equal(10133 - 138, quad.CornerNW_Z);
        Assert.Equal(11344 + 138, quad.CornerSE_X);
        Assert.Equal(10133 + 138, quad.CornerSE_Z);

        // Test Screen Offset calculation
        // DeltaX = (-48 - 21) / 2 = -34.5
        // DeltaY = (-48 + 21) / 4 = -6.75
        var (screenDx, screenDy) = shadow.ComputeScreenOffset();
        Assert.Equal(-34.5, screenDx);
        Assert.Equal(-6.75, screenDy);

        // Test mask retrieval and frame decoding
        var frame = shadow.Document.DecodeFrame();
        Assert.Equal(2, frame.Width);
        Assert.Equal(2, frame.Height);
        Assert.Equal(4, frame.AlphaMask.Length);
    }

    [Fact]
    public void Catalog_CachesDocuments_AcrossMultipleCalls()
    {
        var catalog = CreateSyntheticCatalog();
        var shadow1 = catalog.GetShadow("BauGerWoh00_Wohnhaus");
        var shadow2 = catalog.GetShadow("BauGerWoh00_Wohnhaus");
        Assert.NotNull(shadow1);
        Assert.NotNull(shadow2);
        Assert.Same(shadow1.Document, shadow2.Document);
    }

    private static NativeShadowCatalog CreateSyntheticCatalog()
    {
        byte[] fakeBmp = CreateSyntheticBmp8(2, 2);
        return NativeShadowCatalog.FromText(
            SampleObjdef,
            SampleShadowNames,
            textureName => fakeBmp);
    }

    private static byte[] CreateSyntheticBmp8(int width, int height)
    {
        int stride = (width + 3) & ~3;
        int pixelBytes = stride * height;
        int paletteSize = 256 * 4;
        int fileSize = 14 + 40 + paletteSize + pixelBytes;
        byte[] bytes = new byte[fileSize];

        // BITMAPFILEHEADER
        BinaryPrimitives.WriteUInt16LittleEndian(bytes.AsSpan(0), 0x4D42); // BM
        BinaryPrimitives.WriteUInt32LittleEndian(bytes.AsSpan(2), (uint)fileSize);
        BinaryPrimitives.WriteUInt32LittleEndian(bytes.AsSpan(10), (uint)(14 + 40 + paletteSize)); // offset

        // BITMAPINFOHEADER
        BinaryPrimitives.WriteUInt32LittleEndian(bytes.AsSpan(14), 40);
        BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(18), width);
        BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(22), height);
        BinaryPrimitives.WriteUInt16LittleEndian(bytes.AsSpan(26), 1); // planes
        BinaryPrimitives.WriteUInt16LittleEndian(bytes.AsSpan(28), 8); // bpp
        BinaryPrimitives.WriteUInt32LittleEndian(bytes.AsSpan(30), 0); // BI_RGB
        BinaryPrimitives.WriteUInt32LittleEndian(bytes.AsSpan(34), (uint)pixelBytes);
        BinaryPrimitives.WriteUInt32LittleEndian(bytes.AsSpan(46), 256); // colors

        // Palette
        int palOffset = 54;
        for (int i = 0; i < 256; i++)
        {
            bytes[palOffset + i * 4] = (byte)i;
            bytes[palOffset + i * 4 + 1] = (byte)i;
            bytes[palOffset + i * 4 + 2] = (byte)i;
            bytes[palOffset + i * 4 + 3] = 0;
        }

        // Pixels (fill with test gradient)
        int pxOffset = 14 + 40 + paletteSize;
        for (int y = 0; y < height; y++)
        {
            for (int x = 0; x < width; x++)
            {
                bytes[pxOffset + y * stride + x] = (byte)(y * 100 + x * 50);
            }
        }

        return bytes;
    }
}

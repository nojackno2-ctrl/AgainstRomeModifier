using System.Buffers.Binary;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed class NativePoolDiagnosticsTests
{
    private static void U16(byte[] b, int offset, ushort val) => BinaryPrimitives.WriteUInt16LittleEndian(b.AsSpan(offset), val);
    private static void U32(byte[] b, int offset, uint val) => BinaryPrimitives.WriteUInt32LittleEndian(b.AsSpan(offset), val);

    private sealed class TestDataPools : IDisposable
    {
        public const int Count = 4;
        public const int Groups = 2;
        public const int PosCount = 8;
        private readonly string _dir = Path.Combine(Path.GetTempPath(), "NativePoolTest_" + Guid.NewGuid().ToString("N"));

        public string DataPath => Path.Combine(_dir, "DATA");

        public byte[] Objects = new byte[16 + Count * (79 + 42)]; // 79 record + 42 columns (18 columns sum to 42)
        public byte[] Anim = new byte[8 + Count * 25];
        public byte[] Gfxtype = new byte[8 + Count * 17];
        public byte[] Action = new byte[12 + Count * 25];
        public byte[] Hirarchy = new byte[12 + Groups * 105];
        public byte[] ObjData = new byte[8 + Count * 123];
        public byte[] Position = new byte[8 + PosCount * 17];
        public byte[] Engine = new byte[94];

        public TestDataPools()
        {
            Directory.CreateDirectory(DataPath);

            // objects.dat header
            U32(Objects, 0, 1); U32(Objects, 4, Count); U32(Objects, 8, 30); U32(Objects, 12, 30);
            // anim.dat header
            U32(Anim, 0, 1); U32(Anim, 4, Count);
            // gfxtype.dat header
            U32(Gfxtype, 0, 1); U32(Gfxtype, 4, Count);
            // action.dat header
            U32(Action, 0, 1); U32(Action, 4, Count); U32(Action, 8, 6);
            // hirarchy.dat header
            U32(Hirarchy, 0, 1); U32(Hirarchy, 4, Groups); U32(Hirarchy, 8, 50);
            // objdata.dat header
            U32(ObjData, 0, 1); U32(ObjData, 4, Count);
            // position.dat header
            U32(Position, 0, 1); U32(Position, 4, PosCount);
            // engine.dat header
            U32(Engine, 0, 1); U32(Engine, 34, 100); // counter = 100

            // 建立 2 個 active 物件 (slot 0, slot 1)
            int col1Offset = 16 + Count * 79 + Count * 2;
            int col4Offset = 16 + Count * 79 + Count * 8; // col 0,1,2,3 each 2 bytes -> col4 at offset 8

            for (int s = 0; s < 2; s++)
            {
                int r = 16 + s * 79;
                Objects[r] = 1; // active
                U16(Objects, r + 1, (ushort)s); // team
                U32(Objects, r + 3, (uint)(s + 1)); // uid = 1, 2
                U16(Objects, r + 67, (ushort)(s * 2)); // pos0
                U16(Objects, r + 69, (ushort)(s * 2 + 1)); // pos1
                U16(Objects, r + 71, (ushort)s); // anim link
                U16(Objects, r + 73, (ushort)s); // gfxtype link
                U16(Objects, r + 77, (ushort)s); // action link

                // objdata link (column 1)
                U16(Objects, col1Offset + s * 2, (ushort)s);
                // hirarchy link (column 4)
                U16(Objects, col4Offset + s * 2, (ushort)0); // both in group 0

                // anim active
                Anim[8 + s * 21] = 1;
                // gfxtype active
                Gfxtype[8 + s * 15] = 1;
                // action active
                Action[12 + s * 25] = 1;
                // objdata active
                ObjData[8 + s * 9] = 1;
                // positions valid
                Position[8 + (s * 2) * 17] = 1;
                Position[8 + (s * 2 + 1) * 17] = 1;
            }

            // hirarchy group 0 active with members [0, 1]
            int gOff = 12;
            Hirarchy[gOff] = 1; // active
            U16(Hirarchy, gOff + 1, 2); // member count = 2
            U16(Hirarchy, gOff + 3, 0); // member 0 = slot 0
            U16(Hirarchy, gOff + 5, 1); // member 1 = slot 1

            // empty slots for s=2, 3
            for (int s = 2; s < Count; s++)
            {
                int r = 16 + s * 79;
                Objects[r] = 0;
                U16(Objects, col1Offset + s * 2, 0xFFFF);
                U16(Objects, col4Offset + s * 2, 0xFFFF);
            }
        }

        public void WriteAll(bool usePfil = false)
        {
            void Save(string name, byte[] data)
            {
                string p = Path.Combine(DataPath, name);
                if (usePfil)
                {
                    byte[] fakeHeader = new byte[64];
                    fakeHeader[0] = (byte)'P'; fakeHeader[1] = (byte)'F'; fakeHeader[2] = (byte)'I'; fakeHeader[3] = (byte)'L';
                    File.WriteAllBytes(p, GameLZSS.CompressPfil(data, fakeHeader));
                }
                else File.WriteAllBytes(p, data);
            }

            Save("objects.dat", Objects);
            Save("anim.dat", Anim);
            Save("gfxtype.dat", Gfxtype);
            Save("action.dat", Action);
            Save("hirarchy.dat", Hirarchy);
            Save("objdata.dat", ObjData);
            Save("position.dat", Position);
            Save("engine.dat", Engine);
        }

        public void Dispose()
        {
            try { Directory.Delete(_dir, true); } catch { }
        }
    }

    [Fact]
    public void DiagnoseDirectory_WhenDirectoryNull_ThrowsArgumentNullException()
    {
        Assert.Throws<ArgumentNullException>(() => NativePoolDiagnostics.DiagnoseDirectory(null!));
    }

    [Fact]
    public void DiagnoseDirectory_WhenMissingObjects_ReportsError()
    {
        string emptyDir = Path.Combine(Path.GetTempPath(), "Empty_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(emptyDir);
        try
        {
            var report = NativePoolDiagnostics.DiagnoseDirectory(emptyDir);
            Assert.False(report.IsHealthy);
            Assert.Contains(report.Issues, i => i.Code == "missing-objects");
        }
        finally
        {
            Directory.Delete(emptyDir, true);
        }
    }

    [Fact]
    public void DiagnoseDirectory_WhenAllConsistent_ReportsHealthy()
    {
        using var fixture = new TestDataPools();
        fixture.WriteAll();

        var report = NativePoolDiagnostics.DiagnoseDirectory(fixture.DataPath);

        Assert.True(report.IsHealthy);
        Assert.Equal(2, report.ActiveObjects);
        Assert.Equal(2, report.ActiveAnimSlots);
        Assert.Equal(2, report.ActiveGfxtypeSlots);
        Assert.Equal(2, report.ActiveActionSlots);
        Assert.Equal(1, report.ActiveGroups);
        Assert.Equal(100u, report.EngineUidCounter);
        Assert.DoesNotContain(report.Issues, i => i.Severity == NativePoolIssueSeverity.Error);
    }

    [Fact]
    public void DiagnoseDirectory_WithPfilCompression_DecodesAndValidatesSuccessfully()
    {
        using var fixture = new TestDataPools();
        fixture.WriteAll(usePfil: true);

        var report = NativePoolDiagnostics.DiagnoseDirectory(fixture.DataPath);

        Assert.True(report.IsHealthy);
        Assert.Equal(2, report.ActiveObjects);
    }

    [Fact]
    public void DiagnoseDirectory_WhenAnimLinkOutOfRange_ReportsError()
    {
        using var fixture = new TestDataPools();
        U16(fixture.Objects, 16 + 71, 999); // slot 0 anim link = 999
        fixture.WriteAll();

        var report = NativePoolDiagnostics.DiagnoseDirectory(fixture.DataPath);

        Assert.False(report.IsHealthy);
        Assert.Contains(report.Issues, i => i.Code == "anim-out-of-range" && i.ObjectSlot == 0);
    }

    [Fact]
    public void DiagnoseDirectory_WhenAnimTargetInactive_ReportsError()
    {
        using var fixture = new TestDataPools();
        fixture.Anim[8 + 0 * 21] = 0; // slot 0 anim is inactive
        fixture.WriteAll();

        var report = NativePoolDiagnostics.DiagnoseDirectory(fixture.DataPath);

        Assert.False(report.IsHealthy);
        Assert.Contains(report.Issues, i => i.Code == "anim-target-inactive" && i.ObjectSlot == 0);
    }

    [Fact]
    public void DiagnoseDirectory_WhenGfxtypeTargetInactive_ReportsError()
    {
        using var fixture = new TestDataPools();
        fixture.Gfxtype[8 + 1 * 15] = 0; // slot 1 gfxtype inactive
        fixture.WriteAll();

        var report = NativePoolDiagnostics.DiagnoseDirectory(fixture.DataPath);

        Assert.False(report.IsHealthy);
        Assert.Contains(report.Issues, i => i.Code == "gfxtype-target-inactive" && i.ObjectSlot == 1);
    }

    [Fact]
    public void DiagnoseDirectory_WhenActionTargetInactive_ReportsError()
    {
        using var fixture = new TestDataPools();
        fixture.Action[12 + 0 * 25] = 0; // slot 0 action inactive
        fixture.WriteAll();

        var report = NativePoolDiagnostics.DiagnoseDirectory(fixture.DataPath);

        Assert.False(report.IsHealthy);
        Assert.Contains(report.Issues, i => i.Code == "action-target-inactive" && i.ObjectSlot == 0);
    }

    [Fact]
    public void DiagnoseDirectory_WhenObjDataTargetInactive_ReportsError()
    {
        using var fixture = new TestDataPools();
        fixture.ObjData[8 + 0 * 9] = 0; // slot 0 objdata inactive
        fixture.WriteAll();

        var report = NativePoolDiagnostics.DiagnoseDirectory(fixture.DataPath);

        Assert.False(report.IsHealthy);
        Assert.Contains(report.Issues, i => i.Code == "objdata-target-inactive" && i.ObjectSlot == 0);
    }

    [Fact]
    public void DiagnoseDirectory_WhenPositionInvalid_ReportsError()
    {
        using var fixture = new TestDataPools();
        fixture.Position[8 + 0 * 17] = 0; // pos0 for slot 0 invalid
        fixture.WriteAll();

        var report = NativePoolDiagnostics.DiagnoseDirectory(fixture.DataPath);

        Assert.False(report.IsHealthy);
        Assert.Contains(report.Issues, i => i.Code == "position-0-invalid" && i.ObjectSlot == 0);
    }

    [Fact]
    public void DiagnoseDirectory_WhenHirarchyMemberMismatch_ReportsError()
    {
        using var fixture = new TestDataPools();
        // Change group 0 members to only slot 0
        U16(fixture.Hirarchy, 12 + 1, 1); // count = 1
        fixture.WriteAll();

        var report = NativePoolDiagnostics.DiagnoseDirectory(fixture.DataPath);

        Assert.False(report.IsHealthy);
        // slot 1 points to group 0, but group 0 does not contain slot 1
        Assert.Contains(report.Issues, i => i.Code == "hirarchy-member-mismatch" && i.ObjectSlot == 1);
    }

    [Fact]
    public void DiagnoseDirectory_WhenDuplicateUid_ReportsError()
    {
        using var fixture = new TestDataPools();
        U32(fixture.Objects, 16 + 79 + 3, 1); // slot 1 uid set to 1 (same as slot 0)
        fixture.WriteAll();

        var report = NativePoolDiagnostics.DiagnoseDirectory(fixture.DataPath);

        Assert.False(report.IsHealthy);
        Assert.Contains(report.Issues, i => i.Code == "duplicate-uid" && i.ObjectSlot == 1);
    }

    [Fact]
    public void DiagnoseDirectory_WhenUidExceedsEngineCounter_ReportsWarning()
    {
        using var fixture = new TestDataPools();
        U32(fixture.Engine, 34, 1); // engine counter = 1, but slot 1 uid is 2
        fixture.WriteAll();

        var report = NativePoolDiagnostics.DiagnoseDirectory(fixture.DataPath);

        // Warning does not make IsHealthy false unless there are errors
        Assert.True(report.IsHealthy);
        Assert.Contains(report.Issues, i => i.Code == "uid-exceeds-counter" && i.Severity == NativePoolIssueSeverity.Warning && i.ObjectSlot == 1);
    }
}

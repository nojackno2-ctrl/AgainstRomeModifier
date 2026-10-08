using System.Numerics;
using AgainstRomeMapEditor.Modules.Cinematics;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests.Cinematics;

public sealed class CutsceneSequenceCatalogTests
{
    [Fact]
    public void Catalog_AddAndRetrieve_TracksCountAndDirty()
    {
        var catalog = new CutsceneSequenceCatalog();
        Assert.False(catalog.IsDirty);
        Assert.Equal(0, catalog.Count);

        var seq = new CutsceneSequence
        {
            Name = "阿爾卑斯之戰開場",
            CameraWaypoints =
            [
                new CameraWaypoint(new Vector3(1000, 50, 1000), duration: 0f),
                new CameraWaypoint(new Vector3(2000, 20, 2000), duration: 4f)
            ],
            Subtitles =
            [
                new SubtitleKeyframe(1.0f, 3.0f, "族長", "羅馬軍隊已經越過山口！")
            ]
        };

        catalog.Add(seq);
        Assert.Equal(1, catalog.Count);
        Assert.True(catalog.IsDirty);

        catalog.AcceptChanges();
        Assert.False(catalog.IsDirty);
    }

    [Fact]
    public void Catalog_Validate_DetectsInvalidCoordinatesAndPitch()
    {
        var catalog = new CutsceneSequenceCatalog();
        var invalidSeq = new CutsceneSequence
        {
            Name = "錯誤航點測試",
            CameraWaypoints =
            [
                new CameraWaypoint(new Vector3(20000, 0, 1000)), // 超過 16384
                new CameraWaypoint(new Vector3(1000, 0, 1000), pitchDegrees: 2f) // 低於 5 度
            ]
        };

        catalog.Add(invalidSeq);
        Assert.Throws<InvalidDataException>(() => catalog.Validate());
    }

    [Fact]
    public void Catalog_Duplicate_CreatesValidUniqueClone()
    {
        var catalog = new CutsceneSequenceCatalog();
        var seq = new CutsceneSequence
        {
            Name = "原創戰役運鏡",
            CameraWaypoints =
            [
                new CameraWaypoint(new Vector3(1000, 0, 1000), duration: 0f),
                new CameraWaypoint(new Vector3(2000, 0, 2000), duration: 3f)
            ]
        };
        catalog.Add(seq);

        int newIndex = catalog.Duplicate(0, " (複製版)");
        Assert.Equal(1, newIndex);
        Assert.Equal(2, catalog.Count);

        var copy = catalog.Capture()[1];
        Assert.NotEqual(seq.Id, copy.Id);
        Assert.Equal("原創戰役運鏡 (複製版)", copy.Name);
    }

    [Fact]
    public void Catalog_JsonSerialization_RoundtripsIdentically()
    {
        var original = new CutsceneSequenceCatalog();
        original.Add(new CutsceneSequence
        {
            Name = "測試序列",
            CameraWaypoints =
            [
                new CameraWaypoint(new Vector3(4000, 100, 5000), pitchDegrees: 35f, yawDegrees: 60f, zoom: 75f, duration: 0f),
                new CameraWaypoint(new Vector3(4500, 80, 5500), pitchDegrees: 30f, yawDegrees: 90f, zoom: 82f, duration: 2.5f)
            ],
            Subtitles =
            [
                new SubtitleKeyframe(0.5f, 2.0f, "百夫長", "列陣迎敵！")
            ],
            UnitOrders =
            [
                new UnitOrderKeyframe(1.0f, "GER_WARRIORS", CutsceneUnitOrderKind.MoveTo, 4800, 5200)
            ],
            FxEvents =
            [
                new CutsceneFxKeyframe(0f, CutsceneFxEventKind.LetterboxOn)
            ]
        });

        string json = original.ToJson();
        var restored = CutsceneSequenceCatalog.FromJson(json);

        Assert.Equal(original.Count, restored.Count);
        var restoredSeq = restored.Capture()[0];
        Assert.Equal("測試序列", restoredSeq.Name);
        Assert.Equal(2, restoredSeq.CameraWaypoints.Count);
        Assert.Single(restoredSeq.Subtitles);
        Assert.Single(restoredSeq.UnitOrders);
        Assert.Single(restoredSeq.FxEvents);
    }
}

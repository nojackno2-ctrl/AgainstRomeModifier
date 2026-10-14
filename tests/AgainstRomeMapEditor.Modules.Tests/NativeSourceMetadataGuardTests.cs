using System.Buffers.Binary;
using AgainstRomeModifier;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed partial class NativeSourceFamilyCandidateTests
{
    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Brand_new_prebuilt_with_raw_rotation_without_native_copy_request_is_rejected_and_cannot_fallback_to_template(bool pfil)
    {
        // Case (1): Brand-new Prebuilt RawRotation has finite non-integer radians and matching display angle,
        // but no native copy request and no previous binding.
        // Template callback returns a valid OwnedTemplate(0, 100).
        // It must NOT recreate a native object using rounded angle while carrying the original Raw metadata,
        // and must be rejected (expected InvalidDataException).
        float radians = 1.234567f;
        int matchingDisplayAngle = ScenarioLevelObjects.RadiansToDisplayAngle(radians);

        using var map = new NativeFixture(pfil, count: 8, rootRotationRadians: radians);
        var store = LevelObjectStore.Load(map.Map);
        var initialObjects = store.Objects();
        var initialFiles = map.Files();
        var root = initialObjects.Single(o => o.Slot == 0);
        Assert.Equal(BitConverter.SingleToInt32Bits(radians), BitConverter.SingleToInt32Bits(root.Rotation));

        var validTemplate = store.OwnedTemplate(0, 100);
        Assert.NotNull(validTemplate);

        Guid spawnId = Guid.NewGuid();
        var previous = new ScenarioDocument(); // No previous binding
        var brandNewSpawn = new ScenarioSpawn("BauTest", root.X + 20f, root.Z + 30f, root.Team,
            Angle: matchingDisplayAngle, Y: root.Y, Prebuilt: true)
        {
            Id = spawnId,
            RawRotation = radians
        };
        var next = new ScenarioDocument { Spawns = [brandNewSpawn] };

        // Empty copy requests - not requesting native copy
        var requests = new Dictionary<Guid, ScenarioFamilyCopyRequest>();

        // Template callback returns valid snapshot template
        LevelObjectTemplate? TemplateCallback(ScenarioSpawn spawn) => spawn.Alias == "BauTest" ? validTemplate : null;

        Assert.Throws<InvalidDataException>(() => ScenarioLevelObjects.ApplyToCandidate(
            store, previous, next, TemplateCallback, requests,
            nativeAliasResolver: _ => "BauTest"));

        // All source objects, files, and documents must remain completely unchanged
        Assert.Equal(initialObjects, store.Objects());
        Assert.Empty(previous.Spawns);
        Assert.Empty(previous.DataSlots);
        Assert.Single(next.Spawns);
        Assert.Empty(next.DataSlots);
        foreach (var (filePath, bytes) in initialFiles)
            Assert.Equal(bytes, File.ReadAllBytes(filePath));
    }

    [Theory]
    [InlineData(false, "StaleUid")]
    [InlineData(false, "MissingSlot")]
    [InlineData(false, "MissingBinding")]
    [InlineData(true, "StaleUid")]
    [InlineData(true, "MissingSlot")]
    [InlineData(true, "MissingBinding")]
    public void Bound_raw_native_with_corrupted_or_missing_previous_binding_is_rejected_and_cannot_fallback_to_template(bool pfil, string corruptionMode)
    {
        // Case (2): Bound raw native spawn has matching id/alias/raw/team/display between previous and current,
        // but previous binding has stale UID, slot missing root, or binding is missing entirely.
        // It must NOT fallback to template reconstruction (creating a new UID or skipping), and must reject upfront.
        float radians = 1.234567f;
        int matchingDisplayAngle = ScenarioLevelObjects.RadiansToDisplayAngle(radians);

        using var map = new NativeFixture(pfil, count: 8, rootRotationRadians: radians);
        var store = LevelObjectStore.Load(map.Map);
        var initialObjects = store.Objects();
        var initialFiles = map.Files();
        var root = initialObjects.Single(o => o.Slot == 0);
        Assert.Equal(100u, root.Uid);
        Assert.Equal(BitConverter.SingleToInt32Bits(radians), BitConverter.SingleToInt32Bits(root.Rotation));

        var validTemplate = store.OwnedTemplate(0, 100);
        Assert.NotNull(validTemplate);

        Guid existingId = Guid.NewGuid();
        var spawn = new ScenarioSpawn("BauTest", root.X, root.Z, root.Team,
            Angle: matchingDisplayAngle, Y: root.Y, Prebuilt: true)
        {
            Id = existingId,
            RawRotation = radians
        };

        var previous = new ScenarioDocument { Spawns = [spawn] };
        switch (corruptionMode)
        {
            case "StaleUid":
                // Slot 0 exists, but UID 999 is stale (not 100)
                previous.DataSlots = [new ScenarioDataSlot(0, 999u) { SpawnId = existingId }];
                break;
            case "MissingSlot":
                // Slot 7 is inactive / empty in count: 8 fixture
                previous.DataSlots = [new ScenarioDataSlot(7, 100u) { SpawnId = existingId }];
                break;
            case "MissingBinding":
                // No DataSlot binding for existingId
                previous.DataSlots = [];
                break;
            default:
                throw new ArgumentOutOfRangeException(nameof(corruptionMode));
        }

        var next = new ScenarioDocument
        {
            Spawns = [spawn with { }],
            DataSlots = previous.DataSlots.Select(d => d with { }).ToList()
        };

        LevelObjectTemplate? TemplateCallback(ScenarioSpawn spawn) => spawn.Alias == "BauTest" ? validTemplate : null;

        Assert.Throws<InvalidDataException>(() => ScenarioLevelObjects.ApplyToCandidate(
            store, previous, next, TemplateCallback,
            familyCopies: null,
            nativeAliasResolver: _ => "BauTest"));

        // Asserts source store, files, previous and next remain completely unchanged
        Assert.Equal(initialObjects, store.Objects());
        Assert.Single(previous.Spawns);
        Assert.Single(next.Spawns);
        foreach (var (filePath, bytes) in initialFiles)
            Assert.Equal(bytes, File.ReadAllBytes(filePath));
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Bound_raw_native_changing_alias_cannot_early_continue_and_fallback_to_template_recreation(bool pfil)
    {
        // Case (3): Bound raw native with identical Id changes alias (callback provides valid template).
        // It must NOT early-continue Remove/Add to recreate via template, and must be rejected.
        float radians = 1.234567f;
        int matchingDisplayAngle = ScenarioLevelObjects.RadiansToDisplayAngle(radians);

        using var map = new NativeFixture(pfil, count: 8, rootRotationRadians: radians);
        var store = LevelObjectStore.Load(map.Map);
        var initialObjects = store.Objects();
        var initialFiles = map.Files();
        var root = initialObjects.Single(o => o.Slot == 0);

        var validTemplate = store.OwnedTemplate(0, 100);
        Assert.NotNull(validTemplate);

        Guid existingId = Guid.NewGuid();
        var prevSpawn = new ScenarioSpawn("BauTest", root.X, root.Z, root.Team,
            Angle: matchingDisplayAngle, Y: root.Y, Prebuilt: true)
        {
            Id = existingId,
            RawRotation = radians
        };
        var binding = new ScenarioDataSlot(0, 100u) { SpawnId = existingId };
        var previous = new ScenarioDocument
        {
            Spawns = [prevSpawn],
            DataSlots = [binding]
        };

        // Same Id, but alias changed to "BauAlt"
        var nextSpawn = new ScenarioSpawn("BauAlt", root.X, root.Z, root.Team,
            Angle: matchingDisplayAngle, Y: root.Y, Prebuilt: true)
        {
            Id = existingId,
            RawRotation = radians
        };
        var next = new ScenarioDocument
        {
            Spawns = [nextSpawn],
            DataSlots = [binding with { }]
        };

        LevelObjectTemplate? TemplateCallback(ScenarioSpawn spawn) => validTemplate;

        Assert.Throws<InvalidDataException>(() => ScenarioLevelObjects.ApplyToCandidate(
            store, previous, next, TemplateCallback,
            familyCopies: null,
            nativeAliasResolver: _ => "BauTest"));

        Assert.Equal(initialObjects, store.Objects());
        Assert.Single(previous.Spawns);
        Assert.Single(previous.DataSlots);
        Assert.Single(next.Spawns);
        Assert.Single(next.DataSlots);
        foreach (var (filePath, bytes) in initialFiles)
            Assert.Equal(bytes, File.ReadAllBytes(filePath));
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Bound_native_dropping_raw_rotation_cannot_fallback_to_owned_template_recreation(bool pfil)
    {
        // Case (4): Previous has RawRotation, current drops RawRotation (null).
        // It must NOT recreate via OwnedTemplate, and must be rejected.
        float radians = 1.234567f;
        int matchingDisplayAngle = ScenarioLevelObjects.RadiansToDisplayAngle(radians);

        using var map = new NativeFixture(pfil, count: 8, rootRotationRadians: radians);
        var store = LevelObjectStore.Load(map.Map);
        var initialObjects = store.Objects();
        var initialFiles = map.Files();
        var root = initialObjects.Single(o => o.Slot == 0);

        var validTemplate = store.OwnedTemplate(0, 100);
        Assert.NotNull(validTemplate);

        Guid existingId = Guid.NewGuid();
        var prevSpawn = new ScenarioSpawn("BauTest", root.X, root.Z, root.Team,
            Angle: matchingDisplayAngle, Y: root.Y, Prebuilt: true)
        {
            Id = existingId,
            RawRotation = radians
        };
        var binding = new ScenarioDataSlot(0, 100u) { SpawnId = existingId };
        var previous = new ScenarioDocument
        {
            Spawns = [prevSpawn],
            DataSlots = [binding]
        };

        // Same Id and coordinates, but RawRotation dropped to null
        var nextSpawn = new ScenarioSpawn("BauTest", root.X, root.Z, root.Team,
            Angle: matchingDisplayAngle, Y: root.Y, Prebuilt: true)
        {
            Id = existingId,
            RawRotation = null
        };
        var next = new ScenarioDocument
        {
            Spawns = [nextSpawn],
            DataSlots = [binding with { }]
        };

        LevelObjectTemplate? TemplateCallback(ScenarioSpawn spawn) => spawn.Alias == "BauTest" ? validTemplate : null;

        Assert.Throws<InvalidDataException>(() => ScenarioLevelObjects.ApplyToCandidate(
            store, previous, next, TemplateCallback,
            familyCopies: null,
            nativeAliasResolver: _ => "BauTest"));

        Assert.Equal(initialObjects, store.Objects());
        Assert.Single(previous.Spawns);
        Assert.Single(previous.DataSlots);
        Assert.Single(next.Spawns);
        Assert.Single(next.DataSlots);
        foreach (var (filePath, bytes) in initialFiles)
            Assert.Equal(bytes, File.ReadAllBytes(filePath));
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Bound_native_with_raw_rotation_can_be_deleted_cleanly_without_blanket_raw_rejection(bool pfil)
    {
        // Positive case: A bound native family with RawRotation can be legitimately deleted
        // when removed from the next document. It should NOT be blanket blocked because of RawRotation.
        float radians = 1.234567f;
        int matchingDisplayAngle = ScenarioLevelObjects.RadiansToDisplayAngle(radians);

        using var map = new NativeFixture(pfil, count: 8, rootRotationRadians: radians);
        var store = LevelObjectStore.Load(map.Map);
        var initialObjects = store.Objects();
        var initialFiles = map.Files();
        var root = initialObjects.Single(o => o.Slot == 0);

        var validTemplate = store.OwnedTemplate(0, 100);
        Assert.NotNull(validTemplate);

        Guid existingId = Guid.NewGuid();
        var existingSpawn = new ScenarioSpawn("BauTest", root.X, root.Z, root.Team,
            Angle: matchingDisplayAngle, Y: root.Y, Prebuilt: true)
        {
            Id = existingId,
            RawRotation = radians
        };
        var binding = new ScenarioDataSlot(0, 100u) { SpawnId = existingId };

        var previous = new ScenarioDocument
        {
            Spawns = [existingSpawn],
            DataSlots = [binding]
        };

        // Next document removes the spawn (empty spawns and empty bindings)
        var next = new ScenarioDocument
        {
            Spawns = [],
            DataSlots = []
        };

        var result = ScenarioLevelObjects.ApplyToCandidate(
            store, previous, next,
            _ => throw new InvalidOperationException("Deletion should not request template."),
            familyCopies: null,
            nativeAliasResolver: _ => "BauTest");

        // 1. Inputs remain untouched
        Assert.Equal(initialObjects, store.Objects());
        Assert.Single(previous.Spawns);
        Assert.Single(previous.DataSlots);
        Assert.Empty(next.Spawns);
        Assert.Empty(next.DataSlots);
        foreach (var (filePath, bytes) in initialFiles)
            Assert.Equal(bytes, File.ReadAllBytes(filePath));

        // 2. Candidate scenario and store have the root family removed cleanly
        Assert.Empty(result.Scenario.Spawns);
        Assert.Empty(result.Scenario.DataSlots);
        Assert.Null(result.Store.FamilyAt(0, 100u));
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Legacy_null_raw_rotation_changing_alias_or_with_stale_fallback_can_recreate_via_template_in_apply(bool pfil)
    {
        // Positive case: Legacy prebuilt spawn with RawRotation == null allows changing alias or falling back
        // to template recreation when previous binding is stale/missing, preserving existing Apply behavior.
        using var map = new NativeFixture(pfil, count: 8, rootRotationRadians: 0f);
        var store = LevelObjectStore.Load(map.Map);
        var initialObjects = store.Objects();
        var root = initialObjects.Single(o => o.Slot == 0);
        var validTemplate = store.OwnedTemplate(0, 100);
        Assert.NotNull(validTemplate);

        Guid spawnId = Guid.NewGuid();
        var prevSpawn = new ScenarioSpawn("BauOld", root.X, root.Z, root.Team, Angle: 0, Y: root.Y, Prebuilt: true)
        {
            Id = spawnId,
            RawRotation = null
        };
        var previous = new ScenarioDocument
        {
            Spawns = [prevSpawn],
            DataSlots = [new ScenarioDataSlot(0, 100u) { SpawnId = spawnId }]
        };

        // Alias changed to "BauNew", RawRotation remains null
        var nextSpawn = new ScenarioSpawn("BauNew", root.X, root.Z, root.Team, Angle: 0, Y: root.Y, Prebuilt: true)
        {
            Id = spawnId,
            RawRotation = null
        };
        var next = new ScenarioDocument
        {
            Spawns = [nextSpawn],
            DataSlots = [new ScenarioDataSlot(0, 100u) { SpawnId = spawnId }]
        };

        LevelObjectTemplate? TemplateCallback(ScenarioSpawn spawn) => spawn.Alias == "BauNew" ? validTemplate : null;

        var skipped = ScenarioLevelObjects.Apply(store, previous, next, TemplateCallback);
        Assert.Empty(skipped);
        var binding = Assert.Single(next.DataSlots);
        Assert.Equal(spawnId, binding.SpawnId);
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Multi_scenario_apply_aborts_before_first_mutation_when_native_stale_even_if_ordinary_spawn_changed(bool pfil)
    {
        // Positive case: Multiple objects in scenario - an ordinary spawn (slot 3) changed alias/position,
        // but another native spawn with RawRotation has stale UID / corrupt binding.
        // In-place ScenarioLevelObjects.Apply must reject before ANY store mutation,
        // so the ordinary object (slot 3) is NOT deleted, replaced, or touched in the store.
        float radians = 1.234567f;
        int matchingDisplayAngle = ScenarioLevelObjects.RadiansToDisplayAngle(radians);

        using var map = NativeFixture.CreateWithFamily(pfil, count: 12, rootRotationRadians: radians);
        var store = LevelObjectStore.Load(map.Map);
        var initialObjects = store.Objects();
        var initialFiles = map.Files();
        var root = initialObjects.Single(o => o.Slot == 0);
        var independent = initialObjects.Single(o => o.Slot == 3);

        Guid ordinaryId = Guid.NewGuid();
        Guid nativeRawId = Guid.NewGuid();

        // Previous document has an ordinary prebuilt object (slot 3, RawRotation == null)
        // and a native prebuilt object (slot 0, RawRotation == radians)
        var prevOrdinary = new ScenarioSpawn("BauOrdinary", independent.X, independent.Z, independent.Team, Angle: 0, Y: independent.Y, Prebuilt: true)
        {
            Id = ordinaryId,
            RawRotation = null
        };
        var prevNative = new ScenarioSpawn("BauTest", root.X, root.Z, root.Team, Angle: matchingDisplayAngle, Y: root.Y, Prebuilt: true)
        {
            Id = nativeRawId,
            RawRotation = radians
        };

        var previous = new ScenarioDocument
        {
            Spawns = [prevOrdinary, prevNative],
            // Note: slot 0 has STALE UID 9999u (actual is 100u)
            DataSlots = [
                new ScenarioDataSlot(independent.Slot, independent.Uid) { SpawnId = ordinaryId },
                new ScenarioDataSlot(root.Slot, 9999u) { SpawnId = nativeRawId }
            ]
        };

        // Next document changes the ordinary object's position/alias, keeping the stale native raw object
        var nextOrdinary = new ScenarioSpawn("BauOrdinaryMod", independent.X + 10f, independent.Z + 10f, independent.Team, Angle: 0, Y: independent.Y, Prebuilt: true)
        {
            Id = ordinaryId,
            RawRotation = null
        };
        var nextNative = prevNative with { };

        var next = new ScenarioDocument
        {
            Spawns = [nextOrdinary, nextNative],
            DataSlots = previous.DataSlots.Select(d => d with { }).ToList()
        };

        var ordinaryTemplate = store.OwnedTemplate(independent.Slot, independent.Uid);
        Assert.NotNull(ordinaryTemplate);

        LevelObjectTemplate? TemplateCallback(ScenarioSpawn s) => s.Alias == "BauOrdinaryMod" ? ordinaryTemplate : null;

        // Calling in-place ScenarioLevelObjects.Apply directly on store
        Assert.Throws<InvalidDataException>(() => ScenarioLevelObjects.Apply(store, previous, next, TemplateCallback));

        // CRITICAL: The store objects must remain completely intact and NOT mutated (slot 3 must not be removed!)
        Assert.Equal(initialObjects, store.Objects());
        foreach (var (filePath, bytes) in initialFiles)
            Assert.Equal(bytes, File.ReadAllBytes(filePath));
    }
}


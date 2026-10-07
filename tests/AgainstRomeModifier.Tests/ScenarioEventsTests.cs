using System.Buffers.Binary;
using System.Text;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

public sealed class ScenarioEventsTests : IDisposable
{
    private readonly string _root = Path.Combine(Path.GetTempPath(), "ArmEvents_" + Guid.NewGuid().ToString("N"));
    private static readonly string[] Aliases = ["GER_INF01"];
    private static ScenarioEvent Event(int delay = 2, bool repeat = false) => new("Timer", delay, repeat)
        { Actions = [new(ScenarioActionKind.Message, "Привет!")] };

    [Fact]
    public void All_conditions_track_targets_before_timer_and_before_other_conditions_hold()
    {
        Guid dead = Guid.NewGuid(), exists = Guid.NewGuid();
        var item = Event() with { Conditions = [new(ScenarioConditionKind.ObjectExists, exists), new(ScenarioConditionKind.ObjectDeadOrRemoved, dead)] };
        var scenario = BuildingTargets(dead, exists); scenario.Events = [item];
        BciImage image = Fixture(); ScenarioEventCompiler.Inject(image, [item], 0, scenario);
        var vm = new TestVm(image); vm.Objects[43] = (123, false);
        vm.Tick(0); Assert.Empty(vm.Messages);
        // 前一條件為 false 時，後面的死亡目標仍必須被追蹤。
        vm.Objects.Clear(); vm.Objects[44] = (124, false);
        vm.Tick(1000); Assert.Empty(vm.Messages);
        vm.Tick(2000); Assert.Single(vm.Messages);
        vm.Tick(5000); Assert.Single(vm.Messages); Assert.Equal(1, vm.StackDepth);
    }

    [Fact]
    public void Uid_reuse_does_not_retarget_exists_but_counts_as_removal_of_confirmed_target()
    {
        Guid id = Guid.NewGuid(); var scenario = BuildingTargets(id);
        var exists = Event(0) with { Conditions = [new(ScenarioConditionKind.ObjectExists, id)] };
        var removed = Event(2) with { Conditions = [new(ScenarioConditionKind.ObjectDeadOrRemoved, id)] };
        BciImage image = Fixture(); ScenarioEventCompiler.Inject(image, [exists, removed], 0, scenario);
        var vm = new TestVm(image); vm.Objects[43] = (999, false);
        vm.Tick(0); Assert.Empty(vm.Messages); // 錯誤 UID 從未算存在。
        vm.Objects[43] = (123, false); vm.Tick(1000); Assert.Single(vm.Messages);
        vm.Objects[43] = (999, true); vm.Tick(2000); Assert.Equal(2, vm.Messages.Count);
        Assert.Equal(1, vm.StackDepth);
    }

    [Fact]
    public void Dead_flag_and_corpse_removal_fire_once_after_timer()
    {
        Guid id = Guid.NewGuid(); var scenario = BuildingTargets(id);
        var item = Event() with { Conditions = [new(ScenarioConditionKind.ObjectDeadOrRemoved, id)] };
        BciImage image = Fixture(); ScenarioEventCompiler.Inject(image, [item], 0, scenario);
        var vm = new TestVm(image); vm.Objects[43] = (123, false); vm.Tick(0);
        vm.Objects[43] = (123, true); vm.Tick(1000); Assert.Empty(vm.Messages);
        vm.Tick(2000); Assert.Single(vm.Messages);
        vm.Objects.Clear(); vm.Tick(5000); Assert.Single(vm.Messages);
    }

    [Fact]
    public void Failed_script_spawn_never_arms_dead_or_removed_condition()
    {
        Guid id = Guid.NewGuid(); var item = Event(0) with { Conditions = [new(ScenarioConditionKind.ObjectDeadOrRemoved, id)] };
        var scenario = new ScenarioDocument { Spawns = [new("GER_INF01", 4000, 5000, 0, Count: 3) { Id = id }], Events = [item] };
        BciImage image = Fixture(); LevelScriptInjector.Inject(image, scenario.ScriptSpawns);
        ScenarioEventCompiler.Inject(image, [item], 0, scenario);
        var vm = new TestVm(image); vm.CreationResults.Enqueue((-1, 43, 123, true));
        vm.Objects[43] = (123, true); vm.Tick(0); vm.Objects.Clear(); vm.Tick(10000);
        Assert.Empty(vm.Messages); Assert.Equal(1, vm.StackDepth);
    }

    [Fact]
    public void Script_spawn_binding_drives_existence_and_repeat_waits_for_conditions()
    {
        Guid id = Guid.NewGuid(); var item = Event(1, true) with { Conditions = [new(ScenarioConditionKind.ObjectExists, id)] };
        var scenario = new ScenarioDocument { Spawns = [new("GER_INF01", 4000, 5000, 0, Count: 3) { Id = id }] };
        BciImage image = Fixture(); LevelScriptInjector.Inject(image, scenario.ScriptSpawns);
        ScenarioEventCompiler.Inject(image, [item], 0, scenario);
        var vm = new TestVm(image); vm.CreationResults.Enqueue((1, 43, 123, true));
        vm.Tick(0); vm.Tick(3000); Assert.Empty(vm.Messages);
        vm.Objects[43] = (123, false); vm.Tick(4000); Assert.Single(vm.Messages);
        Assert.Equal(5000, vm.Variables["ARM_EVENT_DEADLINE_0"]);
        vm.Objects.Clear(); vm.Tick(5000); Assert.Single(vm.Messages);
        vm.Objects[43] = (123, false); vm.Tick(5500); Assert.Equal(2, vm.Messages.Count);
        Assert.Equal(6500, vm.Variables["ARM_EVENT_DEADLINE_0"]); Assert.Equal(1, vm.StackDepth);
    }

    [Fact]
    public void Missing_target_binding_and_malformed_conditions_fail_before_image_or_file_mutation()
    {
        Guid id = Guid.NewGuid(); var item = Event() with { Conditions = [new(ScenarioConditionKind.ObjectExists, id)] };
        BciImage image = Fixture(); byte[] original = image.Serialize();
        foreach (ScenarioDocument? document in new ScenarioDocument?[] { null, new(), new() { Spawns = [new("HOUSE", 0, 0, 0, Prebuilt: true) { Id = id }] } })
        {
            Assert.Throws<InvalidDataException>(() => ScenarioEventCompiler.Inject(image, [item], 0, document));
            Assert.Equal(original, image.Serialize());
        }
        var scenario = BuildingTargets(id); scenario.Events = [item];
        Directory.CreateDirectory(_root);
        using (var rollback = new FileRollbackScope()) { scenario.Save(_root, rollback); rollback.Commit(); }
        ScenarioDocument loaded = ScenarioDocument.Load(_root); Assert.Equal(item.Conditions, loaded.Events[0].Conditions);
        byte[] json = File.ReadAllBytes(Path.Combine(_root, ScenarioDocument.FileName));
        scenario.Spawns.Clear(); scenario.DataSlots.Clear();
        using (var rollback = new FileRollbackScope()) Assert.Throws<InvalidDataException>(() => scenario.Save(_root, rollback));
        Assert.Equal(json, File.ReadAllBytes(Path.Combine(_root, ScenarioDocument.FileName)));
        ScenarioEventCompiler.Inject(image, [item with { Enabled = false }], 0, scenario); Assert.Equal(original, image.Serialize());
        Assert.Throws<InvalidDataException>(() => ScenarioEventValidator.ValidateConditions([item with { Conditions = null! }]));
        Assert.Throws<InvalidDataException>(() => ScenarioEventValidator.ValidateConditions([item with { Conditions = [new((ScenarioConditionKind)99, id)] }]));
        Assert.Throws<InvalidDataException>(() => ScenarioEventValidator.ValidateConditions([item with { Conditions = Enumerable.Repeat(item.Conditions[0], 33).ToList() }]));
    }

    private static ScenarioDocument BuildingTargets(params Guid[] ids) => new()
    {
        Spawns = ids.Select(id => new ScenarioSpawn("HOUSE", 4000, 5000, 0, Prebuilt: true) { Id = id }).ToList(),
        DataSlots = ids.Select((id, index) => new ScenarioDataSlot(42 + index, (uint)(123 + index)) { SpawnId = id }).ToList()
    };

    [Fact]
    public void Recompiling_after_data_rebinding_uses_new_pair_for_same_condition_target()
    {
        Guid id = Guid.NewGuid(); var scenario = BuildingTargets(id);
        var item = Event(0) with { Conditions = [new(ScenarioConditionKind.ObjectExists, id)] };
        BciImage first = Fixture(); ScenarioEventCompiler.Inject(first, [item], 0, scenario);
        var originalVm = new TestVm(first); originalVm.Objects[43] = (123, false); originalVm.Tick(0);
        Assert.Single(originalVm.Messages);
        scenario.DataSlots[0] = new(8, 456) { SpawnId = id };
        BciImage moved = Fixture(); ScenarioEventCompiler.Inject(moved, [item], 0, scenario);
        var movedVm = new TestVm(moved); movedVm.Objects[43] = (123, false); movedVm.Tick(0);
        Assert.Empty(movedVm.Messages);
        movedVm.Objects[9] = (456, false); movedVm.Tick(1000); Assert.Single(movedVm.Messages);
        Assert.Equal(id, item.Conditions[0].TargetId); Assert.Equal(1, movedVm.StackDepth);
    }

    [Fact]
    public void Spawn_bindings_preserve_native_pairs_across_unit_order_and_building_wait()
    {
        Guid building = Guid.NewGuid(), unit = Guid.NewGuid();
        BciImage image = Fixture();
        LevelScriptInjector.Inject(image, [new("HOUSE", 6000, 7000, 0) { Id = building },
            new("GER_INF01", 4000, 5000, 0, Count: 3) { Id = unit }]);
        var vm = new TestVm(BciImage.Parse(image.Serialize()));
        vm.CreationResults.Enqueue((1, 42, 901, true));
        vm.CreationResults.Enqueue((1, 7, 902, true));
        vm.Tick(0); // 部隊已建立，建築仍在原生成等待。
        Assert.Equal(42, vm.Variables[ScenarioObjectIdentity.RuntimeIndexKey(unit)]);
        Assert.Equal(901, vm.Variables[ScenarioObjectIdentity.RuntimeUidKey(unit)]);
        Assert.False(vm.Variables.ContainsKey(ScenarioObjectIdentity.RuntimeIndexKey(building)));
        vm.Tick(1);
        Assert.Equal(7, vm.Variables[ScenarioObjectIdentity.RuntimeIndexKey(building)]);
        Assert.Equal(902, vm.Variables[ScenarioObjectIdentity.RuntimeUidKey(building)]);
        Assert.Equal(new[] { "s_createUnitAndMems", "s_createObj" }, vm.Creations);
        Assert.All(vm.CreationInputs, pair => Assert.Equal((0, -1), pair));
        vm.Tick(1000);
        Assert.Equal(2, vm.Creations.Count); Assert.Equal(1, vm.StackDepth);
        Assert.Equal(new[] { 10, 10, 10 }, vm.Waits);
    }

    [Theory]
    [InlineData(-1, false)]
    [InlineData(-1, true)]
    [InlineData(0, true)]
    [InlineData(2, true)]
    public void Failed_spawn_cannot_inherit_prior_pair_or_publish_partial_outputs(int result, bool writes)
    {
        Guid first = Guid.NewGuid(), failed = Guid.NewGuid();
        BciImage image = Fixture();
        LevelScriptInjector.Inject(image, [new("GER_INF01", 4000, 5000, 0, Count: 3) { Id = first },
            new("GER_INF01", 4500, 5500, 0, Count: 3) { Id = failed }]);
        var vm = new TestVm(image);
        vm.Variables[ScenarioObjectIdentity.RuntimeIndexKey(failed)] = 999;
        vm.Variables[ScenarioObjectIdentity.RuntimeUidKey(failed)] = 123;
        vm.CreationResults.Enqueue((1, 42, 901, true));
        vm.CreationResults.Enqueue((result, 43, 902, writes));
        vm.Tick(0);
        Assert.Equal(42, vm.Variables[ScenarioObjectIdentity.RuntimeIndexKey(first)]);
        Assert.Equal(901, vm.Variables[ScenarioObjectIdentity.RuntimeUidKey(first)]);
        Assert.Equal(0, vm.Variables[ScenarioObjectIdentity.RuntimeIndexKey(failed)]);
        Assert.Equal(-1, vm.Variables[ScenarioObjectIdentity.RuntimeUidKey(failed)]);
        Assert.All(vm.CreationInputs, pair => Assert.Equal((0, -1), pair));
        Assert.Equal(1, vm.StackDepth);
    }

    [Fact]
    public void Duplicate_spawn_identity_is_rejected_before_changing_image()
    {
        BciImage image = Fixture(); byte[] original = image.Serialize(); Guid id = Guid.NewGuid();
        Assert.Throws<InvalidDataException>(() => LevelScriptInjector.Inject(image,
            [new("GER_INF01", 4000, 5000, 0, Count: 3) { Id = id }, new("HOUSE", 6000, 7000, 0) { Id = id }]));
        Assert.Equal(original, image.Serialize());
    }

    [Fact]
    public void Once_timer_preserves_main_wait_and_stack_and_fires_at_deadline()
    {
        BciImage image = Fixture(); byte[] before = image.Code.ToArray();
        ScenarioEventCompiler.Inject(image, [Event()], image.MainAddress);
        Assert.Equal(before.AsSpan(0, 16).ToArray(), image.Code.AsSpan(0, 16).ToArray());
        Assert.Equal(before.AsSpan(24).ToArray(), image.Code.AsSpan(24, before.Length - 24).ToArray());
        var vm = new TestVm(image);
        vm.Tick(0); vm.Tick(1999); Assert.Empty(vm.Messages);
        vm.Tick(2000); Assert.Equal("Привет!", Assert.Single(vm.Messages));
        vm.Tick(10000); Assert.Single(vm.Messages);
        Assert.Equal(3, vm.OriginalTicks); Assert.Equal(1, vm.StackDepth);
        Assert.All(vm.Waits, ticks => Assert.Equal(10, ticks));
        Assert.Equal(-1, vm.Variables["ARM_EVENT_DEADLINE_0"]);
    }

    [Fact]
    public void Computed_wait_is_replayed_after_restoring_original_local_frame()
    {
        // The original endless loop computes its delay in a local before waiting.
        BciImage image = Fixture([74, 94, 73, 1, 66, 7, 91, 0, 128, 0, 90, 0, 131, 112, -28]);
        ScenarioEventCompiler.Inject(image, [Event()], 0);
        var vm = new TestVm(image); vm.Tick(0); vm.Tick(2000); vm.Tick(5000);
        Assert.Single(vm.Messages);
        Assert.Equal(3, vm.OriginalTicks);
        Assert.All(vm.Waits, value => Assert.Equal(7, value));
        Assert.Equal(2, vm.StackDepth);
    }

    [Fact]
    public void Repeating_timer_uses_interval_without_catching_up_multiple_actions()
    {
        BciImage image = Fixture(); ScenarioEventCompiler.Inject(image, [Event(repeat: true)], 0);
        var vm = new TestVm(image);
        vm.Tick(0); vm.Tick(5500); Assert.Single(vm.Messages);
        Assert.Equal(7500, vm.Variables["ARM_EVENT_DEADLINE_0"]);
        vm.Tick(7499); Assert.Single(vm.Messages);
        vm.Tick(7500); Assert.Equal(2, vm.Messages.Count);
        Assert.Equal(1, vm.StackDepth);
    }

    [Fact]
    public void Multiple_actions_keep_argument_order_and_execute_original_spawn_shim()
    {
        BciImage image = Fixture(); int originalMain = image.MainAddress; Guid id = Guid.NewGuid();
        LevelScriptInjector.Inject(image, [new("GER_INF01", 4000, 5000, 0, Count: 3) { Id = id }]);
        var item = new ScenarioEvent("Actions", 0)
        {
            Actions = [new(ScenarioActionKind.Diplomacy, Team: 0, OtherTeam: 2, Hostile: false),
                new(ScenarioActionKind.SpawnUnit, Team: 3, Alias: "GER_INF01", X: 1000, Z: 2000, Count: 7),
                new(ScenarioActionKind.Message, "Ready")]
        };
        ScenarioEventValidator.Validate([item], Aliases);
        ScenarioEventCompiler.Inject(image, [item], originalMain);
        var vm = new TestVm(image);
        vm.CreationResults.Enqueue((1, 42, 901, true));
        vm.CreationResults.Enqueue((1, 43, 902, true));
        vm.Tick(0);
        Assert.Equal(2, vm.Spawns.Count);
        Assert.Equal((0, 4000, 5000, "GER_INF01", 3), vm.Spawns[0]);
        Assert.Equal((3, 1000, 2000, "GER_INF01", 7), vm.Spawns[1]);
        Assert.Equal((0, 2, 0), Assert.Single(vm.Diplomacy));
        Assert.Equal("Ready", Assert.Single(vm.Messages));
        Assert.Equal(1, vm.StackDepth);
        vm.Tick(1000);
        Assert.Equal(42, vm.Variables[ScenarioObjectIdentity.RuntimeIndexKey(id)]);
        Assert.Equal(901, vm.Variables[ScenarioObjectIdentity.RuntimeUidKey(id)]);
        Assert.Equal(2, vm.Spawns.Count);
    }

    [Fact]
    public void Disabled_events_leave_the_image_byte_identical()
    {
        BciImage image = Fixture(); byte[] before = image.Serialize();
        ScenarioEventCompiler.Inject(image, [Event() with { Enabled = false }], 0);
        Assert.Equal(before, image.Serialize());
    }

    [Fact]
    public void Unsupported_or_ambiguous_hooks_fail_before_modifying_the_image()
    {
        foreach (int[] words in new[] { new[] { 66, 10, 131, 123 }, new[] { 999, 66, 10, 131, 112, -20 },
            new[] { 66, 0, 117, 20, 66, 10, 131, 112, -20, 66, 10, 131, 112, -20 }, new[] { 66, 10, 131, 112, -16 } })
        {
            BciImage image = Fixture(words); byte[] before = image.Serialize();
            Assert.Throws<InvalidDataException>(() => ScenarioEventCompiler.Inject(image, [Event()], 0));
            Assert.Equal(before, image.Serialize());
        }
    }

    [Fact]
    public void Operand_bytes_cannot_be_mistaken_for_a_wait_instruction()
    {
        BciImage image = Fixture([67, 66, 10, 131, 112, -24]);
        Assert.Throws<InvalidDataException>(() => ScenarioEventCompiler.Inject(image, [Event()], 0));
    }

    [Fact]
    public void Unreachable_function_loop_is_not_used_as_the_main_hook()
    {
        BciImage image = Fixture([121, 66, 10, 131, 112, -20]);
        byte[] before = image.Serialize();
        Assert.Throws<InvalidDataException>(() => ScenarioEventCompiler.Inject(image, [Event()], 0));
        Assert.Equal(before, image.Serialize());
    }

    [Fact]
    public void Unreachable_extra_loop_does_not_make_the_real_main_hook_ambiguous()
    {
        BciImage image = Fixture([112, 20, 66, 10, 131, 112, -20, 66, 10, 131, 128, 0, 112, -28]);
        byte[] before = image.Code.ToArray();
        ScenarioEventCompiler.Inject(image, [Event()], 0);
        Assert.Equal(before.AsSpan(0, 28).ToArray(), image.Code.AsSpan(0, 28).ToArray());
        Assert.Equal(112, BitConverter.ToInt32(image.Code, 28));
        var vm = new TestVm(image); vm.Tick(0); vm.Tick(2000);
        Assert.Single(vm.Messages); Assert.Equal(1, vm.OriginalTicks); Assert.Equal(0, vm.StackDepth);
    }

    [Fact]
    public void Called_function_wait_is_not_borrowed_when_main_has_no_wait_loop()
    {
        BciImage image = Fixture([120, 4, 121, 66, 10, 131, 112, -20]);
        byte[] before = image.Serialize();
        Assert.Throws<InvalidDataException>(() => ScenarioEventCompiler.Inject(image, [Event()], 0));
        Assert.Equal(before, image.Serialize());
    }

    [Fact]
    public void Apply_is_idempotent_preserves_pfil_header_and_restores_when_events_removed()
    {
        string scriptDir = Path.Combine(_root, "SCRIPT"); Directory.CreateDirectory(scriptDir);
        string script = Path.Combine(scriptDir, LevelScriptInjector.ScriptFile);
        byte[] header = new byte[64]; "PFIL"u8.CopyTo(header); header[40] = 123;
        byte[] original = GameLZSS.CompressPfil(Fixture().Serialize(), header); File.WriteAllBytes(script, original);
        var document = new ScenarioDocument { Events = [Event()] };
        using (var rollback = new FileRollbackScope()) { document.Save(_root, rollback); LevelScriptInjector.Apply(_root, document, Aliases, rollback); rollback.Commit(); }
        byte[] first = File.ReadAllBytes(script); Assert.Equal(123, first[40]);
        Assert.Equal(original, File.ReadAllBytes(Path.Combine(scriptDir, LevelScriptInjector.OriginalBackupFile)));
        using (var rollback = new FileRollbackScope()) { LevelScriptInjector.Apply(_root, document, Aliases, rollback); rollback.Commit(); }
        Assert.Equal(first, File.ReadAllBytes(script));
        document.Events.Clear();
        using (var rollback = new FileRollbackScope()) { LevelScriptInjector.Apply(_root, document, Aliases, rollback); rollback.Commit(); }
        Assert.Equal(original, File.ReadAllBytes(script));
    }

    [Fact]
    public void Failed_hook_rolls_back_json_script_and_new_original_backup()
    {
        string scriptDir = Path.Combine(_root, "SCRIPT"); Directory.CreateDirectory(scriptDir);
        string script = Path.Combine(scriptDir, LevelScriptInjector.ScriptFile);
        byte[] original = Fixture([66, 10, 131, 123]).Serialize(); File.WriteAllBytes(script, original);
        var old = new ScenarioDocument { Spawns = [new("HOUSE", 4000, 5000, 0, Prebuilt: true)], DataSlots = [new(42, 123)] };
        using (var rollback = new FileRollbackScope()) { old.Save(_root, rollback); rollback.Commit(); }
        byte[] originalJson = File.ReadAllBytes(Path.Combine(_root, ScenarioDocument.FileName));
        old.Events.Add(Event());
        using (var rollback = new FileRollbackScope())
        {
            old.Save(_root, rollback);
            Assert.Throws<InvalidDataException>(() => LevelScriptInjector.Apply(_root, old, Aliases, rollback));
        }
        Assert.Equal(originalJson, File.ReadAllBytes(Path.Combine(_root, ScenarioDocument.FileName)));
        Assert.Equal(original, File.ReadAllBytes(script));
        Assert.False(File.Exists(Path.Combine(scriptDir, LevelScriptInjector.OriginalBackupFile)));
    }

    [Fact]
    public void Legacy_documents_keep_placements_and_ownership_when_events_are_saved()
    {
        Directory.CreateDirectory(_root);
        File.WriteAllText(Path.Combine(_root, ScenarioDocument.FileName), "{\"Version\":2,\"Spawns\":[{\"Alias\":\"HOUSE\",\"X\":4000,\"Z\":5000,\"Team\":0,\"Prebuilt\":true}],\"DataSlots\":[{\"Slot\":42,\"Uid\":123}]}");
        ScenarioDocument document = ScenarioDocument.Load(_root); Assert.Empty(document.Events);
        document.Events.Add(Event());
        using (var rollback = new FileRollbackScope()) { document.Save(_root, rollback); rollback.Commit(); }
        ScenarioDocument loaded = ScenarioDocument.Load(_root);
        Assert.Equal(5, loaded.Version);
        Assert.Equal(document.Spawns, loaded.Spawns); Assert.Equal(document.DataSlots, loaded.DataSlots);
        Assert.Equal("Timer", Assert.Single(loaded.Events).Name);
    }

    [Theory]
    [InlineData("null")]
    [InlineData("{\"Version\":99}")]
    [InlineData("{\"Version\":3,\"Events\":null}")]
    public void Invalid_document_is_rejected(string json)
    {
        Directory.CreateDirectory(_root); File.WriteAllText(Path.Combine(_root, ScenarioDocument.FileName), json);
        Assert.Throws<InvalidDataException>(() => ScenarioDocument.Load(_root));
    }

    [Fact]
    public void Invalid_timers_actions_and_unrepresentable_messages_are_rejected()
    {
        Assert.Throws<InvalidDataException>(() => ScenarioEventValidator.Validate([Event(0, true)], Aliases));
        Assert.Throws<InvalidDataException>(() => ScenarioEventValidator.Validate([Event(-1)], Aliases));
        Assert.Throws<InvalidDataException>(() => ScenarioEventValidator.Validate([new("Empty")], Aliases));
        Assert.Throws<EncoderFallbackException>(() => ScenarioEventValidator.Validate([Event() with { Actions = [new(ScenarioActionKind.Message, "中文")] }], Aliases));
        Assert.Throws<InvalidDataException>(() => ScenarioEventValidator.Validate([Event() with { Actions = [new(ScenarioActionKind.SpawnUnit, Alias: "BAD")] }], Aliases));
        Assert.Throws<InvalidDataException>(() => ScenarioEventValidator.Validate([Event() with { Actions = [new(ScenarioActionKind.SpawnUnit, Alias: "GER_INF01", X: float.NaN)] }], Aliases));
        Assert.Throws<InvalidDataException>(() => ScenarioEventValidator.Validate([Event() with { Actions = [new(ScenarioActionKind.Diplomacy, Team: 0, OtherTeam: 0)] }], Aliases));
    }

    internal static BciImage Fixture(int[]? words = null)
    {
        words ??= [74, 94, 73, 0, 66, 10, 131, 128, 0, 112, -28];
        using var stream = new MemoryStream(); using var writer = new BinaryWriter(stream);
        byte[] constant = Encoding.ASCII.GetBytes("s_originalTick\0");
        writer.Write("BCI0"u8); foreach (int value in new[] { 1, words.Length * 4, 0, constant.Length, 1, 0, 0 }) writer.Write(value);
        writer.Write("CODE"u8); foreach (int word in words) writer.Write(word);
        writer.Write("SYMB"u8); writer.Write("CONS"u8); writer.Write(constant); writer.Write("CIDX"u8); writer.Write(0);
        writer.Write("VAR "u8); writer.Write("VIDX"u8); writer.Write(0); writer.Write(0);
        return BciImage.Parse(stream.ToArray());
    }

    private sealed class TestVm(BciImage image)
    {
        private readonly List<object> _stack = new();
        private int _pc = image.MainAddress, _fp, _now, _result;
        public Dictionary<string, int> Variables { get; } = new();
        public List<string> Messages { get; } = new();
        public List<int> Waits { get; } = new();
        public List<(int, int, int)> Diplomacy { get; } = new();
        public List<(int, int, int, string, int)> Spawns { get; } = new();
        public Queue<(int Result, int Index, int Uid, bool Writes)> CreationResults { get; } = new();
        public List<string> Creations { get; } = new();
        public List<(int, int)> CreationInputs { get; } = new();
        public Dictionary<int, (int Uid, bool Dead)> Objects { get; } = new();
        public int OriginalTicks { get; private set; }
        public int StackDepth => _stack.Count;
        private object Pop() { object value = _stack[^1]; _stack.RemoveAt(_stack.Count - 1); return value; }
        private int Int() => (int)Pop();
        private int Word() { int value = BinaryPrimitives.ReadInt32LittleEndian(image.Code.AsSpan(_pc)); _pc += 4; return value; }
        private T Arg<T>(int index) => (T)_stack[_stack.Count - 1 - index];

        public void Tick(int time)
        {
            _now = time;
            for (int steps = 0; steps < 10000; steps++)
            {
                int op = Word();
                switch (op)
                {
                    case 66: _stack.Add(Word()); break;
                    case 76:
                        int index = Word(), start = image.ConstOffsets[index], end = Array.IndexOf(image.ConstBlob, (byte)0, start);
                        _stack.Add(MapTextEncoding.Game.GetString(image.ConstBlob, start, end - start)); break;
                    case 78: _stack.Add(_fp + Word()); break;
                    case 91: int local = Word(); _stack[_fp + local] = Pop(); break;
                    case 90: _stack.Add(_stack[_fp + Word()]); break;
                    case 74: _stack.Add(_fp); break;
                    case 94: _fp = _stack.Count; break;
                    case 95: _stack.RemoveRange(_fp, _stack.Count - _fp); break;
                    case 75: _fp = Int(); break;
                    case 73:
                        int count = Word(); if (count >= 0) for (int i = 0; i < count; i++) _stack.Add(0);
                        else _stack.RemoveRange(_stack.Count + count, -count); break;
                    case 128: Native(image.Constant(Word())); break;
                    case 86: _stack.Add(_result); break;
                    case 16: int angle = Int(); _stack.Add(angle); _stack.Add(0); break;
                    case 32: int add = Int(); _stack.Add(unchecked(Int() + add)); break;
                    case 96: int right = Int(); _stack.Add(Int().CompareTo(right)); break;
                    case 101: _stack.Add(Int() >= 0 ? 1 : 0); break;
                    case 112: int jump = Word(); _pc += jump; break;
                    case 113: int negative = Word(); if (Int() < 0) _pc += negative; break;
                    case 117: int zero = Word(); if (Int() == 0) _pc += zero; break;
                    case 118: int nonzero = Word(); if (Int() != 0) _pc += nonzero; break;
                    case 131: Waits.Add(Int()); return;
                    default: throw new InvalidOperationException($"Unsupported test VM opcode {op}");
                }
            }
            throw new InvalidOperationException("VM did not return to original wait.");
        }

        private void Native(string name)
        {
            _result = 0;
            switch (name)
            {
                case "s_getTime": _result = _now; break;
                case "s_setScriptVarL": Variables[Arg<string>(0)] = Arg<int>(1); break;
                case "s_getScriptVarL": _result = Variables.GetValueOrDefault(Arg<string>(0)); break;
                case "s_objExists": _result = Objects.TryGetValue(Arg<int>(0), out var exists) && exists.Uid == Arg<int>(1) ? 1 : 0; break;
                case "s_objDead": _result = Objects.TryGetValue(Arg<int>(0), out var dead) && dead.Uid == Arg<int>(1) && dead.Dead ? 1 : 0; break;
                case "s_showTextBox": Assert.Equal(0, Arg<int>(0)); Messages.Add(Arg<string>(1)); break;
                case "s_setTeamHostile": Diplomacy.Add((Arg<int>(0), Arg<int>(1), Arg<int>(2))); break;
                case "s_createUnitAndMems":
                    Assert.Equal(1, Arg<int>(3)); Assert.Equal(0, Arg<int>(4)); Assert.Equal(1, Arg<int>(5));
                    Assert.Equal(100, Arg<int>(13)); Assert.Equal(100, Arg<int>(14));
                    Spawns.Add((Arg<int>(2), Arg<int>(8), Arg<int>(9), Arg<string>(10), Arg<int>(11)));
                    Created(name); break;
                case "s_createObj": Created(name); break;
                case "s_originalTick": OriginalTicks++; break;
                default: throw new InvalidOperationException($"Unexpected native {name}");
            }
        }

        private void Created(string name)
        {
            Creations.Add(name);
            int indexOutput = Arg<int>(0), uidOutput = Arg<int>(1);
            CreationInputs.Add(((int)_stack[indexOutput], (int)_stack[uidOutput]));
            if (CreationResults.Count == 0) return;
            var creation = CreationResults.Dequeue(); _result = creation.Result;
            if (creation.Writes) { _stack[indexOutput] = creation.Index; _stack[uidOutput] = creation.Uid; }
        }
    }

    public void Dispose() { if (Directory.Exists(_root)) Directory.Delete(_root, true); }
}

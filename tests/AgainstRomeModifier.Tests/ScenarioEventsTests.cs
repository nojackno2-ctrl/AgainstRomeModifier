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
        BciImage image = Fixture(); int originalMain = image.MainAddress;
        LevelScriptInjector.Inject(image, [new("GER_INF01", 4000, 5000, 0, Count: 3)]);
        var item = new ScenarioEvent("Actions", 0)
        {
            Actions = [new(ScenarioActionKind.Diplomacy, Team: 0, OtherTeam: 2, Hostile: false),
                new(ScenarioActionKind.SpawnUnit, Team: 3, Alias: "GER_INF01", X: 1000, Z: 2000, Count: 7),
                new(ScenarioActionKind.Message, "Ready")]
        };
        ScenarioEventValidator.Validate([item], Aliases);
        ScenarioEventCompiler.Inject(image, [item], originalMain);
        var vm = new TestVm(image); vm.Tick(0);
        Assert.Equal(2, vm.Spawns.Count);
        Assert.Equal((0, 4000, 5000, "GER_INF01", 3), vm.Spawns[0]);
        Assert.Equal((3, 1000, 2000, "GER_INF01", 7), vm.Spawns[1]);
        Assert.Equal((0, 2, 0), Assert.Single(vm.Diplomacy));
        Assert.Equal("Ready", Assert.Single(vm.Messages));
        Assert.Equal(1, vm.StackDepth);
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
            new[] { 66, 10, 131, 112, -20, 66, 10, 131, 112, -20 }, new[] { 66, 10, 131, 112, -16 } })
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
        Assert.Equal(3, loaded.Version);
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
                case "s_showTextBox": Assert.Equal(0, Arg<int>(0)); Messages.Add(Arg<string>(1)); break;
                case "s_setTeamHostile": Diplomacy.Add((Arg<int>(0), Arg<int>(1), Arg<int>(2))); break;
                case "s_createUnitAndMems":
                    Assert.Equal(1, Arg<int>(3)); Assert.Equal(0, Arg<int>(4)); Assert.Equal(1, Arg<int>(5));
                    Assert.Equal(100, Arg<int>(13)); Assert.Equal(100, Arg<int>(14));
                    Spawns.Add((Arg<int>(2), Arg<int>(8), Arg<int>(9), Arg<string>(10), Arg<int>(11))); break;
                case "s_originalTick": OriginalTicks++; break;
                default: throw new InvalidOperationException($"Unexpected native {name}");
            }
        }
    }

    public void Dispose() { if (Directory.Exists(_root)) Directory.Delete(_root, true); }
}

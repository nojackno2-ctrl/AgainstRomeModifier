using System.Buffers.Binary;
using AgainstRomeMapEditor.Modules.AI;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

public sealed partial class ScenarioEventsTests
{
    // Original ENDL_000 code 0xB374..0xB3D4, source SHA256 in wave-orders-re.md.
    private const string OriginalMoveCall = "4200000002000000420000000000000042000000000000005A000000FBFFFFFF5A000000FCFFFFFF42000000080000005A000000040000005A0000000300000051000000060000005100000005000000800000005E00000049000000F6FFFFFF";

    [Fact]
    public void Wave_move_call_bytes_match_original_script_with_relocated_coordinates_and_identities()
    {
        var image = Fixture(); var action = MoveSpawn(3210, 4567);
        ScenarioEventCompiler.Inject(image, [Event(0) with { Actions = [action] }], 0);
        byte[] expected = Convert.FromHexString(OriginalMoveCall);
        void Replace(int word, int value) => BinaryPrimitives.WriteInt32LittleEndian(expected.AsSpan(word * 4), value);
        Replace(6, 66); Replace(7, 4567); Replace(8, 66); Replace(9, 3210);
        // Original recipient locals 4/3 become creation outputs 1/0. Sender is this same group.
        Replace(13, 1); Replace(15, 0); Replace(16, 90); Replace(17, 1); Replace(18, 90); Replace(19, 0);
        int symbol = image.ConstOffsets.Select((_, i) => i).Single(i => image.Constant(i) == "s_sendMsg");
        Replace(21, symbol);
        Assert.Equal(1, Enumerable.Range(0, image.Code.Length - expected.Length + 1)
            .Count(i => image.Code.AsSpan(i, expected.Length).SequenceEqual(expected)));
    }

    [Theory]
    [InlineData(1, true, 43, 123, 1)]
    [InlineData(0, false, 0, 0, 0)]
    [InlineData(-1, true, 43, 123, 0)]
    public void Wave_orders_only_the_successfully_created_group(int result, bool writes, int index, int uid, int orders)
    {
        var image = Fixture(); ScenarioEventCompiler.Inject(image, [Event(0) with { Actions = [MoveSpawn(100, 200)] }], 0);
        var vm = new TestVm(image); vm.CreationResults.Enqueue((result, index, uid, writes)); vm.Tick(0);
        Assert.Equal(orders, vm.GroupOrders.Count);
        if (orders != 0) Assert.Equal(new[] { index, uid, index, uid, 8, 100, 200, 0, 0, 2 }, vm.GroupOrders[0]);
        Assert.Equal(1, vm.StackDepth); vm.Tick(100); Assert.Equal(orders, vm.GroupOrders.Count);
    }

    [Fact]
    public void Failed_second_spawn_does_not_reuse_first_groups_outputs_and_conditions_do_not_overwrite_order_identity()
    {
        var id = Guid.NewGuid(); var image = Fixture();
        ScenarioEventCompiler.Inject(image, [Event(0) with { Actions = [MoveSpawn(100, 200), MoveSpawn(300, 400)],
            Conditions = [new(ScenarioConditionKind.ObjectInArea, id)] }], 0, BuildingTargets(id));
        var vm = new TestVm(image); vm.Objects[43] = (123, false); vm.Positions[43] = (600, 700);
        vm.CreationResults.Enqueue((1, 80, 900, true)); vm.CreationResults.Enqueue((0, 0, 0, false)); vm.Tick(0);
        Assert.Single(vm.GroupOrders); Assert.Equal(new[] { 80, 900, 80, 900, 8, 100, 200, 0, 0, 2 }, vm.GroupOrders[0]);
        Assert.All(vm.CreationInputs, inputs => Assert.Equal((0, 0), inputs)); Assert.Equal(1, vm.StackDepth);
    }

    [Theory]
    [InlineData(-1, 0)]
    [InlineData(0, 16384)]
    public void Invalid_move_destinations_are_rejected_before_injection(int x, int z)
    {
        var image = Fixture(); byte[] before = image.Serialize(); var item = Event() with { Actions = [MoveSpawn(x, z)] };
        Assert.Throws<InvalidDataException>(() => ScenarioEventValidator.Validate([item], Aliases));
        Assert.Throws<InvalidDataException>(() => ScenarioEventCompiler.Inject(image, [item], 0));
        Assert.Equal(before, image.Serialize());
    }

    [Fact]
    public void Move_destination_survives_scenario_save_load_and_campaign_lowering()
    {
        var wave = new WaveAttackDefinition(1, 0, "", 8000, 8000, [new("GER_INF01", 10, 1)], MoveDestination: new(0, 16383));
        var plan = new CampaignMissionPlan("Orders", "", [], [wave], [], []);
        var compiled = CampaignMissionCompiler.Compile(plan, null, Aliases);
        Assert.True(compiled.Success); var doc = new ScenarioDocument { Events = compiled.CompiledEvents.ToList() };
        Directory.CreateDirectory(_root);
        using (var rollback = new FileRollbackScope())
        { doc.Save(_root, rollback); rollback.Commit(); }
        Assert.Equal(wave.MoveDestination, ScenarioDocument.Load(_root)!.Events[0].Actions[0].MoveDestination);
        Assert.False(CampaignMissionCompiler.Compile(plan with { Waves = [wave with { TargetObjectId = Guid.NewGuid() }] }, null, Aliases).Success);
        Assert.False(CampaignMissionCompiler.Compile(plan with { Waves = [wave with { AssignedPathId = Guid.NewGuid() }] }, null, Aliases).Success);
        Assert.False(CampaignMissionCompiler.Compile(plan with { Waves = [wave with { Squads = [new("GER_INF01", 10, 1, 1)] }] }, null, Aliases).Success);
    }

    private static ScenarioAction MoveSpawn(int x, int z) => new(ScenarioActionKind.SpawnUnit, Alias: "GER_INF01", Team: 1)
        { MoveDestination = new(x, z) };

    [Fact]
    public void Orders_repeat_with_fresh_identity_and_non_spawn_orders_are_rejected()
    {
        var image = Fixture(); ScenarioEventCompiler.Inject(image, [Event(1, true) with { Actions = [MoveSpawn(0, 16383)] }], 0);
        var vm = new TestVm(image); vm.Tick(0);
        vm.CreationResults.Enqueue((1, 10, 20, true)); vm.Tick(1000);
        vm.CreationResults.Enqueue((1, 11, 21, true)); vm.Tick(2000);
        Assert.Equal(new[] { 10, 20, 10, 20, 8, 0, 16383, 0, 0, 2 }, vm.GroupOrders[0]);
        Assert.Equal(new[] { 11, 21, 11, 21, 8, 0, 16383, 0, 0, 2 }, vm.GroupOrders[1]);
        Assert.Equal(1, vm.StackDepth);
        Assert.Throws<InvalidDataException>(() => ScenarioEventValidator.Validate(
            [Event() with { Actions = [new(ScenarioActionKind.Message, "Hello") { MoveDestination = new(0, 0) }] }], Aliases));
    }
}

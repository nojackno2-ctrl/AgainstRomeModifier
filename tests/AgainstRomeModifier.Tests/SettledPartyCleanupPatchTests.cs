namespace AgainstRomeModifier.Tests;

public sealed class SettledPartyCleanupPatchTests
{
    private const int DeleteParty = 256;
    private const int DeleteTeam = 257;

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Apply_migrates_legacy_delete_team_to_safe_delete_party(bool enabled)
    {
        var patch = new P15_SettledPartyDeleteTeamPatch();
        byte[] script = CreateScript(DeleteTeam, DeleteTeam);

        Assert.Equal(PatchState.Legacy, patch.Detect(script));
        Assert.True(patch.Apply(ref script, enabled));
        Assert.Equal(PatchState.Original, patch.Detect(script));
        Assert.Equal(DeleteParty, ReadTerminalState(script, 0));
        Assert.Equal(DeleteParty, ReadTerminalState(script, 1));
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Apply_is_noop_for_safe_original_state(bool enabled)
    {
        var patch = new P15_SettledPartyDeleteTeamPatch();
        byte[] script = CreateScript(DeleteParty, DeleteParty);

        Assert.Equal(PatchState.Original, patch.Detect(script));
        Assert.False(patch.Apply(ref script, enabled));
        Assert.Equal(PatchState.Original, patch.Detect(script));
    }

    [Fact]
    public void Detect_reports_partially_migrated_state_as_legacy()
    {
        var patch = new P15_SettledPartyDeleteTeamPatch();
        byte[] script = CreateScript(DeleteParty, DeleteTeam);

        Assert.Equal(PatchState.Legacy, patch.Detect(script));
    }

    [Fact]
    public void Orchestrator_keeps_P15_out_of_M3_and_runs_it_as_mandatory_repair()
    {
        var orchestrator = new EndlessAiOrchestrator();

        Assert.DoesNotContain(orchestrator.M3.Patches, patch => patch.Id == "P15");
        Assert.Contains(orchestrator.R0.Patches, patch => patch.Id == "P15");
    }

    private static byte[] CreateScript(int firstState, int secondState)
    {
        int[] first = CreateTerminalSequence(firstState, 7);
        int[] second = CreateTerminalSequence(secondState, 6);
        byte[] bytes = new byte[(first.Length + second.Length + 1) * sizeof(int)];
        WriteWords(bytes, 0, first);
        WriteWords(bytes, (first.Length + 1) * sizeof(int), second);
        return bytes;
    }

    private static int[] CreateTerminalSequence(int state, int stateLocal) =>
    [
        71, 66, 0, 117, 16, 66, state, 91, stateLocal, 112, 0,
        66, DeleteParty, 90, 14, 96, 118
    ];

    private static void WriteWords(byte[] destination, int offset, int[] words)
    {
        foreach (int word in words)
        {
            BitConverter.GetBytes(word).CopyTo(destination, offset);
            offset += sizeof(int);
        }
    }

    private static int ReadTerminalState(byte[] script, int siteIndex)
    {
        int sequenceWords = 17;
        int sequenceStart = siteIndex * (sequenceWords + 1) * sizeof(int);
        return BitConverter.ToInt32(script, sequenceStart + 6 * sizeof(int));
    }
}

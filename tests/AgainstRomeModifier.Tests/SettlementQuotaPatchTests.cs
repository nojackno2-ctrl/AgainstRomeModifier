namespace AgainstRomeModifier.Tests;

public sealed class SettlementQuotaPatchTests
{
    private static IEndlessPatch Patch => new EndlessAiOrchestrator().M6.Patches.Single();

    [Fact]
    public void P16_applies_balanced_three_plus_one_quota_and_restores_vanilla_range()
    {
        byte[] data = Words(66, 4, 66, 2, 128, 16, 73, -2, 86, 82, 70);
        IEndlessPatch patch = Patch;

        Assert.Equal(PatchState.Original, patch.Detect(data));
        Assert.True(patch.Apply(ref data, true));
        Assert.Equal(PatchState.Ultimate, patch.Detect(data));
        Assert.Equal(3, BitConverter.ToInt32(data, 4));
        Assert.Equal(3, BitConverter.ToInt32(data, 12));

        Assert.True(patch.Apply(ref data, false));
        Assert.Equal(PatchState.Original, patch.Detect(data));
        Assert.Equal(4, BitConverter.ToInt32(data, 4));
        Assert.Equal(2, BitConverter.ToInt32(data, 12));
    }

    [Fact]
    public void P16_migrates_legacy_four_type_one_quota()
    {
        byte[] data = Words(66, 4, 66, 4, 128, 16, 73, -2, 86, 82, 70);
        IEndlessPatch patch = Patch;

        Assert.Equal(PatchState.Legacy, patch.Detect(data));
        Assert.True(patch.Apply(ref data, true));
        Assert.Equal(PatchState.Ultimate, patch.Detect(data));
        Assert.Equal(3, BitConverter.ToInt32(data, 4));
        Assert.Equal(3, BitConverter.ToInt32(data, 12));
    }

    private static byte[] Words(params int[] words)
    {
        byte[] data = new byte[words.Length * 4];
        for (int i = 0; i < words.Length; i++)
            BitConverter.GetBytes(words[i]).CopyTo(data, i * 4);
        return data;
    }
}

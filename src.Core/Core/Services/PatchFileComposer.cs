using AgainstRomeModifier.Core.Features;

namespace AgainstRomeModifier.Core.Services;

internal sealed record PatchFilePlan(IReadOnlyDictionary<string, byte[]> Files);

internal sealed class PatchFileComposer
{
    private readonly IReadOnlyList<IPatchFileContributor> _contributors;
    private readonly ILogger _logger;

    internal PatchFileComposer(ILogger logger)
        : this(logger, PatchFileContributors.CreateDefault()) { }

    internal PatchFileComposer(ILogger logger, IReadOnlyList<IPatchFileContributor> contributors)
    {
        _logger = logger;
        _contributors = contributors;
    }

    internal PatchFilePlan Compose(string gamePath, BackupManager backupManager, PatchProfile profile)
    {
        var context = new PatchFileCompositionContext(gamePath, backupManager, profile, _logger);
        foreach (IPatchFileContributor contributor in _contributors)
            contributor.Contribute(context);
        return new PatchFilePlan(context.Files);
    }
}

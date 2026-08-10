using System.Threading.Tasks;

namespace AgainstRomeModifier.Core.Services;

internal sealed class PatchOperationRunner
{
    private readonly Action<string> _log;

    internal PatchOperationRunner(Action<string> log) => _log = log;

    internal async Task ExecuteAsync(
        Action<FileRollbackScope> operation,
        string checkpointCreatedMessage,
        string rollbackStartedMessage,
        string rollbackCompletedMessage)
    {
        using var rollback = new FileRollbackScope();
        _log(checkpointCreatedMessage);
        try
        {
            await Task.Run(() => operation(rollback));
            rollback.Commit();
        }
        catch
        {
            if (!rollback.IsCommitted)
            {
                _log(rollbackStartedMessage);
                rollback.RestoreAll(_log);
                _log(rollbackCompletedMessage);
            }
            throw;
        }
    }
}

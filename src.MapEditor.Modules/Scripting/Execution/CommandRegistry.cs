using System.Collections.Concurrent;

namespace AgainstRomeMapEditor.Modules.Scripting.Execution;

/// <summary>
/// 指令登錄表：管理所有註冊的 DSL 編輯指令。
/// </summary>
internal sealed class CommandRegistry
{
    private readonly ConcurrentDictionary<string, IEditorCommand> _commands = new(StringComparer.OrdinalIgnoreCase);
    private readonly List<IEditorCommand> _uniqueCommands = new();

    public int Count => _uniqueCommands.Count;

    internal void Register(IEditorCommand command)
    {
        ArgumentNullException.ThrowIfNull(command);

        _commands[command.Name] = command;
        foreach (var alias in command.Aliases)
        {
            _commands[alias] = command;
        }

        if (!_uniqueCommands.Contains(command))
        {
            _uniqueCommands.Add(command);
        }
    }

    internal void RegisterRange(IEnumerable<IEditorCommand> commands)
    {
        ArgumentNullException.ThrowIfNull(commands);
        foreach (var cmd in commands)
        {
            Register(cmd);
        }
    }

    internal bool TryGetCommand(string nameOrAlias, out IEditorCommand? command)
    {
        if (string.IsNullOrWhiteSpace(nameOrAlias))
        {
            command = null;
            return false;
        }

        string clean = nameOrAlias.StartsWith('/') ? nameOrAlias[1..] : nameOrAlias;
        return _commands.TryGetValue(clean, out command);
    }

    internal IReadOnlyList<IEditorCommand> GetAllCommands() =>
        _uniqueCommands.ToArray();

    /// <summary>
    /// 建立包含所有內建指令的預設登錄表。
    /// </summary>
    public static CommandRegistry CreateDefault()
    {
        var registry = new CommandRegistry();
        registry.Register(new Commands.ElevateCommand());
        registry.Register(new Commands.FlattenCommand());
        registry.Register(new Commands.ReplaceTextureCommand());
        registry.Register(new Commands.ScatterCommand());
        registry.Register(new Commands.SpawnRingCommand());
        registry.Register(new Commands.AlignGridCommand());
        registry.Register(new Commands.SelectTeamCommand());
        registry.Register(new Commands.SetTeamCommand());
        registry.Register(new Commands.HealNavMeshCommand());
        registry.Register(new Commands.DiagnoseCommand());
        registry.Register(new Commands.HelpCommand(registry));
        registry.Register(new Commands.EchoCommand());
        registry.Register(new Commands.RunMacroCommand());
        registry.Register(new Commands.UndoCommand());
        registry.Register(new Commands.RedoCommand());
        registry.Register(new Commands.ClearCommand());
        return registry;
    }
}

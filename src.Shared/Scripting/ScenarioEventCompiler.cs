using System.Buffers.Binary;

namespace AgainstRomeModifier.Scripting;

/// <summary>在原 main 的已辨識等待點加入事件輪詢；不搬動原程式碼或阻塞原邏輯。</summary>
public static class ScenarioEventCompiler
{
    private static readonly HashSet<int> OneOperand = [64, 65, 66, 68, 69, 70, 73, 76, 77, 78, 80, 81, 82, 83, 84,
        90, 91, 92, 93, 112, 113, 114, 115, 116, 117, 118, 120, 128, 129, 160];
    private static readonly HashSet<int> NoOperand = [1, 2, 3, 4, 5, 6, 16, 17, 32, 33, 34, 35, 36, 37, 38, 39,
        40, 41, 42, 43, 44, 45, 48, 49, 50, 51, 52, 53, 71, 72, 74, 75, 85, 86, 87, 88, 89,
        94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 121, 122, 123, 130, 131, 144, 145, 161, 162, 163, 164, 165, 166, 176, 177];

    public static void Inject(BciImage image, IReadOnlyList<ScenarioEvent> events, int originalMain)
    {
        ScenarioEvent[] active = events.Where(item => item.Enabled).ToArray();
        if (active.Length == 0) return;
        int hook = FindWaitHook(image.Code, originalMain);
        int waitPush = BinaryPrimitives.ReadInt32LittleEndian(image.Code.AsSpan(hook));
        int waitOperand = BinaryPrimitives.ReadInt32LittleEndian(image.Code.AsSpan(hook + 4));
        int continuation = image.MainAddress;
        var constants = new Dictionary<string, int>(StringComparer.Ordinal);
        int Constant(string value) => constants.TryGetValue(value, out int index) ? index : constants[value] = image.AddConstant(value);
        string Key(int index) => $"ARM_EVENT_DEADLINE_{index}";
        void Call(CodeBuilder b, string name, int arguments, bool result = false)
        {
            b.Emit(128, Constant(name)); if (arguments > 0) b.Emit(73, -arguments); if (result) b.Emit(86);
        }
        void Now(CodeBuilder b) => Call(b, "s_getTime", 0, true);
        void Deadline(CodeBuilder b, int index) { b.Emit(76, Constant(Key(index))); Call(b, "s_getScriptVarL", 1, true); }
        void Store(CodeBuilder b, int index) { b.Emit(76, Constant(Key(index))); Call(b, "s_setScriptVarL", 2); }

        // 開局只初始化事件狀態，然後接回放置 shim（若有）或原 main。
        var entry = new CodeBuilder(image.Code.Length);
        for (int i = 0; i < active.Length; i++)
        {
            Now(entry); entry.Emit(66, checked(active[i].DelaySeconds * 1000)); entry.Emit(32); Store(entry, i);
        }
        entry.Jump(112, continuation);
        int entryAddress = image.AppendCode(entry.Bytes());

        var poll = new CodeBuilder(image.Code.Length);
        poll.Emit(74); poll.Emit(94); poll.Emit(73, 2); // 專屬輸出參數框架，不使用原 main locals。
        for (int i = 0; i < active.Length; i++)
        {
            ScenarioEvent item = active[i];
            Deadline(poll, i); int done = poll.Placeholder(113); // -1 代表單次事件已完成。
            Now(poll); Deadline(poll, i); poll.Emit(96); poll.Emit(101); int early = poll.Placeholder(117);
            if (item.Repeat) { Now(poll); poll.Emit(66, checked(item.DelaySeconds * 1000)); poll.Emit(32); }
            else poll.Emit(66, -1);
            Store(poll, i); // 在動作之前更新，避免重入時重複觸發。
            foreach (ScenarioAction action in item.Actions)
            {
                switch (action.Kind)
                {
                    case ScenarioActionKind.Message:
                        poll.Emit(76, image.AddGameConstant(action.Text)); poll.Emit(66, 0); Call(poll, "s_showTextBox", 2);
                        break;
                    case ScenarioActionKind.Diplomacy:
                        poll.Emit(66, action.Hostile ? 1 : 0); poll.Emit(66, action.OtherTeam); poll.Emit(66, action.Team);
                        Call(poll, "s_setTeamHostile", 3); break;
                    case ScenarioActionKind.SpawnUnit:
                        poll.Emit(66, 100); poll.Emit(66, 100); poll.Emit(66, 0); poll.Emit(66, action.Count);
                        poll.Emit(76, Constant(action.Alias)); poll.Emit(66, (int)MathF.Round(action.Z)); poll.Emit(66, (int)MathF.Round(action.X));
                        poll.Emit(66, 0); poll.Emit(16); poll.Emit(66, 1); poll.Emit(66, 0); poll.Emit(66, 1); poll.Emit(66, action.Team);
                        poll.Emit(78, 1); poll.Emit(78, 0); Call(poll, "s_createUnitAndMems", 15); break;
                    default: throw new InvalidDataException("不支援的事件動作。");
                }
            }
            poll.Resolve(done, poll.Address); poll.Resolve(early, poll.Address);
        }
        // 恢復原框架之後重播原等待參數；原版無盡模式用 local 20 計算等待值。
        poll.Emit(95); poll.Emit(75); poll.Emit(waitPush, waitOperand); poll.Jump(112, hook + 8);
        int pollAddress = image.AppendCode(poll.Bytes());
        BinaryPrimitives.WriteInt32LittleEndian(image.Code.AsSpan(hook), 112);
        BinaryPrimitives.WriteInt32LittleEndian(image.Code.AsSpan(hook + 4), pollAddress - (hook + 8));
        image.MainAddress = entryAddress;
    }

    private static int FindWaitHook(byte[] code, int main)
    {
        if (main < 0 || main % 4 != 0 || main >= code.Length) throw new InvalidDataException("事件的原 main 進入點無效。");
        var instructions = new List<(int Address, int Op, int Operand)>();
        for (int offset = main; offset < code.Length;)
        {
            int op = BinaryPrimitives.ReadInt32LittleEndian(code.AsSpan(offset));
            int words = op == 67 ? 3 : OneOperand.Contains(op) ? 2 : NoOperand.Contains(op) ? 1 : 0;
            if (words == 0 || offset + words * 4 > code.Length) throw new InvalidDataException($"無法辨識事件注入點（指令 0x{offset:x}）。");
            instructions.Add((offset, op, words > 1 ? BinaryPrimitives.ReadInt32LittleEndian(code.AsSpan(offset + 4)) : 0));
            offset += words * 4;
        }
        var boundaries = instructions.Select(item => item.Address).ToHashSet();
        foreach (var instruction in instructions.Where(item => item.Op is >= 112 and <= 118 or 120))
        {
            long target = (long)instruction.Address + 8 + instruction.Operand;
            // 分支可呼叫 main 之前的原函式；main 區域內的目標必須是指令邊界。
            if (target < 0 || target >= code.Length || target % 4 != 0 || target >= main && !boundaries.Contains((int)target))
                throw new InvalidDataException("原腳本的跳躍目標無效，無法安全注入事件。");
        }
        // main 後面可能還有其他函式。只接受 main 自身可走到的等待點，
        // internal call 只沿呼叫返回後的指令繼續，不能借用被呼叫函式的等待迴圈。
        var byAddress = instructions.ToDictionary(item => item.Address);
        var reachable = new HashSet<int>();
        var pending = new Stack<int>(); pending.Push(main);
        while (pending.TryPop(out int address))
        {
            if (!byAddress.TryGetValue(address, out var instruction) || !reachable.Add(address)) continue;
            if (instruction.Op is >= 112 and <= 118)
                pending.Push(instruction.Address + 8 + instruction.Operand);
            if (instruction.Op == 112 || instruction.Op is 121 or 122 or 123) continue;
            int words = instruction.Op == 67 ? 3 : OneOperand.Contains(instruction.Op) ? 2 : 1;
            pending.Push(instruction.Address + words * 4);
        }
        var candidates = new List<int>();
        for (int i = 0; i + 1 < instructions.Count; i++)
        {
            var instruction = instructions[i];
            if (!reachable.Contains(instruction.Address) || !(instruction.Op == 66 && instruction.Operand == 10 || instruction.Op == 90 && instruction.Operand >= 0)
                || instructions[i + 1].Op != 131) continue;
            if (instructions.Any(branch => reachable.Contains(branch.Address) && branch.Op == 112 && branch.Address > instruction.Address
                && (long)branch.Address + 8 + branch.Operand >= main
                && (long)branch.Address + 8 + branch.Operand <= instruction.Address
                && ReachesWait(branch.Address + 8 + branch.Operand, instruction.Address))) candidates.Add(instruction.Address);
        }
        if (candidates.Count != 1) throw new InvalidDataException("此地圖沒有唯一可辨識的主迴圈等待點，事件尚無法安全套用。");
        return candidates[0];

        bool ReachesWait(int start, int wait)
        {
            var visited = new HashSet<int>(); var work = new Stack<int>(); work.Push(start);
            while (work.TryPop(out int address))
            {
                if (address == wait) return true;
                if (!visited.Add(address) || !byAddress.TryGetValue(address, out var instruction)) continue;
                if (instruction.Op is >= 112 and <= 118) work.Push(address + 8 + instruction.Operand);
                if (instruction.Op == 112 || instruction.Op is 121 or 122 or 123) continue;
                int words = instruction.Op == 67 ? 3 : OneOperand.Contains(instruction.Op) ? 2 : 1;
                work.Push(address + words * 4);
            }
            return false;
        }
    }

    private sealed class CodeBuilder(int start)
    {
        private readonly List<int> _words = new();
        public int Address => start + _words.Count * 4;
        public void Emit(int op) => _words.Add(op);
        public void Emit(int op, int operand) { _words.Add(op); _words.Add(operand); }
        public void Jump(int op, int target) => Emit(op, target - (Address + 8));
        public int Placeholder(int op) { int at = _words.Count; Emit(op, 0); return at; }
        public void Resolve(int index, int target) => _words[index + 1] = target - (start + index * 4 + 8);
        public byte[] Bytes()
        {
            byte[] bytes = new byte[_words.Count * 4];
            for (int i = 0; i < _words.Count; i++) BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(i * 4), _words[i]);
            return bytes;
        }
    }
}

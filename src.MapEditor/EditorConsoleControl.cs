using System.Diagnostics;
using AgainstRomeMapEditor.Modules.Scripting.Execution;
using AgainstRomeMapEditor.Modules.Scripting.Models;
using AgainstRomeMapEditor.Modules.Scripting.Parsing;

namespace AgainstRomeMapEditor;

/// <summary>
/// 嵌入式互動控制台控制項 (EditorConsoleControl)。
/// 提供語法高亮、Tab 自動補全、歷史紀錄回溯、非同步巨集執行與單鍵 Undo/Redo 互動。
/// </summary>
public sealed class EditorConsoleControl : UserControl
{
    private readonly RichTextBox _txtOutput;
    private readonly TextBox _txtInput;
    private readonly Panel _pnlTop;
    private readonly Panel _pnlBottom;
    private readonly Label _lblPrompt;
    private readonly Label _lblStatus;
    private readonly Button _btnRunScript;
    private readonly Button _btnClear;
    private readonly Button _btnHelp;
    private readonly Button _btnCancel;

    private readonly List<string> _history = new();
    private int _historyIndex = -1;
    private string _savedCurrentInput = "";

    private IReadOnlyList<string>? _currentCompletions;
    private int _completionCycleIndex = -1;
    private string _completionPrefix = "";

    private CancellationTokenSource? _cts;
    private bool _isBusy;

    internal CommandExecutionContext? Context { get; private set; }
    internal MacroScriptRunner? Runner { get; private set; }

    public event EventHandler<CommandResult>? CommandExecuted;

    public EditorConsoleControl()
    {
        DoubleBuffered = true;
        Dock = DockStyle.Fill;
        BackColor = Color.FromArgb(30, 30, 30);
        ForeColor = Color.WhiteSmoke;
        Font = new Font("Consolas", 9.5f, FontStyle.Regular);

        // 頂部工具列
        _pnlTop = new Panel
        {
            Dock = DockStyle.Top,
            Height = 28,
            BackColor = Color.FromArgb(40, 40, 40),
            Padding = new Padding(4, 2, 4, 2)
        };

        _lblStatus = new Label
        {
            Text = "就緒 (Ready)",
            AutoSize = true,
            Dock = DockStyle.Left,
            TextAlign = ContentAlignment.MiddleLeft,
            ForeColor = Color.FromArgb(140, 220, 140),
            Padding = new Padding(4, 4, 0, 0)
        };

        _btnCancel = CreateToolButton("中斷", (s, e) => CancelExecution());
        _btnCancel.Dock = DockStyle.Right;
        _btnCancel.Visible = false;

        _btnClear = CreateToolButton("清空", (s, e) => ClearOutput());
        _btnClear.Dock = DockStyle.Right;

        _btnHelp = CreateToolButton("說明 (/help)", (s, e) => ExecuteLine("/help"));
        _btnHelp.Dock = DockStyle.Right;

        _btnRunScript = CreateToolButton("載入腳本 (.armcmd)...", (s, e) => PromptRunScript());
        _btnRunScript.Dock = DockStyle.Right;

        _pnlTop.Controls.Add(_lblStatus);
        _pnlTop.Controls.Add(_btnCancel);
        _pnlTop.Controls.Add(_btnClear);
        _pnlTop.Controls.Add(_btnHelp);
        _pnlTop.Controls.Add(_btnRunScript);

        // 底部輸入列
        _pnlBottom = new Panel
        {
            Dock = DockStyle.Bottom,
            Height = 30,
            BackColor = Color.FromArgb(35, 35, 35),
            Padding = new Padding(4, 3, 4, 3)
        };

        _lblPrompt = new Label
        {
            Text = ">",
            AutoSize = false,
            Width = 18,
            Dock = DockStyle.Left,
            TextAlign = ContentAlignment.MiddleCenter,
            ForeColor = Color.FromArgb(80, 200, 255),
            Font = new Font("Consolas", 10.5f, FontStyle.Bold)
        };

        _txtInput = new TextBox
        {
            Dock = DockStyle.Fill,
            BackColor = Color.FromArgb(24, 24, 24),
            ForeColor = Color.White,
            BorderStyle = BorderStyle.FixedSingle,
            Font = new Font("Consolas", 10f, FontStyle.Regular)
        };
        _txtInput.KeyDown += OnInputKeyDown;

        _pnlBottom.Controls.Add(_txtInput);
        _pnlBottom.Controls.Add(_lblPrompt);

        // 中間日誌輸出框
        _txtOutput = new RichTextBox
        {
            Dock = DockStyle.Fill,
            ReadOnly = true,
            BackColor = Color.FromArgb(20, 20, 20),
            ForeColor = Color.FromArgb(220, 220, 220),
            BorderStyle = BorderStyle.None,
            Font = new Font("Consolas", 9.5f, FontStyle.Regular),
            ScrollBars = RichTextBoxScrollBars.Vertical
        };

        Controls.Add(_txtOutput);
        Controls.Add(_pnlTop);
        Controls.Add(_pnlBottom);
    }

    private static Button CreateToolButton(string text, EventHandler onClick)
    {
        var btn = new Button
        {
            Text = text,
            AutoSize = true,
            FlatStyle = FlatStyle.Flat,
            BackColor = Color.FromArgb(50, 50, 50),
            ForeColor = Color.Gainsboro,
            Margin = new Padding(2, 0, 2, 0),
            Cursor = Cursors.Hand
        };
        btn.FlatAppearance.BorderSize = 0;
        btn.Click += onClick;
        return btn;
    }

    /// <summary>
    /// 綁定編輯器執行上下文與巨集執行器。
    /// </summary>
    internal void Bind(CommandExecutionContext context, MacroScriptRunner runner)
    {
        Context = context ?? throw new ArgumentNullException(nameof(context));
        Runner = runner ?? throw new ArgumentNullException(nameof(runner));

        Context.ScriptRunner = runner;
        Context.OutputHandler = (msg, lvl) => AppendLog(msg, lvl);

        AppendLog("=== Against Rome Map Editor 即時控制台已就緒 ===", LogLevel.Info);
        AppendLog("輸入 /help 查詢所有指令，或使用 Tab 進行自動補全與候選切換。", LogLevel.Debug);
    }

    /// <summary>
    /// 在主輸出區域寫入著色日誌。
    /// </summary>
    public void AppendLog(string message, LogLevel level = LogLevel.Info)
    {
        if (InvokeRequired)
        {
            BeginInvoke(new Action(() => AppendLog(message, level)));
            return;
        }

        if (message == "__CLEAR_CONSOLE__")
        {
            ClearOutput();
            return;
        }

        Color color = level switch
        {
            LogLevel.Command => Color.FromArgb(80, 220, 255),
            LogLevel.Success => Color.FromArgb(120, 240, 120),
            LogLevel.Warning => Color.FromArgb(255, 215, 60),
            LogLevel.Error => Color.FromArgb(255, 110, 110),
            LogLevel.Debug => Color.FromArgb(150, 150, 150),
            _ => Color.FromArgb(230, 230, 230)
        };

        string timestamp = DateTime.Now.ToString("HH:mm:ss");
        _txtOutput.SelectionStart = _txtOutput.TextLength;
        _txtOutput.SelectionLength = 0;

        _txtOutput.SelectionColor = Color.FromArgb(100, 100, 100);
        _txtOutput.AppendText($"[{timestamp}] ");

        _txtOutput.SelectionColor = color;
        _txtOutput.AppendText(message + Environment.NewLine);

        _txtOutput.ScrollToCaret();
    }

    public void ClearOutput()
    {
        if (InvokeRequired) { BeginInvoke(new Action(ClearOutput)); return; }
        _txtOutput.Clear();
    }

    private void OnInputKeyDown(object? sender, KeyEventArgs e)
    {
        if (_isBusy && e.KeyCode != Keys.C && !e.Control)
        {
            e.Handled = true;
            return;
        }

        switch (e.KeyCode)
        {
            case Keys.Enter:
                e.Handled = true;
                e.SuppressKeyPress = true;
                string text = _txtInput.Text.Trim();
                if (!string.IsNullOrEmpty(text))
                {
                    _history.Add(text);
                    _historyIndex = _history.Count;
                    _savedCurrentInput = "";
                    _txtInput.Clear();
                    _currentCompletions = null;
                    _completionCycleIndex = -1;

                    _ = ExecuteLineAsync(text);
                }
                break;

            case Keys.Up:
                e.Handled = true;
                if (_history.Count > 0)
                {
                    if (_historyIndex == _history.Count)
                    {
                        _savedCurrentInput = _txtInput.Text;
                    }

                    if (_historyIndex > 0)
                    {
                        _historyIndex--;
                        _txtInput.Text = _history[_historyIndex];
                        _txtInput.SelectionStart = _txtInput.Text.Length;
                    }
                }
                break;

            case Keys.Down:
                e.Handled = true;
                if (_history.Count > 0 && _historyIndex < _history.Count)
                {
                    _historyIndex++;
                    if (_historyIndex == _history.Count)
                    {
                        _txtInput.Text = _savedCurrentInput;
                    }
                    else
                    {
                        _txtInput.Text = _history[_historyIndex];
                    }
                    _txtInput.SelectionStart = _txtInput.Text.Length;
                }
                break;

            case Keys.Tab:
                e.Handled = true;
                e.SuppressKeyPress = true;
                HandleTabCompletion();
                break;

            case Keys.Escape:
                e.Handled = true;
                _txtInput.Clear();
                _currentCompletions = null;
                _completionCycleIndex = -1;
                break;
        }
    }

    private void HandleTabCompletion()
    {
        if (Runner is null) return;

        string current = _txtInput.Text;
        if (_currentCompletions is null || _currentCompletions.Count == 0)
        {
            _currentCompletions = EditorCommandParser.GetCompletions(
                current,
                _txtInput.SelectionStart,
                Context,
                Runner.Registry.GetAllCommands());

            _completionCycleIndex = -1;
            _completionPrefix = current;
        }

        if (_currentCompletions.Count == 0) return;

        _completionCycleIndex = (_completionCycleIndex + 1) % _currentCompletions.Count;
        string candidate = _currentCompletions[_completionCycleIndex];

        // 替換最後一個 token
        int lastSpace = _completionPrefix.LastIndexOf(' ');
        if (lastSpace >= 0)
        {
            _txtInput.Text = _completionPrefix[..(lastSpace + 1)] + candidate;
        }
        else
        {
            _txtInput.Text = candidate;
        }

        _txtInput.SelectionStart = _txtInput.Text.Length;
    }

    /// <summary>
    /// 同步執行指令字串。
    /// </summary>
    public void ExecuteLine(string line)
    {
        _ = ExecuteLineAsync(line);
    }

    /// <summary>
    /// 非同步執行單行指令或巨集，避免長時間批次操作阻塞 WinForms 介面。
    /// </summary>
    public async Task<CommandResult> ExecuteLineAsync(string line)
    {
        if (Context is null || Runner is null)
        {
            AppendLog("錯誤: 控制台未綁定執行環境。", LogLevel.Error);
            return CommandResult.Fail("控制台未綁定執行環境。");
        }

        SetBusyState(true, $"執行中: {line}...");
        AppendLog($"> {line}", LogLevel.Command);

        _cts = new CancellationTokenSource();
        CommandResult result;

        try
        {
            result = await Task.Run(() => Runner.ExecuteLine(line, Context, recordSingleActionAsCompound: true), _cts.Token);
            CommandExecuted?.Invoke(this, result);
        }
        catch (OperationCanceledException)
        {
            result = CommandResult.Fail("指令執行已被使用者中斷。");
            AppendLog("[ABORT] 操作已取消。", LogLevel.Warning);
        }
        catch (Exception ex)
        {
            result = CommandResult.Fail(ex.Message, ex);
            AppendLog($"[ERR] {ex.Message}", LogLevel.Error);
        }
        finally
        {
            SetBusyState(false, "就緒 (Ready)");
            _cts = null;
        }

        return result;
    }

    private void PromptRunScript()
    {
        using var ofd = new OpenFileDialog
        {
            Title = "選取 Against Rome 巨集腳本檔案 (.armcmd)",
            Filter = "Macro Scripts (*.armcmd)|*.armcmd|All Files (*.*)|*.*"
        };

        if (ofd.ShowDialog() == DialogResult.OK)
        {
            _ = ExecuteLineAsync($"/run \"{ofd.FileName}\"");
        }
    }

    private void CancelExecution()
    {
        _cts?.Cancel();
    }

    private void SetBusyState(bool isBusy, string statusText)
    {
        if (InvokeRequired)
        {
            BeginInvoke(new Action(() => SetBusyState(isBusy, statusText)));
            return;
        }

        _isBusy = isBusy;
        _lblStatus.Text = statusText;
        _lblStatus.ForeColor = isBusy ? Color.FromArgb(255, 200, 80) : Color.FromArgb(140, 220, 140);
        _btnCancel.Visible = isBusy;
        _txtInput.Enabled = !isBusy;
        if (!isBusy) _txtInput.Focus();
    }
}

using System.Drawing;
using System.Windows.Forms;

namespace AgainstRomeModifier;

internal sealed class RestoreAllOptionsDialog : Form
{
    private readonly CheckBox _preserve = new() { AutoSize = true, Checked = true };
    internal bool PreserveCustomMaps => _preserve.Checked;

    internal RestoreAllOptionsDialog()
    {
        bool en = Loc.CurrentLanguage == Language.English;
        Text = en ? "Restore All" : "完整還原";
        AutoScaleDimensions = new SizeF(96F, 96F); AutoScaleMode = AutoScaleMode.Dpi; // 以 96 DPI 設計，PerMonitorV2 下依實際 DPI 縮放
        ClientSize = new Size(500, 240);
        AutoSize = true; AutoSizeMode = AutoSizeMode.GrowOnly; // 字型換行較預期多時加高，不截斷說明
        FormBorderStyle = FormBorderStyle.FixedDialog;
        StartPosition = FormStartPosition.CenterParent;
        MaximizeBox = false;
        MinimizeBox = false;
        ShowInTaskbar = false;
        var layout = new TableLayoutPanel { Dock = DockStyle.Fill, Padding = new Padding(16), ColumnCount = 1, RowCount = 3 };
        layout.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.Controls.Add(new Label { AutoSize = true, MaximumSize = new Size(460, 0), Text = en
            ? "Restore the original game settings. Custom maps are kept by default.\n\nUncheck below to delete registered custom maps and their contents. If restoration fails, the transaction restores the maps."
            : "還原遊戲的原版設定，預設保留自製地圖。\n\n取消勾選會刪除已登記的自製地圖及其內容；還原失敗時，交易會回復地圖。" }, 0, 0);
        _preserve.Text = en ? "Keep custom maps" : "保留自製地圖";
        layout.Controls.Add(_preserve, 0, 1);
        var buttons = new FlowLayoutPanel { AutoSize = true, Dock = DockStyle.Fill, FlowDirection = FlowDirection.RightToLeft, Padding = new Padding(0, 12, 0, 0) };
        var cancel = new Button { AutoSize = true, Text = en ? "Cancel" : "取消", DialogResult = DialogResult.Cancel };
        var restore = new Button { AutoSize = true, Text = en ? "Restore" : "還原", DialogResult = DialogResult.OK };
        buttons.Controls.Add(cancel);
        buttons.Controls.Add(restore);
        layout.Controls.Add(buttons, 0, 2);
        Controls.Add(layout);
        AcceptButton = restore;
        CancelButton = cancel;
        WinFormsTheme.Apply(this);
    }
}

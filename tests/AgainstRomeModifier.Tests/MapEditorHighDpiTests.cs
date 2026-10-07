using System.Drawing;
using System.Reflection;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

/// <summary>
/// 以 96 DPI 測試程序模擬高 DPI：字型依倍率放大（等同點數字型在高 DPI 的像素大小），
/// 已啟用 <see cref="AutoScaleMode.Dpi"/> 的視窗改宣告較低的設計 DPI，讓 WinForms 以同一倍率走真正的 PerformAutoScale。
/// 這不是實體高 DPI 螢幕證據；截圖輸出到 ARM_DPI_OUTPUT 或 TEMP 供人工檢視。
/// </summary>
public sealed partial class MapEditorSaveTransactionTests
{
    public static TheoryData<float> DpiScales => new() { 1F, 1.5F, 2F };

    [Theory]
    [MemberData(nameof(DpiScales))]
    public void Editor_forms_do_not_clip_text_when_emulating_high_dpi(float scale)
    {
        string output = Environment.GetEnvironmentVariable("ARM_DPI_OUTPUT")
            ?? Path.Combine(Path.GetTempPath(), "ArmDpi_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(output);
        string map = CreateFixture();
        var problems = new List<string>();
        RunInSta(() =>
        {
            Language previous = Loc.CurrentLanguage;
            try
            {
                foreach (Language language in new[] { Language.TraditionalChinese, Language.English })
                {
                    Loc.OverrideLanguageForTesting(language);
                    bool en = language == Language.English;
                    string tag = $"{(en ? "en" : "zh")}-{scale * 100:0}";
                    using (var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "DPI", "Test")))
                    {
                        _ = form.Handle; Invoke(form, "LoadSelectedMap"); Invoke(form, "SetActiveView", false);
                        Show(form, scale);
                        Type mode = typeof(MapEditorForm).GetNestedType("EditMode", BindingFlags.NonPublic)!;
                        foreach (string name in Enum.GetNames(mode))
                        {
                            Invoke(form, "SetEditMode", Enum.Parse(mode, name)); Settle(form);
                            Inspect(form, $"main-{name}-{tag}", output, problems);
                        }
                    }
                    var target = new ScenarioSpawn("HOUSE", 4000, 5000, 0) { Id = Guid.NewGuid() };
                    Check(new ScenarioEventDialog(new("Event") { Actions = [new(ScenarioActionKind.Message, "Ready")], Conditions = [new(ScenarioConditionKind.ObjectExists, target.Id)] },
                        ["GER_INF01"], en, targets: [target]), $"event-{tag}", scale, output, problems);
                    Check(new ScenarioActionDialog(new(ScenarioActionKind.SpawnUnit, Alias: "GER_INF01"), ["GER_INF01"], en), $"action-{tag}", scale, output, problems);
                    Check(new ScenarioConditionDialog(new(ScenarioConditionKind.ObjectInArea, target.Id, 0, 0, 100, 100), [target], en), $"condition-{tag}", scale, output, problems);
                    var placed = new SdlPlacedObject(new SdlObjectType("FigGerLeader", 1, SdlObjectCategory.Figure, "Ger", 1,
                        new Dictionary<string, string> { ["alias"] = "LEADER" }), 100, 0, 200, 0, 45, 10) { ScenarioId = Guid.NewGuid() };
                    Check(new PlacedObjectEditDialog(placed, en), $"placed-{tag}", scale, output, problems);
                    Check(new MapSelectionForm(_root), $"select-{tag}", scale, output, problems);
                    Check(new RestoreAllOptionsDialog(), $"restore-{tag}", scale, output, problems);
                }
            }
            finally { Loc.OverrideLanguageForTesting(previous); }
        }, TimeSpan.FromMinutes(3));
        File.WriteAllLines(Path.Combine(output, $"problems-{scale * 100:0}.txt"), problems);
        Assert.True(problems.Count == 0, $"{problems.Count} 個截字／越界（截圖：{output}）：\n" + string.Join("\n", problems.Take(40)));
    }

    private static void Check(Form form, string name, float scale, string output, List<string> problems)
    {
        using (form) { Show(form, scale); Inspect(form, name, output, problems); }
    }

    private static void Show(Form form, float scale)
    {
        var fonts = new List<(Control Control, Font Font)>();
        void Collect(Control control) { fonts.Add((control, control.Font)); foreach (Control child in control.Controls) Collect(child); }
        Collect(form);
        foreach (var (control, font) in fonts) control.Font = new Font(font.FontFamily, font.SizeInPoints * scale, font.Style, GraphicsUnit.Point);
        if (form.AutoScaleMode == AutoScaleMode.Dpi) form.AutoScaleDimensions = new SizeF(96F / scale, 96F / scale);
        form.StartPosition = FormStartPosition.Manual; form.Location = new Point(-30000, -30000);
        form.Show(); Settle(form);
        if (!form.MinimumSize.IsEmpty) form.Size = form.MinimumSize; // 最小尺寸是最容易截字的情境
        Settle(form);
    }

    private static void Settle(Form form) { form.PerformLayout(); for (int i = 0; i < 5; i++) Application.DoEvents(); }

    private static void Inspect(Form form, string name, string output, List<string> problems)
    {
        using (var bitmap = new Bitmap(Math.Max(1, form.Width), Math.Max(1, form.Height)))
        {
            form.DrawToBitmap(bitmap, new Rectangle(Point.Empty, form.Size));
            bitmap.Save(Path.Combine(output, name + ".png"));
        }
        Visit(form, name, problems);
        var layout = new List<string>();
        void Dump(Control control, string indent)
        {
            foreach (Control child in control.Controls)
            {
                layout.Add($"{indent}{child.GetType().Name} '{Short(child.Text)}' {child.Bounds} visible={child.Visible}");
                Dump(child, indent + "  ");
            }
        }
        Dump(form, "");
        File.WriteAllLines(Path.Combine(output, name + ".layout.txt"), layout);
    }

    private static void Visit(Control control, string name, List<string> problems)
    {
        if (control is TableLayoutPanel) // 儲存格內容超出所在列時會與相鄰儲存格重疊
        {
            Control[] cells = control.Controls.Cast<Control>().Where(child => child.Visible && child.Width > 0 && child.Height > 0).ToArray();
            for (int i = 0; i < cells.Length; i++)
            for (int j = i + 1; j < cells.Length; j++)
                if (cells[i].Bounds.IntersectsWith(cells[j].Bounds))
                    problems.Add($"{name}: {cells[i].GetType().Name} '{Short(cells[i].Text)}' {cells[i].Bounds} 與 {cells[j].GetType().Name} '{Short(cells[j].Text)}' {cells[j].Bounds} 重疊");
        }
        foreach (Control child in control.Controls)
        {
            if (!child.Visible) continue;
            if (child is Label or ButtonBase && !child.AutoSize && !string.IsNullOrWhiteSpace(child.Text) && child.Width > 0)
            {
                int padding = child.Padding.Horizontal + (child is ButtonBase ? 10 : 0) + (child is CheckBox or RadioButton ? 20 : 0);
                int available = Math.Max(1, child.Width - padding);
                Size needed = TextRenderer.MeasureText(child.Text, child.Font, new Size(available, int.MaxValue),
                    TextFormatFlags.WordBreak | TextFormatFlags.TextBoxControl);
                if (needed.Height > child.Height - child.Padding.Vertical + 2)
                    problems.Add($"{name}: {child.GetType().Name} '{Short(child.Text)}' {child.Size} 需要 {needed}");
            }
            if (child is Label or ButtonBase && child.AutoSize && !string.IsNullOrWhiteSpace(child.Text))
            {
                // 版面容器可能把 AutoSize 控制項壓在儲存格內；實際大小小於自身需要的大小即為截斷。
                // Label 可換行：以實際寬度求需要的高度；按鈕類不換行，比較兩個方向。
                Size preferred = child is Label ? child.GetPreferredSize(new Size(child.Width, 0)) : child.GetPreferredSize(Size.Empty);
                if (preferred.Height > child.Height + 2 || (child is ButtonBase && preferred.Width > child.Width + 2))
                    problems.Add($"{name}: {child.GetType().Name} '{Short(child.Text)}' {child.Size} 被壓縮，需要 {preferred}");
            }
            if (control is not ScrollableControl { AutoScroll: true } && control is not ToolStrip && child.Width > 0 && child.Height > 0
                && (child.Right > control.ClientSize.Width + 1 || child.Bottom > control.ClientSize.Height + 1) && control.ClientSize.Width > 0)
                problems.Add($"{name}: {child.GetType().Name} '{Short(child.Text)}' {child.Bounds} 超出 {control.GetType().Name} {control.ClientSize}");
            Visit(child, name, problems);
        }
    }

    private static string Short(string text) => text.Length <= 24 ? text.Replace('\n', ' ') : text[..24].Replace('\n', ' ') + "…";
}

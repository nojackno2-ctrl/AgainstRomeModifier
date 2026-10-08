using AgainstRomeModifier;
using System.Globalization;

namespace AgainstRomeMapEditor;

internal enum TerrainRegionOperation { Rectangle, Connected, Road, SelectObjects, River, Cliff, Flora }

internal sealed class TerrainRegionDialog : Form
{
    private readonly ComboBox _operation = new() { DropDownStyle = ComboBoxStyle.DropDownList, Dock = DockStyle.Fill };
    private readonly TextBox _vertices = new() { Multiline = true, Dock = DockStyle.Fill, ScrollBars = ScrollBars.Vertical, Text = "10,10\r\n20,20" };
    private readonly NumericUpDown _width = new() { Minimum = 1, Maximum = 64, Value = 3, Dock = DockStyle.Fill };
    private readonly Label _summary = new() { Dock = DockStyle.Fill };
    internal TerrainRegionOperation Operation { get => (TerrainRegionOperation)_operation.SelectedIndex; set => _operation.SelectedIndex = (int)value; }
    internal string VerticesText { get => _vertices.Text; set => _vertices.Text = value; }
    internal int RoadWidth { get => (int)_width.Value; set => _width.Value = value; }
    internal void UseRectangleInput(string title) { Text = title; Operation = TerrainRegionOperation.Rectangle; _operation.Enabled = false; }

    internal (int X, int Y)[] ReadVertices() => _vertices.Text.Split(['\r', '\n'], StringSplitOptions.RemoveEmptyEntries)
        .Select(line => line.Split(','))
        .Select(parts => parts.Length == 2 ? (int.Parse(parts[0].Trim(), CultureInfo.InvariantCulture), int.Parse(parts[1].Trim(), CultureInfo.InvariantCulture))
            : throw new FormatException("Use X,Y on each line.")).ToArray();

    public TerrainRegionDialog(int dimension)
    {
        bool en = Loc.CurrentLanguage == Language.English;
        Text = en ? "Region tools" : "區域工具";
        AutoScaleMode = AutoScaleMode.Dpi; AutoScaleDimensions = new SizeF(96, 96);
        StartPosition = FormStartPosition.CenterParent; ClientSize = new Size(440, 390);
        MinimizeBox = MaximizeBox = false; FormBorderStyle = FormBorderStyle.FixedDialog;
        _operation.Items.AddRange(en ? new object[] { "Rectangle fill", "Connected material fill", "Dirt road polyline", "Select objects in rectangle", "River flow in rectangle", "Cliff face on slopes in rectangle", "Ecology flora scatter in rectangle" }
            : new object[] { "矩形填色", "同材質連通區填色", "土路折線", "矩形選取物件", "矩形水系河流", "矩形懸崖岩壁", "矩形生態植被散播" });
        _width.Maximum = dimension;
        var grid = new TableLayoutPanel { Dock = DockStyle.Fill, Padding = new Padding(12), ColumnCount = 1, RowCount = 6 };
        grid.RowStyles.Add(new RowStyle(SizeType.AutoSize)); grid.RowStyles.Add(new RowStyle(SizeType.Absolute, 72));
        grid.RowStyles.Add(new RowStyle(SizeType.Percent, 100)); grid.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        grid.RowStyles.Add(new RowStyle(SizeType.Absolute, 48)); grid.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        grid.Controls.Add(_operation, 0, 0);
        grid.Controls.Add(new Label { Dock = DockStyle.Fill, Text = en
            ? $"Tile coordinates: 0–{dimension - 1}. One X,Y per line. Rectangle/River/Cliff: two opposite corners (or road waypoints); fill: one seed. Road width is in tiles."
            : $"圖格座標：0–{dimension - 1}；每行 X,Y。矩形填色/河流/懸崖輸入兩個對角（折線輸入轉折點）、連通填色輸入一個起點。下方為土路寬度（格）。" }, 0, 1);
        grid.Controls.Add(_vertices, 0, 2); grid.Controls.Add(_width, 0, 3); grid.Controls.Add(_summary, 0, 4);
        var buttons = new FlowLayoutPanel { AutoSize = true, Dock = DockStyle.Fill, FlowDirection = FlowDirection.RightToLeft };
        var apply = new Button { AutoSize = true, Text = en ? "Apply" : "套用" };
        var cancel = new Button { AutoSize = true, Text = en ? "Cancel" : "取消", DialogResult = DialogResult.Cancel };
        apply.Click += (_, _) =>
        {
            try
            {
                var vertices = ReadVertices();
                int count = Operation == TerrainRegionOperation.Connected ? 1 : 2;
                if (vertices.Length < count || (Operation != TerrainRegionOperation.Road && Operation != TerrainRegionOperation.River && vertices.Length != count) ||
                    vertices.Any(v => v.X < 0 || v.Y < 0 || v.X >= dimension || v.Y >= dimension)) throw new FormatException();
                DialogResult = DialogResult.OK; Close();
            }
            catch (Exception ex) when (ex is FormatException or OverflowException)
            { _summary.Text = en ? "Check coordinate format, bounds and vertex count." : "請檢查座標格式、範圍與轉折點數量。"; }
        };
        _operation.SelectedIndexChanged += (_, _) => _width.Enabled = Operation == TerrainRegionOperation.Road;
        Operation = TerrainRegionOperation.Rectangle;
        buttons.Controls.AddRange([apply, cancel]); grid.Controls.Add(buttons, 0, 5);
        Controls.Add(grid); AcceptButton = apply; CancelButton = cancel; WinFormsTheme.Apply(this);
    }
}

using System.Numerics;
using AgainstRomeMapEditor.Modules.Cinematics;
using AgainstRomeModifier;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    private MapStartCamera _startCamera = new() { Enabled = false };
    private bool _startCameraDirty;

    private void LoadStartCameraForMap(string map)
    {
        try { _startCamera = MapStartCamera.Load(map); }
        catch (Exception ex) when (ex is IOException or InvalidDataException or System.Text.Json.JsonException)
        { _startCamera = new() { Enabled = false }; }
        _startCameraDirty = false;
    }

    /// <summary>開局相機只在存檔交易內經由 LevelScriptInjector 從原版備份重建注入。</summary>
    private Action<AgainstRomeModifier.Scripting.BciImage>? StartCameraPrologue()
    {
        MapStartCamera camera = _startCamera;
        return camera.Enabled ? image => MapStartCameraInjector.Inject(image, camera) : null;
    }
    private readonly ToolStripMenuItem _startCameraMenuItem = new("開局相機設定…");

    private void InitializeStartCameraMenu(ToolStrip tools)
    {
        _startCameraMenuItem.Click += (_, _) => OpenStartCameraDialog();

        // 插入至配置選單中（野外巢穴下方）
        int idx = _layoutMenu.DropDownItems.IndexOf(_wildLairs);
        if (idx >= 0)
            _layoutMenu.DropDownItems.Insert(idx + 1, _startCameraMenuItem);
        else
            _layoutMenu.DropDownItems.Add(_startCameraMenuItem);
    }

    internal void OpenStartCameraDialog()
    {
        if (_selected?.IsCustom != true) return;
        bool isEn = Loc.CurrentLanguage == Language.English;

        // 若目前地圖已有存檔的開局相機，先行載入
        MapStartCamera current = MapStartCamera.Load(_selected.DirectoryPath);
        if (!current.Enabled && !_startCamera.Enabled)
        {
            // 若未曾設定，預設以目前視角填入
            if (GetCurrentViewCameraCoords() is { } view)
            {
                current = new MapStartCamera
                {
                    Enabled = true,
                    X = view.x,
                    Y = view.y,
                    Z = view.z,
                    Zoom = view.zoom,
                    Primitive = StartCameraPrimitive.CenterToPos
                };
            }
        }

        using var dialog = new StartCameraDialog(current)
        {
            ViewCaptureProvider = GetCurrentViewCameraCoords
        };

        if (dialog.ShowDialog(this) == DialogResult.OK)
        {
            _startCamera = dialog.Camera;
            _startCameraDirty = true;
            UpdateEditorState();
            _status.Text = isEn
                ? $"Start camera set at ({_startCamera.X:F0}, {_startCamera.Z:F0}), Zoom {_startCamera.Zoom:F1}; Save to write."
                : $"已設定開局相機：座標 ({_startCamera.X:F0}, {_startCamera.Z:F0})，縮放 {_startCamera.Zoom:F1}；按「儲存」才會寫入。";
        }
    }

    private (double x, double y, double z, double zoom)? GetCurrentViewCameraCoords()
    {
        if (_selected is null) return null;
        if (_view3d is not null && _view3d.Visible)
        {
            Vector3 target = _view3d.CameraTarget;
            double wx = Math.Clamp(target.X * 256.0, 0, 16383);
            double wz = Math.Clamp(target.Z * 256.0, 0, 16383);
            double wy = target.Y;
            return (wx, wy, wz, 5.0);
        }
        else if (_canvas is not null)
        {
            Rectangle bounds = _canvas.SceneBoundsRectangle;
            if (bounds.Width > 0 && bounds.Height > 0)
            {
                float fracX = (_canvas.ClientSize.Width / 2f - bounds.X) / (float)bounds.Width;
                float fracY = (_canvas.ClientSize.Height / 2f - bounds.Y) / (float)bounds.Height;
                int dim = _texturesDocument?.Dimension ?? _canvas.MapDimension;
                if (dim <= 0) dim = 64;
                double wx = Math.Clamp(fracX * dim * 256.0, 0, 16383);
                double wz = Math.Clamp(fracY * dim * 256.0, 0, 16383);
                return (wx, 0.0, wz, 5.0);
            }
        }
        return (8192.0, 0.0, 8192.0, 5.0);
    }
}

/// <summary>
/// 供企劃在編輯器中指定地圖開局相機位置與縮放之互動視窗。
/// </summary>
internal sealed class StartCameraDialog : Form
{
    private readonly CheckBox _enabled = new() { AutoSize = true, Checked = true };
    private readonly NumericUpDown _numX = new() { Minimum = 0, Maximum = 16383, Value = 8192, Increment = 32, Dock = DockStyle.Fill };
    private readonly NumericUpDown _numZ = new() { Minimum = 0, Maximum = 16383, Value = 8192, Increment = 32, Dock = DockStyle.Fill };
    private readonly NumericUpDown _numY = new() { Minimum = -500, Maximum = 5000, Value = 0, Increment = 1, Dock = DockStyle.Fill };
    private readonly NumericUpDown _numZoom = new() { Minimum = 0, Maximum = 9, Value = 5, Increment = 0.5m, DecimalPlaces = 1, Dock = DockStyle.Fill };
    private readonly RadioButton _radCenterToPos = new() { AutoSize = true, Checked = true };
    private readonly RadioButton _radDirectEnginePos = new() { AutoSize = true };
    private readonly Button _btnCapture = new() { AutoSize = true };

    internal Func<(double x, double y, double z, double zoom)?>? ViewCaptureProvider { get; set; }

    internal MapStartCamera Camera => new()
    {
        Enabled = _enabled.Checked,
        X = (double)_numX.Value,
        Y = (double)_numY.Value,
        Z = (double)_numZ.Value,
        Zoom = (double)_numZoom.Value,
        Primitive = _radCenterToPos.Checked ? StartCameraPrimitive.CenterToPos : StartCameraPrimitive.DirectEnginePos
    };

    internal void CaptureFromCurrentView()
    {
        if (ViewCaptureProvider?.Invoke() is { } view)
        {
            _numX.Value = (decimal)Math.Clamp(view.x, 0, 16383);
            _numY.Value = (decimal)Math.Clamp(view.y, -500, 5000);
            _numZ.Value = (decimal)Math.Clamp(view.z, 0, 16383);
            _numZoom.Value = (decimal)Math.Clamp(view.zoom, 0, 9);
        }
    }

    public StartCameraDialog(MapStartCamera? initial = null)
    {
        bool en = Loc.CurrentLanguage == Language.English;
        Text = en ? "Map Start Camera Settings" : "地圖開局相機設定";
        AutoScaleMode = AutoScaleMode.Dpi;
        AutoScaleDimensions = new SizeF(96, 96);
        StartPosition = FormStartPosition.CenterParent;
        FormBorderStyle = FormBorderStyle.FixedDialog;
        MinimizeBox = MaximizeBox = false;
        ClientSize = new Size(500, 360);

        _enabled.Text = en ? "Enable Custom Start Camera" : "啟用自訂開局相機（寫入關卡腳本）";
        _radCenterToPos.Text = en
            ? "s_centerToPos(x, z) [Evidenced in campaign scripts; auto-elevation]"
            : "s_centerToPos(x, z) [原版戰役/多人腳本驗證；自動地表高度校正]";
        _radDirectEnginePos.Text = en
            ? "s_lgcSetEnginePos(x, y, z) [Direct engine coordinates; manual elevation]"
            : "s_lgcSetEnginePos(x, y, z) [直接相機空間座標；手動指定海拔]";

        _btnCapture.Text = en ? "Capture Current View (讀取目前視角)" : "讀取目前編輯器視角 (Capture View)";
        _btnCapture.Click += (_, _) => CaptureFromCurrentView();

        if (initial is not null)
        {
            _enabled.Checked = initial.Enabled;
            _numX.Value = (decimal)Math.Clamp(initial.X, 0, 16383);
            _numY.Value = (decimal)Math.Clamp(initial.Y, -500, 5000);
            _numZ.Value = (decimal)Math.Clamp(initial.Z, 0, 16383);
            _numZoom.Value = (decimal)Math.Clamp(initial.Zoom, 0, 9);
            if (initial.Primitive == StartCameraPrimitive.DirectEnginePos)
            {
                _radCenterToPos.Checked = false;
                _radDirectEnginePos.Checked = true;
            }
            else
            {
                _radDirectEnginePos.Checked = false;
                _radCenterToPos.Checked = true;
            }
        }

        var grid = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            ColumnCount = 2,
            RowCount = 7,
            Padding = new Padding(12)
        };
        grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 40));
        grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 60));

        grid.Controls.Add(_enabled, 0, 0);
        grid.SetColumnSpan(_enabled, 2);

        grid.Controls.Add(new Label { Text = en ? "World X (世界座標 X):" : "世界座標 X (0..16383):", AutoSize = true, Anchor = AnchorStyles.Left }, 0, 1);
        grid.Controls.Add(_numX, 1, 1);

        grid.Controls.Add(new Label { Text = en ? "World Z (世界座標 Z):" : "世界座標 Z (0..16383):", AutoSize = true, Anchor = AnchorStyles.Left }, 0, 2);
        grid.Controls.Add(_numZ, 1, 2);

        grid.Controls.Add(new Label { Text = en ? "Elevation Y (海拔高度):" : "相機海拔 Y (預設 0):", AutoSize = true, Anchor = AnchorStyles.Left }, 0, 3);
        grid.Controls.Add(_numY, 1, 3);

        grid.Controls.Add(new Label { Text = en ? "Native Zoom (原生縮放 0..9):" : "原生縮放 (0..9，預設 5):", AutoSize = true, Anchor = AnchorStyles.Left }, 0, 4);
        grid.Controls.Add(_numZoom, 1, 4);

        var primPanel = new FlowLayoutPanel { Dock = DockStyle.Fill, AutoSize = true, FlowDirection = FlowDirection.TopDown };
        primPanel.Controls.Add(_radCenterToPos);
        primPanel.Controls.Add(_radDirectEnginePos);
        grid.Controls.Add(primPanel, 0, 5);
        grid.SetColumnSpan(primPanel, 2);

        grid.Controls.Add(_btnCapture, 0, 6);
        grid.SetColumnSpan(_btnCapture, 2);

        var buttons = new FlowLayoutPanel
        {
            Dock = DockStyle.Bottom,
            Height = 44,
            FlowDirection = FlowDirection.RightToLeft,
            Padding = new Padding(8)
        };
        var btnCancel = new Button { Text = en ? "Cancel" : "取消", DialogResult = DialogResult.Cancel, AutoSize = true };
        var btnOk = new Button { Text = en ? "OK" : "確定", DialogResult = DialogResult.OK, AutoSize = true };
        buttons.Controls.Add(btnCancel);
        buttons.Controls.Add(btnOk);

        Controls.Add(grid);
        Controls.Add(buttons);
        AcceptButton = btnOk;
        CancelButton = btnCancel;
    }
}

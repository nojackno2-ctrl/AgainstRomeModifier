using System.Drawing.Drawing2D;
using AgainstRomeMapEditor.Modules.Events.Graph;

namespace AgainstRomeMapEditor.Events;

/// <summary>
/// 劇情事件節點圖 2D 畫布控制項 (WinForms)：
/// 支援硬體加速自繪、視角平移、縮放、三次貝茲連線、節點拖曳選取與端口動態連線互動。
/// </summary>
public sealed class EventGraphCanvasControl : Control
{
    private const float DefaultNodeWidth = 220f;
    private const float DefaultNodeHeaderHeight = 28f;
    private const float PortRowHeight = 24f;
    private const float PortCircleRadius = 5.5f;

    private EventGraph? _graph;
    private float _zoom = 1.0f;
    private PointF _pan = new(0, 0);

    // 互動狀態
    private enum DragState { None, Panning, MovingNodes, DraggingWire, BoxSelecting }
    private DragState _state = DragState.None;
    private Point _lastMousePos;
    private Point _dragStartScreen;
    private RectangleF _selectionBoxWorld;

    // 端口連線暫存
    private GraphPort? _dragSourcePort;
    private PointF _dragWireWorldCurrent;

    // 選取與診斷
    private readonly HashSet<Guid> _selectedNodes = new();
    private readonly List<GraphDiagnostic> _diagnostics = new();

    public event EventHandler? SelectionChanged;
    public event EventHandler? GraphModified;

    public EventGraph? Graph
    {
        get => _graph;
        set
        {
            _graph = value;
            _selectedNodes.Clear();
            Invalidate();
        }
    }

    public float Zoom
    {
        get => _zoom;
        set
        {
            _zoom = Math.Clamp(value, 0.25f, 2.5f);
            Invalidate();
        }
    }

    public PointF Pan
    {
        get => _pan;
        set
        {
            _pan = value;
            Invalidate();
        }
    }

    public IReadOnlySet<Guid> SelectedNodes => _selectedNodes;

    public EventGraphCanvasControl()
    {
        SetStyle(
            ControlStyles.AllPaintingInWmPaint |
            ControlStyles.UserPaint |
            ControlStyles.OptimizedDoubleBuffer |
            ControlStyles.ResizeRedraw,
            true);

        BackColor = Color.FromArgb(30, 30, 32);
        TabStop = true;
    }

    public void SetDiagnostics(IEnumerable<GraphDiagnostic> diagnostics)
    {
        _diagnostics.Clear();
        if (diagnostics is not null)
        {
            _diagnostics.AddRange(diagnostics);
        }
        Invalidate();
    }

    #region Coordinate Transformations

    public PointF ScreenToWorld(Point pt) =>
        new((pt.X - _pan.X) / _zoom, (pt.Y - _pan.Y) / _zoom);

    public Point WorldToScreen(PointF pt) =>
        new((int)MathF.Round(pt.X * _zoom + _pan.X), (int)MathF.Round(pt.Y * _zoom + _pan.Y));

    public RectangleF GetNodeBoundsWorld(GraphNode node)
    {
        int inputCount = node.Ports.Count(p => p.Direction == PortDirection.Input);
        int outputCount = node.Ports.Count(p => p.Direction == PortDirection.Output);
        int rows = Math.Max(1, Math.Max(inputCount, outputCount));
        float height = DefaultNodeHeaderHeight + rows * PortRowHeight + 12f;
        return new RectangleF(node.X, node.Y, DefaultNodeWidth, height);
    }

    public PointF GetPortCenterWorld(GraphPort port)
    {
        var node = _graph?.FindNode(port.NodeId);
        if (node is null) return PointF.Empty;

        var bounds = GetNodeBoundsWorld(node);
        var dirPorts = node.Ports.Where(p => p.Direction == port.Direction).ToList();
        int index = dirPorts.IndexOf(port);
        if (index < 0) index = 0;

        float y = bounds.Y + DefaultNodeHeaderHeight + 12f + index * PortRowHeight + PortRowHeight * 0.5f;
        float x = port.Direction == PortDirection.Input ? bounds.Left : bounds.Right;
        return new PointF(x, y);
    }

    #endregion

    #region Rendering

    protected override void OnPaint(PaintEventArgs e)
    {
        base.OnPaint(e);
        var g = e.Graphics;
        g.SmoothingMode = SmoothingMode.AntiAlias;
        g.TextRenderingHint = System.Drawing.Text.TextRenderingHint.ClearTypeGridFit;

        // 1. 繪製背景網格
        DrawBackgroundGrid(g);

        if (_graph is null) return;

        // 設定世界變換矩陣
        var state = g.Save();
        g.TranslateTransform(_pan.X, _pan.Y);
        g.ScaleTransform(_zoom, _zoom);

        // 2. 繪製已建立的連線
        DrawEdges(g);

        // 3. 繪製正在拖曳中的動態連線
        DrawDraggingWire(g);

        // 4. 繪製所有節點卡片與連接埠
        DrawNodes(g);

        // 5. 繪製框選範圍
        if (_state == DragState.BoxSelecting)
        {
            using var boxBrush = new SolidBrush(Color.FromArgb(40, 0, 122, 204));
            using var boxPen = new Pen(Color.FromArgb(180, 0, 122, 204), 1f / _zoom);
            g.FillRectangle(boxBrush, _selectionBoxWorld);
            g.DrawRectangle(boxPen, _selectionBoxWorld.X, _selectionBoxWorld.Y, _selectionBoxWorld.Width, _selectionBoxWorld.Height);
        }

        g.Restore(state);
    }

    private void DrawBackgroundGrid(Graphics g)
    {
        float gridSize = 32f * _zoom;
        if (gridSize < 8f) return;

        float startX = _pan.X % gridSize;
        float startY = _pan.Y % gridSize;

        using var pen = new Pen(Color.FromArgb(42, 42, 46), 1f);
        for (float x = startX; x < Width; x += gridSize)
        {
            g.DrawLine(pen, x, 0, x, Height);
        }
        for (float y = startY; y < Height; y += gridSize)
        {
            g.DrawLine(pen, 0, y, Width, y);
        }
    }

    private void DrawEdges(Graphics g)
    {
        if (_graph is null) return;

        foreach (var edge in _graph.Edges)
        {
            var p1 = _graph.FindPort(edge.SourcePortId);
            var p2 = _graph.FindPort(edge.TargetPortId);
            if (p1 is null || p2 is null) continue;

            PointF start = GetPortCenterWorld(p1);
            PointF end = GetPortCenterWorld(p2);

            bool isExec = p1.Type == PortType.Execution;
            Color wireColor = isExec ? Color.FromArgb(230, 230, 235) : Color.FromArgb(46, 204, 113);
            float width = isExec ? 2.5f : 2.0f;

            DrawCubicBezierWire(g, start, end, wireColor, width);
        }
    }

    private void DrawDraggingWire(Graphics g)
    {
        if (_state != DragState.DraggingWire || _dragSourcePort is null) return;

        PointF start = GetPortCenterWorld(_dragSourcePort);
        PointF end = _dragWireWorldCurrent;
        if (_dragSourcePort.Direction == PortDirection.Input)
        {
            (start, end) = (end, start);
        }

        bool isExec = _dragSourcePort.Type == PortType.Execution;
        Color wireColor = isExec ? Color.FromArgb(255, 220, 100) : Color.FromArgb(100, 255, 150);
        DrawCubicBezierWire(g, start, end, wireColor, 2.5f, dashed: true);
    }

    private static void DrawCubicBezierWire(Graphics g, PointF start, PointF end, Color color, float width, bool dashed = false)
    {
        float dx = Math.Abs(end.X - start.X) * 0.5f;
        dx = Math.Max(dx, 40f);

        PointF c1 = new(start.X + dx, start.Y);
        PointF c2 = new(end.X - dx, end.Y);

        using var pen = new Pen(color, width);
        if (dashed)
        {
            pen.DashStyle = DashStyle.Dash;
        }
        g.DrawBezier(pen, start, c1, c2, end);
    }

    private void DrawNodes(Graphics g)
    {
        if (_graph is null) return;

        using var headerFont = new Font(Font.FontFamily, 9.5f, FontStyle.Bold);
        using var portFont = new Font(Font.FontFamily, 8.5f, FontStyle.Regular);

        foreach (var node in _graph.Nodes)
        {
            var bounds = GetNodeBoundsWorld(node);
            bool isSelected = _selectedNodes.Contains(node.Id);

            // 檢查節點診斷錯誤
            var nodeDiags = _diagnostics.Where(d => d.NodeId == node.Id).ToList();
            bool hasError = nodeDiags.Any(d => d.Severity == GraphDiagnosticSeverity.Error);
            bool hasWarning = nodeDiags.Any(d => d.Severity == GraphDiagnosticSeverity.Warning);

            // 1. 卡片主體底色
            using (var bodyBrush = new SolidBrush(Color.FromArgb(45, 45, 48)))
            {
                g.FillRectangle(bodyBrush, bounds);
            }

            // 2. 標題列背景色
            Color headerColor = GetNodeHeaderColor(node);
            var headerRect = new RectangleF(bounds.X, bounds.Y, bounds.Width, DefaultNodeHeaderHeight);
            using (var headerBrush = new SolidBrush(headerColor))
            {
                g.FillRectangle(headerBrush, headerRect);
            }

            // 3. 標題文字
            using (var textBrush = new SolidBrush(Color.White))
            {
                var textRect = new RectangleF(headerRect.X + 8, headerRect.Y + 6, headerRect.Width - 16, headerRect.Height - 12);
                g.DrawString(node.Title, headerFont, textBrush, textRect);
            }

            // 4. 外框線（選取高亮 / 錯誤高亮 / 普通邊界）
            Color borderColor = hasError ? Color.FromArgb(231, 76, 60) :
                                hasWarning ? Color.FromArgb(241, 196, 15) :
                                isSelected ? Color.FromArgb(0, 153, 255) : Color.FromArgb(70, 70, 74);
            float borderWidth = (isSelected || hasError) ? 2.5f : 1.2f;

            using (var borderPen = new Pen(borderColor, borderWidth))
            {
                g.DrawRectangle(borderPen, bounds.X, bounds.Y, bounds.Width, bounds.Height);
            }

            // 5. 繪製連接埠
            DrawNodePorts(g, node, bounds, portFont);
        }
    }

    private void DrawNodePorts(Graphics g, GraphNode node, RectangleF bounds, Font font)
    {
        var inputPorts = node.Ports.Where(p => p.Direction == PortDirection.Input).ToList();
        var outputPorts = node.Ports.Where(p => p.Direction == PortDirection.Output).ToList();

        // 繪製輸入端口 (左側)
        for (int i = 0; i < inputPorts.Count; i++)
        {
            var port = inputPorts[i];
            PointF pt = GetPortCenterWorld(port);
            Color portColor = port.Type == PortType.Execution ? Color.White : Color.FromArgb(46, 204, 113);

            using var brush = new SolidBrush(portColor);
            g.FillEllipse(brush, pt.X - PortCircleRadius, pt.Y - PortCircleRadius, PortCircleRadius * 2, PortCircleRadius * 2);

            using var textBrush = new SolidBrush(Color.FromArgb(200, 200, 200));
            g.DrawString(port.DisplayName, font, textBrush, pt.X + 10, pt.Y - 7);
        }

        // 繪製輸出端口 (右側)
        for (int i = 0; i < outputPorts.Count; i++)
        {
            var port = outputPorts[i];
            PointF pt = GetPortCenterWorld(port);
            Color portColor = port.Type == PortType.Execution ? Color.White : Color.FromArgb(46, 204, 113);

            using var brush = new SolidBrush(portColor);
            g.FillEllipse(brush, pt.X - PortCircleRadius, pt.Y - PortCircleRadius, PortCircleRadius * 2, PortCircleRadius * 2);

            var size = g.MeasureString(port.DisplayName, font);
            using var textBrush = new SolidBrush(Color.FromArgb(200, 200, 200));
            g.DrawString(port.DisplayName, font, textBrush, pt.X - size.Width - 10, pt.Y - 7);
        }
    }

    private static Color GetNodeHeaderColor(GraphNode node) => node switch
    {
        EventTriggerNode => Color.FromArgb(192, 57, 43),     // 觸發：磚紅
        ConditionNode => Color.FromArgb(39, 174, 96),        // 條件：翡翠綠
        VictoryActionNode => Color.FromArgb(243, 156, 18),   // 勝利：金色
        DefeatActionNode => Color.FromArgb(142, 68, 173),    // 失敗：深紫
        ActionNode => Color.FromArgb(41, 128, 185),          // 動作：天空藍
        DelayNode => Color.FromArgb(127, 140, 141),          // 流控延遲：灰石
        _ => Color.FromArgb(52, 73, 94)
    };

    #endregion

    #region Mouse Interactions

    protected override void OnMouseDown(MouseEventArgs e)
    {
        base.OnMouseDown(e);
        Focus();
        _lastMousePos = e.Location;
        _dragStartScreen = e.Location;
        PointF worldPos = ScreenToWorld(e.Location);

        if (e.Button == MouseButtons.Middle || (e.Button == MouseButtons.Left && ModifierKeys.HasFlag(Keys.Space)))
        {
            _state = DragState.Panning;
            return;
        }

        if (e.Button == MouseButtons.Left && _graph is not null)
        {
            // 1. 優先檢查是否點擊連接埠
            var hitPort = HitTestPort(worldPos);
            if (hitPort is not null)
            {
                _state = DragState.DraggingWire;
                _dragSourcePort = hitPort;
                _dragWireWorldCurrent = worldPos;
                Invalidate();
                return;
            }

            // 2. 檢查是否點擊節點
            var hitNode = HitTestNode(worldPos);
            if (hitNode is not null)
            {
                _state = DragState.MovingNodes;
                if (!ModifierKeys.HasFlag(Keys.Shift) && !_selectedNodes.Contains(hitNode.Id))
                {
                    _selectedNodes.Clear();
                }
                _selectedNodes.Add(hitNode.Id);
                SelectionChanged?.Invoke(this, EventArgs.Empty);
                Invalidate();
                return;
            }

            // 3. 點擊空白處：啟動框選
            if (!ModifierKeys.HasFlag(Keys.Shift))
            {
                _selectedNodes.Clear();
                SelectionChanged?.Invoke(this, EventArgs.Empty);
            }
            _state = DragState.BoxSelecting;
            _selectionBoxWorld = new RectangleF(worldPos.X, worldPos.Y, 0, 0);
            Invalidate();
        }
    }

    protected override void OnMouseMove(MouseEventArgs e)
    {
        base.OnMouseMove(e);
        PointF worldPos = ScreenToWorld(e.Location);
        float dx = (e.X - _lastMousePos.X) / _zoom;
        float dy = (e.Y - _lastMousePos.Y) / _zoom;

        switch (_state)
        {
            case DragState.Panning:
                _pan.X += e.X - _lastMousePos.X;
                _pan.Y += e.Y - _lastMousePos.Y;
                Invalidate();
                break;

            case DragState.MovingNodes:
                if (_graph is not null)
                {
                    foreach (var id in _selectedNodes)
                    {
                        var node = _graph.FindNode(id);
                        if (node is not null)
                        {
                            node.X += dx;
                            node.Y += dy;
                        }
                    }
                    Invalidate();
                }
                break;

            case DragState.DraggingWire:
                _dragWireWorldCurrent = worldPos;
                Invalidate();
                break;

            case DragState.BoxSelecting:
                PointF startWorld = ScreenToWorld(_dragStartScreen);
                float x = Math.Min(startWorld.X, worldPos.X);
                float y = Math.Min(startWorld.Y, worldPos.Y);
                float w = Math.Abs(startWorld.X - worldPos.X);
                float h = Math.Abs(startWorld.Y - worldPos.Y);
                _selectionBoxWorld = new RectangleF(x, y, w, h);
                Invalidate();
                break;
        }

        _lastMousePos = e.Location;
    }

    protected override void OnMouseUp(MouseEventArgs e)
    {
        base.OnMouseUp(e);
        PointF worldPos = ScreenToWorld(e.Location);

        if (_state == DragState.DraggingWire && _graph is not null && _dragSourcePort is not null)
        {
            var targetPort = HitTestPort(worldPos);
            if (targetPort is not null && targetPort != _dragSourcePort)
            {
                if (_graph.Connect(_dragSourcePort, targetPort, out _, out _))
                {
                    GraphModified?.Invoke(this, EventArgs.Empty);
                }
            }
        }
        else if (_state == DragState.BoxSelecting && _graph is not null)
        {
            foreach (var node in _graph.Nodes)
            {
                var bounds = GetNodeBoundsWorld(node);
                if (_selectionBoxWorld.IntersectsWith(bounds))
                {
                    _selectedNodes.Add(node.Id);
                }
            }
            SelectionChanged?.Invoke(this, EventArgs.Empty);
        }
        else if (_state == DragState.MovingNodes)
        {
            GraphModified?.Invoke(this, EventArgs.Empty);
        }

        _state = DragState.None;
        _dragSourcePort = null;
        Invalidate();
    }

    protected override void OnMouseWheel(MouseEventArgs e)
    {
        base.OnMouseWheel(e);
        float oldZoom = _zoom;
        float factor = e.Delta > 0 ? 1.15f : 0.85f;
        float newZoom = Math.Clamp(_zoom * factor, 0.25f, 2.5f);

        if (Math.Abs(newZoom - oldZoom) > 0.001f)
        {
            // 以滑鼠當前游標位置為縮放中心進行平移補償
            Point mousePos = e.Location;
            _pan.X = mousePos.X - (mousePos.X - _pan.X) * (newZoom / oldZoom);
            _pan.Y = mousePos.Y - (mousePos.Y - _pan.Y) * (newZoom / oldZoom);
            _zoom = newZoom;
            Invalidate();
        }
    }

    private GraphNode? HitTestNode(PointF worldPos)
    {
        if (_graph is null) return null;
        for (int i = _graph.Nodes.Count - 1; i >= 0; i--)
        {
            var node = _graph.Nodes[i];
            var bounds = GetNodeBoundsWorld(node);
            if (bounds.Contains(worldPos)) return node;
        }
        return null;
    }

    private GraphPort? HitTestPort(PointF worldPos)
    {
        if (_graph is null) return null;
        foreach (var node in _graph.Nodes)
        {
            foreach (var port in node.Ports)
            {
                PointF pt = GetPortCenterWorld(port);
                float distSq = (worldPos.X - pt.X) * (worldPos.X - pt.X) + (worldPos.Y - pt.Y) * (worldPos.Y - pt.Y);
                if (distSq <= (PortCircleRadius + 4f) * (PortCircleRadius + 4f))
                {
                    return port;
                }
            }
        }
        return null;
    }

    #endregion
}

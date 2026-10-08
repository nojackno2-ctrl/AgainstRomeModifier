# Against Rome 地圖編輯器：歷史戰役相機運鏡與過場動畫導演系統架構設計

> **文檔版本**：1.0.0  
> **更新日期**：2026-10-08  
> **模組命名空間**：`AgainstRomeMapEditor.Modules.Cinematics`  
> **核心職責**：為《反抗羅馬》(Against Rome) 歷史戰役製作提供高精度相機樣條飛行、多軌道導演時間軸、3D 視圖即時試飛預覽及原生 BCI 腳本編譯輸出能力。

---

## 1. 系統背景與設計目標 (Executive Summary & Goals)

### 1.1 背景與問題定義
在《反抗羅馬》歷史戰役地圖中，開場運鏡（如相機自遠方雪覆山巔平滑俯衝至玩家大本營）與劇情高潮過場（如羅馬軍團突然越過隘口時的動態追焦特寫）是提升戰役代入感與敘事氛圍的核心要素。然而，原版遊戲引擎僅提供低階腳本指令，且缺乏可視化的路徑編輯與即時試飛工具：
1. **傳統打點極其耗時**：地圖作者必須在二進位腳本中盲猜坐標、手動推算時間間隔，並反覆重啟遊戲觀看效果。
2. **相機運動生硬突兀**：缺乏樣條曲線插值，相機在關鍵點之間只能進行生硬的折線移動或突兀瞬移，視角旋轉（Yaw）常在 $360^\circ$ 邊界發生劇烈倒轉（如 $350^\circ \to 10^\circ$ 旋轉 $340^\circ$）。
3. **敘事元素割裂**：相機運動、劇情對話（字幕）、部隊進軍命令、背景音樂切換分散在各處，無法在統一時間軸（Timeline）上進行多軌同步編排與預覽。

### 1.2 設計目標
本系統為地圖編輯器建立一套專業、高內聚、無 GPU 相依的**歷史戰役相機運鏡與過場動畫導演系統 (Cinematic Camera Track & Cutscene Director)**：
- **CameraTrackSplinePlanner**：基於向心 Catmull-Rom 樣條曲線（$\alpha = 0.5$），支援世界坐標 $(X, Y, Z)$、俯仰角 (Pitch)、偏航角 (Yaw)、焦距/距離 (Zoom)、加減速過渡 (Easing) 之精確平滑插值。
- **CutsceneSequenceCatalog**：將相機軌、字幕對白軌、部隊演員軌、氛圍特效軌整合成統一的多軌導演時間軸，支援版本追蹤、髒標記與 JSON 序列化。
- **CameraFlightPreviewer**：在 `Map3DViewControl` 中提供「一鍵試飛播放 (Play / Pause / Scrub / Stop)」純邏輯狀態控制器，支援即時 OSD 字幕監聽與相機姿態無損還原。
- **CinematicBciCompiler**：匯出為 Against Rome 原生 BCI 鏡頭呼叫指令流（調用原生 `s_lgcSetEnginePos`, `s_lgcSetEngineZoom`, `s_showTextBox`）與相容現有 `ScenarioEvent` 機制之事件集合。

---

## 2. 樣條曲線相機軌跡規劃器 (`CameraTrackSplinePlanner`)

### 2.1 坐標系統與度量規範
- **世界坐標空間**：Against Rome 地圖尺寸為 $64 \times 64$ 圖塊（Tile），一個 Tile 為 $256$ 世界單位，全圖世界空間範圍為 $0 \sim 16384$。
  - $X$：東西軸（$0 \dots 16384$）
  - $Z$：南北軸（$0 \dots 16384$）
  - $Y$：地形高度軸（由 `boden.bmp` 灰階與 `Heightmapstep` 導出）
- **相機姿態角**：
  - **俯仰角 (PitchDegrees)**：俯角 $10^\circ \sim 89^\circ$（原遊戲預設約 $30^\circ$ 2:1 等角俯角；遠景可放寬至 $15^\circ$，俯衝近景可達 $60^\circ \sim 75^\circ$）。
  - **偏航角 (YawDegrees)**：水平方位角 $0^\circ \sim 360^\circ$（原遊戲預設 $45^\circ$ 等角方向）。
  - **焦距 / 距離 (Zoom)**：正交模式下定義可視高度，透視模式下定義視點與焦點距離（預設 $82.0$，原版 1:1 標準）。

### 2.2 向心 Catmull-Rom 樣條曲線數學推導 (Centripetal Catmull-Rom)
傳統均勻 Catmull-Rom 樣條（Uniform Catmull-Rom, $\alpha = 0$）在控制點間距懸殊或急轉彎處極易產生自交迴圈（Self-intersection Loops）與過衝（Overshoot）。為保證電影鏡頭的絕對平滑與優雅，本系統採用**向心 Catmull-Rom 樣條（Centripetal Catmull-Rom, $\alpha = 0.5$）**。

#### 節點時間計算 (Knot Parameterization)
給定連續四個控制點 $P_0, P_1, P_2, P_3$，定義時間節點 $t_0, t_1, t_2, t_3$：
$$t_0 = 0$$
$$t_{i+1} = t_i + \|P_{i+1} - P_i\|^\alpha \quad (\alpha = 0.5)$$

#### Barry-Goldman 金字塔遞迴求值
對於局部插值參數 $t \in [t_1, t_2]$，通過三層凸組合遞迴求解：
$$A_1 = \frac{t_1 - t}{t_1 - t_0} P_0 + \frac{t - t_0}{t_1 - t_0} P_1$$
$$A_2 = \frac{t_2 - t}{t_2 - t_1} P_1 + \frac{t - t_1}{t_2 - t_1} P_2$$
$$A_3 = \frac{t_3 - t}{t_3 - t_2} P_2 + \frac{t - t_2}{t_3 - t_2} P_3$$
$$B_1 = \frac{t_2 - t}{t_2 - t_0} A_1 + \frac{t - t_0}{t_2 - t_0} A_2$$
$$B_2 = \frac{t_3 - t}{t_3 - t_1} A_2 + \frac{t - t_1}{t_3 - t_1} A_3$$
$$C(t) = \frac{t_2 - t}{t_2 - t_1} B_1 + \frac{t - t_1}{t_2 - t_1} B_2$$

數學上已嚴格證明：當 $\alpha = 0.5$ 時，曲線無尖點（Cusps），且當控制點不共線時無局部自相交，保證運鏡軌跡圓潤流暢。

#### 邊界虛擬控制點構造 (Clamped Ghost Endpoints)
對於開放式航跡，若有 $N$ 個航點 $P_0, P_1, \dots, P_{N-1}$，為求解第 0 段（$P_0 \to P_1$）與最後一段（$P_{N-2} \to P_{N-1}$），構造鏡像虛擬端點：
$$P_{-1} = 2P_0 - P_1$$
$$P_N = 2P_{N-1} - P_{N-2}$$
此構造維持起步與收尾處之一階導數連續性（$C^1$ 連續）。

### 2.3 角度連續展開與最短弧插值 (Angle Unwrapping & Shortest Arc Lerp)
相機偏航角存在 $0^\circ \sim 360^\circ$ 週期性。直接數值插值會在航點跨越 $0^\circ$ 時造成錯誤旋轉。
本系統採用兩層保護策略：
1. **全局角度連續展開 (Angle Unwrapping)**：
   在軌跡初始化時，遍歷航點計算相鄰航點的最短有向差值：
   $$\Delta\theta = ((\theta_{i} - \theta_{i-1} + 180) \bmod 360) - 180$$
   $$\theta_i^{\text{unwrapped}} = \theta_{i-1}^{\text{unwrapped}} + \Delta\theta$$
   這使得連續環繞鏡頭（例如俯衝盤旋 $720^\circ$）能被完整記錄與平滑插值。
2. **求值時標準化歸一**：
   插值求得 $\theta(t)$ 後，再透過 $\theta \bmod 360$ 映射回合法的 $0^\circ \dots 360^\circ$。

### 2.4 過渡加減速 (Easing Functions)
航點區間支援 5 種經典電影過渡加減速算法：
- **Linear**：等速直線過渡 $f(t) = t$。
- **SmoothStep**：電影經典 Hermite 平滑 $f(t) = 3t^2 - 2t^3$。
- **EaseInQuad**：起步加速 $f(t) = t^2$。
- **EaseOutQuad**：減速定格 $f(t) = 1 - (1 - t)^2$。
- **EaseInOutCubic**：起步與定格深層慢入慢出。

---

## 3. 過場導演時間軸與序列目錄 (`CutsceneSequenceCatalog`)

### 3.1 多軌道統一時間軸模型 (Unified Timeline Model)
過場序列 `CutsceneSequence` 整合四條核心軌道：

```
時間軸 (Timeline 0.0s ────────────────────────────────────────── TotalDuration)
[相機軌]  Waypoint 0 ────── Waypoint 1 ──────────── Waypoint 2 ────── Waypoint 3
[字幕軌]       |── Subtitle 1 ──|       |─── Subtitle 2 ───|
[演員軌]       Unit: MoveTo ──────>     Unit: AttackTarget ───>
[特效軌]  LetterboxOn                                       FadeOut / Trigger
```

1. **相機軌道 (`CameraWaypoints`)**：驅動焦點位置 $(X, Y, Z)$、姿態角與距離。
2. **字幕對白軌道 (`Subtitles`)**：定義說話者頭銜、台詞文字、語音樣本代碼（`VoiceSampleAlias`）。
3. **部隊調度軌道 (`UnitOrders`)**：指示特定部隊演員執行移動、攻擊、切換陣形或播放動作。
4. **電影氛圍與事件軌道 (`FxEvents`)**：控制寬銀幕黑邊（Letterbox）、黑幕淡入淡出、音效與後續戰役觸發器。

### 3.2 序列資料結構定義
```csharp
public sealed record CutsceneSequence
{
    public string Id { get; init; } = Guid.NewGuid().ToString("N");
    public string Name { get; init; } = "未命名過場序列";
    public List<CameraWaypoint> CameraWaypoints { get; init; } = new();
    public List<SubtitleKeyframe> Subtitles { get; init; } = new();
    public List<UnitOrderKeyframe> UnitOrders { get; init; } = new();
    public List<CutsceneFxKeyframe> FxEvents { get; init; } = new();
    public bool AutoLetterbox { get; init; } = true;
    public bool DisablePlayerControl { get; init; } = true;
    public bool RestoreCameraOnComplete { get; init; } = true;
    public string? OnCompleteTriggerEvent { get; init; }
}
```

### 3.3 目錄模組職責 (`CutsceneSequenceCatalog`)
- 實作地圖編輯器標準 `IEditorModule<IReadOnlyList<CutsceneSequence>>`。
- 提供完整資料一致性驗證（`Validate()`）：
  - 限制單張地圖過場序列上限為 64 個。
  - 檢驗航點坐標是否落在地圖合法空間（$0 \dots 16384$）。
  - 檢驗俯仰角在有效視野範圍（$5^\circ \dots 89^\circ$）。
  - 檢驗字幕長度不超過 1000 字元且持續時間大於 0。
- 支援乾淨的 JSON 序列化與反序列化，供地圖打包器持久化儲存。

---

## 4. 3D 視圖即時運鏡預覽器 (`CameraFlightPreviewer`)

### 4.1 控制器狀態架構
`CameraFlightPreviewer` 採用純邏輯狀態機設計，與具體 GPU Context 完全解耦，具備極高的單元測試覆蓋率與 UI 靈活性：

```
           Play(sequence)
  [Stopped] ────────────> [Playing]
     ^                       │  ^
     │ Stop()        Pause() │  │ Resume()
     │                       v  │
     └─────────────────── [Paused]
```

### 4.2 視角快照與無損還原機制 (Camera State Snapshot & Restore)
- 在進入試飛播放前，預覽器自動拍攝當前 3D 相機姿態快照（`SavedOriginalCameraPose`）。
- 當試飛結束（或點擊停止按鈕時），若啟用了 `RestoreCameraOnComplete`，預覽器觸發姿態更新事件，將 3D 控制項無縫歸位回玩家原本的編輯視角，確保地圖編輯流程不被打斷。

### 4.3 與 `Map3DViewControl` 之整合介面
在 3D 檢視控制項中，預覽器透過事件驅動與渲染管線對接：
1. **每幀推進 (`Update(deltaTime)`)**：
   WinForms Timer 或高精度時鐘每 16~33ms 呼叫 `Update`。
2. **相機同步 (`CameraPoseUpdated`)**：
   ```csharp
   _previewer.CameraPoseUpdated += (_, pose) => {
       _camera.Target = pose.Position;
       _camera.Rotate(pose.YawDegrees - _camera.YawDegrees, pose.PitchDegrees - _camera.PitchDegrees);
       _camera.ZoomTo(pose.Zoom);
       Invalidate();
   };
   ```
3. **OSD 電影字幕與黑邊繪製**：
   在 `Paint` 事件的最後（Post-render pass），若處於試飛狀態：
   - 繪製頂部與底部 12% 高度的純黑 Letterbox 橫條。
   - 於底部黑條中心繪製金黃色說話者姓名與柔白字體對白。

---

## 5. 原生 BCI 鏡頭呼叫腳本編譯器 (`CinematicBciCompiler`)

### 5.1 Against Rome 引擎原生 Native 逆向映射
本系統全面利用已驗證的引擎原生腳本函式（摘自 `Against_Rome.exe` 與 `docs/reverse-engineering/script-natives.md`）：

| 原生函式名 | 原生簽章 | 虛擬機位址 | 功能說明 |
|---|---|---|---|
| `s_lgcSetEnginePos` | `v(ddd)` | `0x54c400` | **核心鏡頭控制**：設定遊戲相機世界坐標 $(X, Y, Z)$ |
| `s_lgcSetEngineZoom` | `v(d)` | `0x54c620` | **鏡頭縮放**：設定視野焦距等級 |
| `s_centerToPos` | `v(ii)` | `0x51fcb0` | 引擎平滑滑行對焦至坐標 $(X, Z)$ |
| `s_showTextBox` | `i(ii)` | `0x521f10` | 呼叫遊戲原生對話框或劇情字幕 |
| `s_playVoiceSample` | `i(i)` | `0x521fb0` | 播放劇情語音或號角樣本 |
| `s_conMoveTo` | `i(iiiiiiiii)` | `0x5345c0` | 下令部隊演員向目標點進軍 |
| `s_conWaitTime` | `i(iiiiii)` | `0x534cb0` | 腳本時間軸等待指定毫秒 |

### 5.2 雙模式編譯管線

#### 管線 A：相容模式 (`ScenarioEvent` Pipeline)
針對現有地圖發布需求，將過場序列依時間延遲自動切分，轉譯為標準 `ScenarioEvent` 動作清單。此模式不依賴未公開的自訂 opcode，能 100% 透過 `ScenarioEventCompiler.Inject` 注入遊戲 `ak_level.bci` 主等待迴圈中安全執行。

#### 管線 B：原生 BCI0 字節碼串流 (Native BCI Script Generation)
產生完整的 IPR 原生過程代碼 `void cutscene_{id}_main()`，按時間採樣生成階梯呼叫指令：
```c
// 範例：生成的原版 IPR 呼叫腳本
void cutscene_ambush_intro_main()
{
    call s_disableGUI(1); // 鎖定玩家輸入

    // --- [時間戳記 0.00s] ---
    pushd 4096.0; pushd 120.0; pushd 2048.0;
    call s_lgcSetEnginePos; // 原生 0x54c400
    pushd 82.0;
    call s_lgcSetEngineZoom; // 原生 0x54c620
    pushstr "[阿爾米紐斯] 伏擊隊伍已各就各位！";
    push 0;
    call s_showTextBox; // 原生 0x521f10

    push 3000;
    call s_conWaitTime; // 等待 3 秒

    // --- [時間戳記 3.00s: 俯衝至山隘] ---
    pushd 6144.0; pushd 40.0; pushd 5120.0;
    call s_lgcSetEnginePos;
    pushd 60.0;
    call s_lgcSetEngineZoom;

    call s_disableGUI(0); // 歸還控制權
}
```

---

## 6. 核心資料結構與演算法 C# 原型骨架

### 6.1 `CameraTrackSplinePlanner.cs` (核心演算法骨架)
```csharp
public sealed class CameraTrackSplinePlanner
{
    private readonly List<CameraWaypoint> _waypoints = new();
    private readonly List<float> _cumulativeTimes = new();
    private readonly List<float> _unwrappedYaws = new();

    public float TotalDuration => _cumulativeTimes.Count > 0 ? _cumulativeTimes[^1] : 0f;

    public CameraPose Evaluate(float timeSeconds)
    {
        if (_waypoints.Count == 0) return DefaultPose(timeSeconds);
        if (_waypoints.Count == 1 || timeSeconds <= 0f) return FirstPose();
        if (timeSeconds >= TotalDuration) return LastPose();

        int seg = FindSegment(timeSeconds);
        float segDuration = _cumulativeTimes[seg + 1] - _cumulativeTimes[seg];
        float rawT = (timeSeconds - _cumulativeTimes[seg]) / segDuration;
        float easedT = ApplyEasing(rawT, _waypoints[seg + 1].Easing);

        var (p0, p1, p2, p3) = GetSplineSegmentPositions(seg);
        Vector3 pos = EvaluateCentripetalCatmullRom(p0, p1, p2, p3, easedT);
        float pitch = MathUtilLerp(_waypoints[seg].PitchDegrees, _waypoints[seg + 1].PitchDegrees, easedT);
        float yaw = MathUtilLerp(_unwrappedYaws[seg], _unwrappedYaws[seg + 1], easedT);
        float zoom = MathUtilLerp(_waypoints[seg].Zoom, _waypoints[seg + 1].Zoom, easedT);

        return new CameraPose(pos, pitch, NormalizeAngle(yaw), zoom, timeSeconds);
    }
}
```

### 6.2 `CameraFlightPreviewer.cs` (即時預覽控制器骨架)
```csharp
public sealed class CameraFlightPreviewer
{
    public CameraPreviewState State { get; private set; } = CameraPreviewState.Stopped;
    public float CurrentTime { get; private set; }
    public event EventHandler<CameraPose>? CameraPoseUpdated;
    public event EventHandler<SubtitleKeyframe?>? ActiveSubtitleChanged;

    public void Play(CutsceneSequence sequence, CameraPose? originalPose = null)
    {
        _activeSequence = sequence;
        _splinePlanner = new CameraTrackSplinePlanner(sequence.CameraWaypoints);
        _savedOriginalCameraPose = originalPose;
        CurrentTime = 0f;
        State = CameraPreviewState.Playing;
        EvaluateFrame(0f);
    }

    public void Update(float deltaTimeSeconds)
    {
        if (State != CameraPreviewState.Playing) return;
        CurrentTime += deltaTimeSeconds * PlaybackSpeed;
        if (CurrentTime >= TotalDuration) { Stop(restoreCamera: true); return; }
        EvaluateFrame(CurrentTime);
    }
}
```

---

## 7. 測試策略與驗證清單

### 7.1 單元測試矩陣 (`AgainstRomeMapEditor.Modules.Tests.Cinematics`)
- **樣條插值與端點驗證**：
  - 空控制點與單一控制點邊界安全降級。
  - 控制點精確通過性（$t = t_i$ 時距離誤差 $< 0.1$ 單位）。
  - 角度繞圈過渡測試（$350^\circ \to 10^\circ$ 經過 $0^\circ$，不逆轉 $340^\circ$）。
  - 加減速曲線（SmoothStep, EaseInQuad, EaseOutQuad）邊界與中點驗證。
- **過場序列目錄驗證**：
  - 坐標越界攔截（超出 $0 \dots 16384$ 拋出 `InvalidDataException`）。
  - 髒標記（`IsDirty`）精準追蹤。
  - JSON 序列化與反序列化 round-trip 等價性。
- **即時預覽器驗證**：
  - 播放時序推進與姿態事件觸發。
  - 試飛結束時原相機姿態精確還原。
  - 時間軸任意跳轉（`ScrubTo`）精確度。
  - 字幕區間動態進入與退出事件通知。
- **BCI 編譯器驗證**：
  - 產生的 ScenarioEvent 時間順序單調遞增。
  - 產生的 IPR 腳本字串包含所有關鍵 Native 呼叫與正確參數順序。

### 7.2 未來演進方向
1. **相機震屏 (Screen Shake Track)**：支援戰役中巨石投石機砸落或山崩時的相機震顫效果。
2. **注視點追焦 (Look-At Target Tracking)**：讓相機沿樣條飛行的同時，鏡頭中心強制鎖定特定英雄或首領部隊移動。

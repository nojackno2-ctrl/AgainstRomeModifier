# 部隊生成隊形：原生取樣算法與截圖證據

2026-10-07 Codex。僅使用 repository EXE 與指定 TEMP 離線副本；未讀取遊戲安裝目錄、未提交或變更 Git 歷史。依本次「只新增指定檔案」限制，本文件兼記本次里程碑，未修改 AI_HANDOFF.md。

## 狀態與缺口

- **static-verified**：ScenarioSpawn 生成參數、formDef=0、1–20 人上限；formatio.dat 保存格式；formdef.dau 幾何欄位；成員線段取樣、spacing 倍率、世界旋轉；旗幟額外建立。
- **experimental**：game-units.png 的旗幟原點與九個清楚士兵錨點，以原 ALR 樣板配準；第十個被旗幟遮擋，只能保留低信心候選。
- **candidate**：從同一截圖反推、量化的十人偏移（下表）。這是校準資料，**不是獨立驗證原生預設隊形**。
- **rejected**：直接把 Count 10 畫成置中的四欄、128 間距、逐列填滿矩形；最佳一對一配對 RMS 仍為 52.093 px。
- **尚未完成**：指定離線副本沒有 `SYSTEM/DATA_MP/DEFAULTS/formdef.dau`。因此不能確認 id 0 名稱、原始線段內容、任意 N 的實際列／欄數及固定相鄰間距，也不能提供獨立的原生十人座標預測誤差。已向使用者詢問離線副本路徑。

純函式實作接受外部定義，沒有硬編碼假定的平方根／四欄算法；尚未接到編輯器繪製流程。取得 formdef.dau 後，解析 id 0 即可沿已實作算法算出每個 N 的偏移，再以本截圖驗證。不得以測試通過宣稱缺失的 default 資料已解決。

## 證據來源

- `re_workspace/Against_Rome.exe` SHA256：`6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`。
- `%TEMP%/ArmGameCompare_20261007/ENDL_005/DATA/formatio.dat` SHA256：`04958fa1b0f78f4d94c2587b2092873daab4ad3d0b87bcd8d7bcbcb016352da1`。
- ENDL_000、ENDL_005 兩份 formatio.dat 解壓 payload 完全相同，皆為 350008 bytes，全部槽位 inactive。
- `%TEMP%/ArmGameCompare_20261007/game-units.png`（1024×768）SHA256：`3dd98158f0787bc94e3ea756e31a3c5d8e1ec1d404fc0f0af1c79ecf296acc18`。
- `%TEMP%/ArmNativeAssets_20261007/{alr.dat,objdef.txt,cl_alr.txt,cl_epara.txt}`。
- `%TEMP%/ArmInGameEvidence_20261007/arm_scenario.json` 核對 GER_INF01、Count 10、Angle 0、(9856,9344)。未引用其中的個人 runtime ID。

已讀 AGENTS.md、AI_HANDOFF.md、reverse-engineering/README.md；搜尋 docs、src.Shared、Modules、tools/re 的 `Formation|formatio|anzv|SpawnUnit`，參考 endless-mode-ai.md、exe-functions.md、native-scene-rendering.md、colors-detail-animation.md、map-editor-spec.md、map-editor-user-guide.md。Git 起始 HEAD `876e9d9`，分支「主要開發」，原有未追蹤 `.claude/`，沒有 tracked diff；保留既有及其他代理工作。

## 生成路徑與旗幟

`src.Shared/Scripting/LevelScriptInjector.cs`：

```text
s_createUnitAndMems(&obj,&odesc,team,unitType=1,formDef=0,unused=1,
                   angle(double),x,z,memberAlias,count,packHorses=0,life=100,morale=100)
```

- `0x52A787` 註冊名稱（字串 `0x600639`）到 callback `0x52A020`，轉入 `0x524530` → `0x5245D0`。
- `0x5247A1` / `0x5247AB`：軍事成員上限 20；另一 unitType 的上限 4，不能混用。
- `0x524673..0x5246DF` 建立士兵並收集成員；packHorses 是另一路，編輯器目前傳 0。
- `0x524809` → `0x5243A0`；`0x52442E` 額外建立旗幟；`0x524446` 加入成員關聯；`0x52445C` → `0x528700` 設定隊形。旗幟不是 Count 的第十一名士兵。
- `0x528731..0x528750` 初始 spacing float = `0x43A00000` = **320.0**；`0x528779` 套用 formDef；`0x528783` 更新狀態。
- `0x54B901..0x54B90C` 明確允許 formDef 0；其名稱不能僅憑 ACT_Formation_Haufen 等 UI 字串判定。
- 截圖旗幟是 objdef 426 `VerGerKamIco00_Kampf_Icon`，ALR 508 `vegeic00.alr`。旗幟原點為 spawn 世界點，螢幕配準 `(512,245)`，NCC **0.994965**。ALR AnchorY=81、圖高54，不能用圖形底端當地面原點。
- 領隊是否對應某個成員索引、旗幟視覺變體與其他民族／騎兵 spacing 尚未驗證。不添加假想 leader offset。

## formatio.dat：保存 runtime 狀態，不是座標表

PFIL header 64 bytes，offset 16 的解壓大小 350008。payload little-endian：

| payload offset | 內容 | 證據 |
|---|---|---|
| 0 | int32 version=1 | `0x48D105..0x48D117` |
| 4 | int32 slotCount=14000 (`0x36B0`) | `0x4898EA..0x4898F1` |
| 8 + i×15 | uint8 active, int16 formDef, float32 spacing, int32 unknownA, int32 unknownB | `0x48D1B4..0x48D220`；writer `0x4898FD..0x48994A` |
| 8 + N×15 | N×int32 更新／等待狀態 | `0x48D18E..0x48D19E`、`0x4BAE0A..0x4BAE25` |
| 8 + N×19 | N×int16 前次 formDef | `0x48D23C..0x48D24C`、`0x4BAAFA` |
| 8 + N×21 | N×float32 前次 spacing | `0x48D260..0x48D270`、`0x4BAB01` |

總長 `8 + 25×14000 = 350008`；首筆 `(0,-1,1.0,0,1000)`，三個尾段首值 `(0,-1,1.0)`。記憶體是 28-byte stride、base `0x11B0294`，檔案是欄位壓緊後 15-byte records 加三個分離陣列；不能直接 memcpy 或以 28-byte 讀檔。

`0x48EF6F` → `0x48D0F0` 載入；`0x4898A4` 使用 `DATA\\formatio.dat` 字串（`0x5F4052`）保存。先前 exe-functions.md 的 `0x73D070/0x73D078` 屬於 ress／派系預設定義選擇，不是這份 runtime 座標表。

## formdef.dau 幾何與 count 算法

`0x480D71` → `0x4D2DC0` 讀 formdef.dau，`0x4D2E03` 解析 `[FormationDefault]`，callback `0x4D2A10`。記憶體 base `0x11A61A4`，每定義 stride `0x19C`，id 範圍 0..99。

| CSV index（0-based） | 幾何存放 |
|---|---|
| 0 | 定義 id |
| 1 | base+0 byte（非零有效；其他語意未命名） |
| 2 | base+2 int16：線段數 S |
| 3+4j | base+4+4j：startX |
| 4+4j | base+0xA4+4j：startZ |
| 5+4j | base+0x54+4j：endX |
| 6+4j | base+0xF4+4j：endZ |

parser `0x4D2A9F..0x4D2B69` 讀 20 組四浮點數；其餘欄位未映射 UI。`FormationLayoutDefinition.FromFormDefText` 只讀上述欄位，預設 id 0；外部先解壓 PFIL、解碼文字，函式本身不碰檔案。

對目前編輯器無馱馬的部隊，N 個成員索引 i=0..N−1：

```text
q = S / N
p = (i + 0.5) * q
j = floor(p)
u = p - j
local = (1-u) * segment[j].start + u * segment[j].end
x = -local.X * spacing
z =  local.Z * spacing
theta = radians(90 - Angle)
worldX = x*cos(theta) - z*sin(theta)
worldZ = x*sin(theta) + z*cos(theta)
```

證據：`0x4BAB0F..0x4BAB13` 比例；`0x4BAEDA..0x4BAEFF` 半間隔；`0x4BAF05` floor helper `0x419B40` → `0x5C5CCF`；`0x4BAF29..0x4BAF53` 線段插值／spacing；`0x4BB066..0x4BB094` X 反射與 90−Angle；`0x49C350..0x49C3A4` 旋轉。`0x5C5D46` fcos、`0x5C5D50` fsin；`0x5F4BAC`=π/180。

因此任意 N 的列／欄數取決於 **S 與原始線段內容**，不必等於 ceil(sqrt(N))。320 是整個定義的縮放倍率，**不是已確認的相鄰間距**。`FormationLayout.Create` 返回連續世界偏移；原生另有 `0x4BB0AF..0x4BB0EC` 整數目標、移動及碰撞步驟，本函式不模擬它們。

cl_epara：RotationFaktor=500.0、SpeedFaktor=0.7、PathDepth=2、CollisionWaitTime=150，是旋轉／移動／碰撞追趕參數，不能把 500 或 0.7 當作靜止排列間距。

## 截圖校準與誤差

ALR `gerinf01.alr`，動畫0、方向14、team palette0；逐一搜尋24格，最佳皆為 frame16。以下位置是 **ALR 地面錨點**，不是用陰影中心或武器尖端量測。投影：`screen=(512,245)+(0.5*(dx-dz),0.25*(dx+dz))`。

| 士兵 | 校準候選世界 dx,dz | 預測 px | 量測 px | NCC | 距離誤差 px |
|---|---|---|---|---|---|
| 1 | -64,192 | 384,277 | 384,277 | .989274 | 0 |
| 2 | -64,64 | 448,245 | 448,245 | .988933 | 0 |
| 3 | 128,192 | 480,325 | 480,325 | .988879 | 0 |
| 4 | 64,64 | 512,277 | 512,277 | .988430 | 0 |
| 5（遮擋） | -128,-128 | 512,181 | 512,181 | .686276 | 0（低信心） |
| 6 | 0,-64 | 544,229 | 545,229 | .989702 | 1 |
| 7 | -64,-192 | 576,181 | 576,181 | .987901 | 0 |
| 8 | 128,-64 | 608,261 | 608,261 | .989100 | 0 |
| 9 | 64,-192 | 640,213 | 640,213 | .989036 | 0 |
| 10 | 192,-128 | 672,261 | 672,261 | .989850 | 0 |

十點校準 RMS=**0.316 px**、平均0.1、最大1；排除遮擋點，九點 RMS=**0.333 px**。這些偏移由同一畫面反推後量化，因此誤差只證明配準／64單位量化近似，**不能證明 default formation 算法**。遮擋位置不可當作完整獨立觀測；第6點的1px差可能是位置／取整／運動，沒有足夠證據選定原因。

作為反例，四欄、三列、localX=-192,-64,64,192、localZ=-128,0,128、前十格逐列填入；用已知 Angle0 旋轉及旗幟原點，最小總平方距離一對一配對 RMS=52.093 px。這只拒絕該特定矩形假設，不排除其他隊形或生成後收隊過渡。

## 重現、驗證與下一步

`tools/re/formation-probe/`：probe.py（PFIL 解壓／指定 EXE 範圍反組譯）、C# ALR exporter、match.py（標準函式庫 PNG 解碼與 masked NCC）、compare_grid.py（拒絕特定矩形假設）。生成的資產／build output 不應提交；probe 使用 explicit paths，不搜尋或讀取遊戲安裝目錄。

```powershell
$env:PYTHONPATH="$env:TEMP/arm-re-python"
python tools/re/scan_native_scene.py re_workspace/Against_Rome.exe --target 0x5f4052 --target 0x600639
python tools/re/formation-probe/probe.py re_workspace/Against_Rome.exe --range 0x4baeda:0x4baf57 --range 0x4bb066:0x4bb099
python tools/re/formation-probe/probe.py "$env:TEMP/ArmGameCompare_20261007/ENDL_005/DATA/formatio.dat"
$env:DOTNET_ROLL_FORWARD='Major'
# 選新的 TEMP 目錄；exporter 拒絕既有輸出目錄。
dotnet run --project tools/re/formation-probe -c Release -p:UseAppHost=false -- "$env:TEMP/ArmNativeAssets_20261007/alr.dat" "$env:TEMP/ArmFormationScreenshot_NEW"
python tools/re/formation-probe/match.py "$env:TEMP/ArmGameCompare_20261007/game-units.png" "$env:TEMP/ArmFormationScreenshot_NEW"
python tools/re/formation-probe/compare_grid.py
```

已驗證：`DOTNET_ROLL_FORWARD=Major`；`dotnet build AgainstRomeModifier.slnx -c Release -p:UseAppHost=false`：0警告／0錯誤；`dotnet test tests/AgainstRomeMapEditor.Modules.Tests -c Release --no-build`：246通過／0失敗／0略過，新增 FormationLayout 20 cases。其他代理在本次期間提交 local lights（HEAD變為 cac6c7e）；因此重新 build/test，以上為合併工作目錄的最後結果。本代理未執行任何 commit/push。測試涵蓋 midpoint、跨線段邊界、N=0/1/20、四方向與角度繞回、兩軸手性、spacing倍率、不可變 snapshot、InvariantCulture 解析及非法輸入。幾何 fixture 是合成定義，沒有偽裝為原版 id0。

下一步：取得離線 formdef.dau，核对 id0 的 S／20組線段，列出 N=1..20 的實際點分布；用同一旗幟原點投影並比較九個清楚錨點，第十點另取不遮擋截圖。再驗證不同角度、步兵／騎兵、不同人數與生成後穩定狀態，確認是否需要成員排序、整數目標或碰撞修正。補齊前不宣稱「預設隊形已還原」。

# Against Rome 無盡模式地圖編輯器 — 實作任務規格書

> 本文件是自足的實作規格:實作者只需要本文件 + 本 repo 即可完成開發,不需要其他對話上下文。
> 語言慣例:說明用繁體中文,程式識別字/檔名/技術名詞保留英文。

---

## 0. 任務摘要

為 2004 年遊戲《Against Rome》建立一個「世紀帝國式」的**無盡模式(Endless)地圖編輯器**,讓玩家能以現有地圖為範本建立、編輯自己的 ENDL 地圖。

> 2026-07-14 介面語意修正：目前可載入的建立流程是**完整複製原版無盡地圖範本**，按鈕必須標示為「從無盡範本建立」，不得稱為空白地圖。真正的空白地圖需要同步產生或重建高度、碰撞與 `DATA` cache；在這些格式完成受控遊戲內驗證前，「新建空白地圖」只能顯示尚未安全支援的說明，不得用清空 SDL／`boden.txt` 的方式偽裝成空白。

**架構決策(已由需求方確定,不可更改):**

1. 地圖編輯器是**獨立程式**(獨立 WinForms EXE),與現有修改器 `AgainstRomeModifier` 互不干擾。
2. 修改器只新增一個「**地圖管理**」分頁,內含地圖清單與「啟動編輯器」按鈕;按下按鈕才啟動編輯器程式。
3. 分三個 Phase 交付;Phase 1 只使用**已完全理解的檔案格式**(全部列於本文件第 2 章),不需要任何逆向工程。

**3D fallback 診斷要求（2026-07-14）：**離線 3D 缺少 `boden.bmp`、無法載入 `floortex.dat`，或 OpenGL 3.3 context／shader 編譯連結失敗時，程式應停用 3D、保持 2D 可用，並在「3D 診斷」中提供可複製的實際原因、資源路徑與 OpenGL/OS 資訊；不得再只做無原因的靜默 fallback。

**關鍵可行性事實(已調查確認):**

- 遊戲**沒有**內建編輯器。
- 遊戲以檔案系統掃描列舉地圖:EXE 內含 `FindFirstFile("MAPS\*.*")` 掃描模式與 `ENDL_%03ld` printf 格式字串 → 新建 `MAPS/ENDL_005/` 資料夾**很可能**自動出現在無盡模式選單(此假設需第一次實作後由使用者進遊戲驗證;若失敗,fallback 方案見 §6.3)。
- 地圖的「材質繪製」「環境參數」「聚落擺放」「文字」全部是**文字格式**(見第 2 章),Phase 1 即可編輯;只有地形高度等 BMP/二進位圖層語意未知,留待 Phase 3。

---

## 1. 專案結構與整合方式

### 1.1 現有 repo 概況

- 主專案 `AgainstRomeModifier.csproj`(repo 根目錄):C# / .NET 8 WinForms、x64、`Nullable=enable`,原始碼在 `src/`。
- 測試 `tests/AgainstRomeModifier.Tests/`(xUnit,`InternalsVisibleTo` 已開)。
- 解決方案檔:`AgainstRomeModifier.slnx`(新式格式)。
- repo 內含一份**唯讀**的原版遊戲檔案 `遊戲原始檔案/`(用於測試 fixture 與格式參考,絕不可修改)。
- CI:`.github/workflows/ci.yml`,windows-latest,`dotnet build` + `dotnet test`。

### 1.2 新增專案

| 專案 | 類型 | 說明 |
|---|---|---|
| `src.Shared/AgainstRome.Shared.csproj`(或等效路徑) | classlib, net8.0 | 兩程式共用的核心程式碼(見 1.3) |
| `src.MapEditor/AgainstRomeMapEditor.csproj` | WinExe, net8.0-windows, x64, WinForms | 地圖編輯器本體 |

兩者都加入 `AgainstRomeModifier.slnx` 與 CI 的 build 步驟。編輯器 EXE 建置後應輸出到修改器可預期的相對位置(建議:與修改器 EXE 同資料夾,檔名 `AgainstRomeMapEditor.exe`)。

### 1.3 程式碼共用策略

把以下**與 UI 無關**的既有類別從主專案移入 `AgainstRome.Shared`(namespace 可保留 `AgainstRomeModifier` 以減少改動,或建新 namespace 並修 using):

- `src/Core/GameLZSS.cs` — PFIL@ LZSS 壓縮/解壓(格式核心,見 §2.1)。
- `src/Core/Services/SafeFileWriter.cs` — 原子寫入(temp + `File.Replace`,3 次重試,內容相同跳過)。
- `src/Core/FileRollbackScope.cs` — 跨多檔交易(`TrackFile` → `Commit`,例外時自動還原)。
- Windows-1251 編碼常數(現在散在 `src/Core/Patches/PatchText.cs` 的 `GameEncoding`)。

**硬性要求:搬移後修改器行為不得改變**,`dotnet test` 現有 ~98 個測試必須全綠。修改器專案改為引用 Shared 專案。

### 1.4 修改器側:新「地圖管理」分頁

修改器 UI 是 partial class `ModifierForm`(`src/UI/ModifierForm.*.cs`),主分頁用自訂 `ModernTabControl`,分頁以 `ShowTabPage(TabPage)`(`src/UI/ModifierForm.cs` 約 :379)延遲載入。新增:

- 新 partial 檔 `src/UI/ModifierForm.MapManager.cs`:分頁「地圖管理 / Map Manager」。
- 內容:
  - **地圖清單**(ListView,大圖示):每張 `MAPS/ENDL_*` 地圖一項,顯示 `minimap.bmp` 縮圖、顯示名稱(讀 `TEXT/US/briefing.put` 的 `briefing_titel_1`,見 §2.5)、槽位編號、「自製」標記(有 `.arm_custom_map` 標記檔者)。
  - **「啟動編輯器」按鈕**:`Process.Start` 啟動 `AgainstRomeMapEditor.exe`,命令列參數:`--game "<gamePath>" [--map ENDL_005]`(gamePath 來自現有 `GetGamePath()`,`src/UI/ModifierForm.Data.cs:23`)。找不到 EXE 時顯示友善錯誤。
  - **「刪除自製地圖」按鈕**:僅自製圖可用,走 §4.2 的三重防護,需二次確認對話框。
- 所有 UI 字串走現有 `Loc.Get` 雙語機制(zh-TW / EN),字串加在 `src/Core/Localization.cs` 與 `src/UI/` 的 Localization 部分。

### 1.5 修改器側:既有程式碼的必要調整

1. **`src/Core/EndlessAi/EndlessAiOrchestrator.cs`**(修改器的無盡模式 AI 修補協調器)目前硬編碼 5 張圖:
   - `ResolvePaths`(約 :211-224):`ak_level.bci` 分支用 `for (int i = 0; i < 5; i++)` → 改為 `Directory.GetDirectories(mapsPath, "ENDL_???")` 動態列舉(SDL 分支 :235-244 已經是動態掃描,照它的做法)。
   - `GetExpectedFileCount`(約 :255-263):固定回傳 5(ak_level.bci)與 42(SDL)→ 改為依 gamePath 動態計算(需改簽章傳入 gamePath;呼叫點在 `DetectModule` :278)。
   - 效果:玩家自製圖會自動被無盡模式 AI 修補涵蓋(剛複製出的圖內容與原廠相同,Detect 狀態天然一致)。
2. **`src/Core/Services/BackupManager.cs`** 會遞迴掃描 `MAPS/**/team.dat` 建備份基準。規範:**自製圖(有 `.arm_custom_map` 標記的資料夾)不進備份基準、不產 `.bak`**;掃描時排除之。完整還原時提供「保留自製地圖」選項(預設保留),不保留則呼叫 §4.2 的刪除流程。
3. 全庫 grep `ENDL_` 確認沒有其他硬編碼 5 張圖的假設。`Map/MaxPopulationFeature`(套用到所有 `MAPS/**/team.dat`)對自製圖生效是**預期行為**,不需排除。

---

## 2. 遊戲地圖檔案格式參考(自足,已驗證)

### 2.1 PFIL@ 容器格式(LZSS 壓縮)

許多遊戲檔案包在自訂容器裡。判別:檔案前 4 bytes = `50 46 49 4C`(`"PFIL"`,第 5 byte 為 `40` = `'@'`)。

- **64-byte header**,已知欄位:
  - offset 0x00:magic `PFIL@\0\0\0`
  - offset 0x08:`01 00 00 00`(版本?)
  - offset 0x0C:`02 00 00 00`
  - **offset 0x10:uint32-LE = 解壓後大小**(唯一需要重寫的欄位)
  - offset 0x20 起:內部指標/雜訊(依檔案而異,原樣保留即可)
- **64 bytes 之後**是 LZSS 壓縮資料:4096-byte 環狀視窗,初始化為前 0xFEE(4078)個 `0x20` + 最後 18 個 `0x00`;控制 byte 每 bit 指示 literal(1)或 2-byte 字典指標(0);指標編碼 `offset = p1 | ((p2 & 0xF0) << 4)`、`length = (p2 & 0x0F) + 3`。
- **不要重新實作**:直接用 Shared 的 `GameLZSS.DecompressPfil(byte[])` 與 `GameLZSS.CompressPfil(byte[] payload, byte[] origHeader)`。`CompressPfil` 需要**原始檔的 64-byte header** 當模板(它只改 0x10 的大小欄位)— 所以讀檔時務必保留原始 bytes。round-trip 已被現有測試驗證與遊戲相容。

### 2.2 文字編碼

所有遊戲文字檔(含 PFIL 解壓後的文字)使用 **Windows-1251** 編碼(`Encoding.GetEncoding(1251)`,需 `CodePagesEncodingProvider.Instance` 註冊)。注意:本 repo 附的遊戲版本 `TEXT/US/` 內容實際是俄文文字。**編輯器寫入的文字必須限制在 CP1251 可表示的字元**(拉丁 + 西里爾);UI 應在使用者輸入無法編碼的字元(如中文)時警告(遊戲無法顯示)。

### 2.3 地圖資料夾清單(以 ENDL_000 為例,大小為原版實測)

每張地圖 = `MAPS/ENDL_00N/` 資料夾。遊戲共 5 張原廠無盡圖 `ENDL_000`–`ENDL_004`。

| 檔案 | 大小(bytes) | 格式 | Phase 1 處理 |
|---|---|---|---|
| `boden.ini` | 1,457 | PFIL→INI 文字 | **可編輯**(環境參數,§2.4) |
| `boden.txt` | 7,255 | PFIL→文字 | **可編輯**(每格材質表,§2.7) |
| `preload.ini` | 457 | 純文字 | 原樣複製(`[ObjDefPreLoad]` 城門物件預載清單) |
| `boden.bmp` / `vertex.bmp` / `emboss.bmp` / `smooth.bmp` | 各 198,460 | 24-bit BMP,257×257 | 原樣複製(語意未知,Phase 3) |
| `collision.bmp` / `minimap.bmp` | 各 196,66x | 24-bit BMP,256×256 | 原樣複製;minimap 可當縮圖顯示 |
| `daynight.bmp` | 488 | 小 BMP | 原樣複製 |
| `briefpic.tga` | 1,950,764 | TGA(簡報圖) | 原樣複製;可選:允許替換圖片 |
| `cliprect.dat` / `shadows.dat` / `skydens.dat` / `visible.dat` | 2,698 / 140,553 / 40,184 / 16,036 | 二進位(未知) | 原樣複製(boden.ini 的 `Handle*Map` 鍵暗示這些是預計算快取) |
| `Endlos_{Ger,Hun,Kel,Rom}_Siedlung{1,2}.sdl` ×8 | 887–5,565 | PFIL→INI 文字 | **可編輯**(聚落模板,§2.6);8 檔跨五張原廠圖 byte-identical |
| `DATA/*.dat` ×25 | 93–304,758 | 全部 PFIL 包裝的二進位,**格式未知** | **全部原樣複製**。其中 `DATA/hirarchy.dat` 跨圖完全相同 |
| `SCRIPT/ak_level.bci` | 27,234 | PFIL→BCI0 編譯腳本 | 原樣複製(無盡模式出兵/派系邏輯;不含地圖專屬路徑,複製後照常運作) |
| `TEXT/US/briefing.put` | 1,548 | **純文字**(非 PFIL),CP1251 | **可編輯**(地圖名稱/簡報,§2.5) |
| `TEXT/US/text.put` | 1,216 | 純文字,CP1251 | **可編輯**(任務目標文字) |

注意:`TEXTUREN/`(或 `Texturen/`)子資料夾存在但為空,複製時保留空資料夾。`ENDL_003` 另有額外 SDL(`Siedlung3/4`、`vil.sdl`)與一個重複的 `Boden.bmp` — 複製邏輯必須「複製來源資料夾的**全部**內容」而非白名單,以免漏檔。

### 2.4 `boden.ini` — 環境參數(PFIL→INI)

格式:`[鍵名]` 行(含 `;` 註解)後跟一行值。實測 ENDL_000 完整內容(節錄關鍵鍵;註解為原檔德文):

```ini
[Waterlevel]         ;0..
120
[Heightmapstep]      ;1..
4
[ShadowMeshMode]     ;0=太陽只上下移動, 1=太陽東南西原始移動
0
[WasserTexturName]   ;水面貼圖基底名 name00.bmp,...
wassAW
[CausticTexturName]
caustA
[SkyTexturName]
sky
[RainDropsOnWater]   ;0/1
1
[WaterWarpShift]     ;18(低)..10(高), 14=default
12
[WaterBumpAmplitude] ;0..1024, 256=default
256
[WaterBumpFrequency] ;1..16, 4=default
4
[FlashPropability]   ;每秒閃電數 0..1000
8
[WaterColor]         ;水色 Hex (bgr), default=0xffbf7f
0xffdfbf
[DayStartTime]       ;白天開始(時)
6
[DayEndTime]         ;夜晚開始(時)
20
```

(其餘鍵:`Skydensspread`、`Skydensaccuracy`、`ShadowMeshAccuracy`、`ShadowMeshXZsize`、`ShadowMeshYsize`、`HandleSkyDensMap`、`HandleVisibleMap`、`HandleClipRectMap`、`HandleShadowMeshes`、`FlashObjectDefaultIndex`、`FlashObjectDefault2Index`、`FlashLightDefaultIndex`、`SnowAlrIndex`、`SnowShadowIndex`、`SnowShadowSize`、`HagelShadowIndex`、`HagelShadowSize`、`MoveListAmplitude`、`ShowCollisionMesh` — 以原檔為準,編輯器 UI 至少要露出:Waterlevel、WaterColor、DayStartTime/DayEndTime、RainDropsOnWater、FlashPropability、Water* 系列。)

**編輯規則:只改值行,其他行(含註解、順序、空白)逐字保留**,再以 `CompressPfil` + 原 header 回寫。

### 2.5 `TEXT/US/*.put` — 地圖名稱與簡報(純文字,CP1251)

`.put` 是**未壓縮**純文字,`//` 註解,格式為 `var:名稱 ="字串";`,字串可跨多行串接(相鄰字串字面值自動連接,同 C 語法),`\n` 為換行。實例(briefing.put 節錄):

```c
//Briefing-Texte fuer Szenario ENDL_000 US

var:briefing_titel_1        ="<地圖清單顯示的標題>";
var:briefing_titel_2        ="<副標題>";
var:briefing_text           ="<簡報正文,可多行串接>"
"<續行>";
var:debriefing_text_win     ="<勝利文字>";
var:debriefing_text_loss    ="<失敗文字>";
var:briefing_text_sample_name="ENDL_ALL_00.wav";
var:briefing_text_teamname0 ="<隊伍0名>";
... teamname1..teamname7
```

`text.put` 同語法,變數名為 `endl_000_text_00`..`03`(任務目標四種狀態)。**注意:複製地圖到新槽位時,`endl_NNN_text_*` 的編號是否需要跟著改成新槽位編號未驗證** — 實作時先原樣保留測試;若遊戲內任務目標文字消失,則把變數名中的編號改為新槽位號再測(此為第一個進遊戲驗證項目之一)。

編輯器 Phase 1 需提供:改 `briefing_titel_1/2`(= 地圖顯示名)、`briefing_text`、隊伍名 `teamname0..7`。

### 2.6 `Endlos_*_Siedlung*.sdl` — 聚落模板(PFIL→INI 文字)

無盡模式各派系(Ger/Hun/Kel/Rom ×2 階段)的聚落建築佈局。實測結構(Endlos_Rom_Siedlung1.sdl 節錄):

```ini
[settlement]
name    =Endlos_Rom_Siedlung1
createdt=2003/09/08
createtm=13:22:22
teamsett=-1
refpos  =1632,159,5792        ; 參考點 x,y,z(整數)

[object0000]
name    =MAPS/ENDL_000/Endlos_Rom_Siedlung1.sdl
namedef =BauRomPal02_Palisadenecke   ; objdef 名稱(對照 SYSTEM 的 objdef.dau)
def     =1676                        ; objdef 數字索引
pos     =-320.00,0.00,-512.00        ; 相對 refpos 的 x,y,z(浮點)
team    =8
lprel   =1.00                        ; 生命值比例
mprel   =0.00
nation  =3                           ; 0..3 派系
ruhm    =0.00                        ; 榮譽
moral   =0.00
objdefn0=                             ; 三組 objdef 覆寫欄(通常空)
objdefv0=926298413
objdefn1= / objdefv1=926298413
objdefn2= / objdefv2=926298413
anzv    =0,0,0
resv    =0,0,0,0,0,0                 ; 資源存量
angle   =0.00                        ; 朝向角
formdefn= / formdef =-1
onload  =0

[object0001] ...(依序遞增,四位數零填)
```

Phase 1 編輯操作:改 `team`/`nation`、整體平移(改 `refpos`,物件 `pos` 是相對座標不用動)、增/刪 `[objectNNNN]` 區塊(刪後**重新連續編號**)、改單一物件 `pos`/`angle`/`def`+`namedef`(objdef 名稱清單可解析 `SYSTEM/` 的 `objdef.dau` 取得,repo 的 `ObjdefPatcher.cs` 有現成解析可參考)。`name` 欄位含來源路徑字串(`MAPS/ENDL_000/...`),複製到新槽位時同步改寫為新路徑(保守作法;不確定遊戲是否讀它,但改掉無害)。

### 2.7 `boden.txt` — 每格材質表(PFIL→文字)★ 材質繪製的關鍵

```
[Dimension]
64
64
[Texturen]
4BJ___51
4BJ___52
4UJX__11
...(共 64×64 = 4096 行,每行一個材質 tile 名稱,row-major)
```

地圖是 **64×64 tile 網格**；每 tile 一個 8 字元材質名（對應遊戲材質庫）。但 `boden.txt` 中的名稱是**渲染用 tile**，不能全部直接當成使用者的作畫顏料。靜態檢視 `floortex.dat` 後，`4B?___5?` 可辨識為同一基礎地表的多個自然變體；`4T*`、`4U*`、`L*B*T*` 等則包含邊界、轉角與拼接片；`AA_Brush*` 是筆刷遮罩／筆觸圖樣，不是地表顏色。

玩家介面必須以 `4B?___5?` 的中間代碼分組，但只呈現「草地、沙地、泥土、岩地」等可理解的地表名稱與實際來源縮圖；材質代碼、自然變體、transition tile 與拼接規則全部屬於編輯器內部實作，不提供專家入口。玩家只需要選地表、筆刷大小並在地圖上繪製。同一玩家素材固定使用一張主紋理，避免相鄰格因底層變體而形成深淺棋盤；兩種相鄰地表使用已驗證的 `4Uxy__dV` 規則，在外圍舊地表格自動選擇朝向新材質的半邊／角落過渡片。玩家直接點擊的格子必須成為完整的新材質，輸入層不得因底層 texture 名稱相同而跳過。三種地表同格交會的 `4T` 規則尚未還原，仍須後續受控驗證，但不得要求玩家手動處理。

2026-07-14 玩家實際 3D 截圖已確認：連續繪製同一「草地」會形成一致區域，先前的深淺棋盤與中央缺格均已消失。此驗證只涵蓋單一素材連續繪製；混合素材邊界與 `4T` 三材質交會仍須分別驗證。

#### 2.7.1 自創 authoring format 與 native bake（2026-07-14 架構決策）

玩家不應直接編輯 64×64 `boden.txt` tile。編輯器應建立自己的專案格式（暫定副檔名 `.armmap`），保存比遊戲輸出更高階的資料：每種基礎材質的 weight map、圓形筆刷半徑／硬度、語意地表、場景物件，以及後續開放的高度資料。3D 預覽可直接依權重在 shader 中平滑混合材質，提供接近現代／官方地圖編輯器的繪製體驗。

遊戲不會直接讀取 `.armmap`。按「儲存到遊戲／遊戲測試」時，獨立 baker 必須將 authoring state 編譯為遊戲原生 `ENDL_*`：材質權重轉成 `boden.txt` 可用的 base／`4U`／`4T` tile，物件轉成 SDL，其他圖層只在格式與相依 cache 通過驗證後輸出。編輯器應提供「平滑 authoring 預覽」與「遊戲原生 bake 預覽」的切換，避免 shader 效果與實際遊戲輸出不一致。

**驗收標準：匯出後在原遊戲中的外觀與行為必須與原版地圖同級。** 自創格式與 shader 只是編輯工具，不能成為降低遊戲輸出品質的理由。原生 bake 預覽是編輯器的預設可信畫面；任何材質混合、高度、碰撞或物件功能都必須在原遊戲內驗證後才可標記完成。若現有逆向證據不足以產生原版 transition／cache，就繼續補齊格式，不以硬方格或 editor-only 平滑效果作為最終交付。

目前 `4U` 僅足以編譯兩材質邊界；`4T` 三材質 junction、更多材質同區混合與任意 alpha 混合仍須逆向或明確降級。自創格式能讓真正空白專案立即成立，但在未知的高度、碰撞、`DATA/*.dat` cache 能安全生成前，仍不能把該專案匯出並宣稱為可載入的真正空白遊戲地圖。若改走讓遊戲直接載入 `.armmap`，就需要額外的 EXE/DLL loader patch，風險與維護成本屬另一專案，不是目前預設路徑。

`[Heightmapstep] 4`(boden.ini)× 64 tiles = 256,+1 = 257 → 對應 vertex/boden/emboss/smooth.bmp 的 257×257(頂點網格),collision/minimap 的 256×256(tile 網格)。此對應關係是 Phase 3 逆向的起點。

### 2.8 槽位規則

- 資料夾名 `ENDL_%03d`(EXE printf 字串 `ENDL_%03ld`),原廠佔用 000–004,自製圖從 **005** 起,上限 999。
- 新槽位 = 最小可用編號(填補空洞)。
- 顯示名稱來自 `briefing.put` 的 `briefing_titel_1`,與資料夾名無關。

---

## 3. 地圖編輯器功能規格(分 Phase)

### Phase 1 — 地圖複製與屬性編輯(必要,先交付)

**編輯器程式骨架:**

- `AgainstRomeMapEditor.exe`,WinForms,啟動參數 `--game "<path>"`(必要)與 `--map ENDL_NNN`(可選,直接開啟該圖)。無參數時顯示遊戲路徑選擇(可重用修改器的 registry 偵測邏輯:`HKCU/HKLM\Software\Against Rome` 的 `Path` 值,fallback `C:\Program Files (x86)\Against Rome`)。
- 主視窗:左側地圖清單(同修改器地圖管理頁的資料來源),右側編輯區(分頁:基本資訊 / 環境參數 / 聚落 / 材質(可延後))。

**核心類別(建議放 Shared 或編輯器專案的 Core/):**

| 類別 | 職責 |
|---|---|
| `EndlessMapCatalog` | 掃描 `<gamePath>/MAPS/ENDL_*`;讀顯示名稱、minimap 縮圖、自製標記;`GetNextFreeSlot()` |
| `EndlessMapCloner` | `Clone(sourceSlot, newSlot, newName)`:見下方複製流程 |
| `CustomMapManifest` | 自製圖登記與刪除防護(§4.2) |
| `BodenIniDocument` | boden.ini 讀改寫(§2.4 規則) |
| `BodenTexturesDocument` | boden.txt 讀改寫(64×64 材質陣列) |
| `PutTextDocument` | .put 讀改寫(CP1251、`var:` 語法、保留註解與未改動行) |
| `SdlDocument` | SDL 讀改寫(§2.6) |

**複製流程(必須照此順序,保證無殘留):**

1. 目標暫存資料夾 `MAPS/ENDL_NNN.tmp_arm`,遞迴複製來源圖**全部**檔案(逐 byte,含未知 .dat 與 ak_level.bci)。
2. 改寫新圖的 `briefing.put` 標題為使用者輸入名稱;SDL 的 `name` 路徑欄改為新槽位路徑。
3. 寫入標記檔 `.arm_custom_map`(內容:JSON,含建立時間、來源槽位、工具版本)。
4. 驗證:檔案數與總大小和來源一致(標記檔除外)。
5. `Directory.Move` 改名為 `ENDL_NNN` → 更新 manifest(§4.2)。
6. 任一步失敗:刪除整個暫存資料夾後回報錯誤(新增性操作,rollback = 刪暫存夾)。

**編輯功能:**

- 基本資訊:地圖名(titel_1/2)、簡報文字、隊伍名 0–7。
- 環境參數:§2.4 列出的 boden.ini 鍵(數值用 NumericUpDown / 顏色用色彩選擇器轉 0xBBGGRR hex)。
- 聚落:8 個 SDL 的清單 → 物件表格(namedef/def/pos/angle/team)、整體平移、增刪物件。
- 材質(可作為 Phase 1.5):64×64 網格檢視(每格填色或縮寫),點格子從材質調色盤替換。
- **原廠圖(ENDL_000–004)一律唯讀**,只能「另存為新地圖」;要改原廠圖 = 先複製再編輯。

> 實作狀態（2026-07-14）：屬性面板已支援 `briefing_titel_1/2`、可串接／多行的 `briefing_text`、`briefing_text_teamname0..7`，以及 Waterlevel、WaterColor、DayStartTime、DayEndTime、RainDropsOnWater、WaterWarpShift、WaterBumpAmplitude、WaterBumpFrequency、FlashPropability。寫入仍只允許 marker-backed 自製地圖，並沿用 CP1251、PFIL header 保留與 `FileRollbackScope` 交易。SDL 文件層已通過合成與八個真實聚落檔的 no-op round-trip；場景檢查器現提供受控驗證用的既有物件 `team`／相對 `pos` 暫存編輯與「還原到本次開啟時」，儲存服務會再次驗證 custom marker、`ENDL_005–999`、來源檔名及索引。此功能仍屬遊戲內驗證階段，尚未開放物件增刪、拖曳或宣稱 runtime verified。

**存檔紀律:** 所有寫入經 `SafeFileWriter`;一次「儲存變更」內的多檔寫入包在 `FileRollbackScope`(全部 `TrackFile` → 寫入 → `Commit`)。PFIL 檔回寫必用原始 64-byte header。

### Phase 2 — 視覺化地圖檢視器

- `MapCanvasControl`(double-buffered):疊層渲染 minimap.bmp / boden.bmp / collision.bmp / 材質網格(boden.txt 上色),縮放平移、圖層開關。256 vs 257 尺寸差 = tile 網格 vs 頂點網格,對齊時 tile (i,j) 對應頂點 (i..i+1, j..j+1)。
- SDL 物件疊加:世界座標 ↔ 像素對應需推導 — 方法:取已知聚落 `refpos`(如 1632,159,5792)對照 minimap 上目視位置,用五張原廠圖交叉擬合 scale/原點/軸向(BMP 是 bottom-up,注意 Y 翻轉)。先只做唯讀疊加讓使用者目視驗證,再開放拖曳(拖曳寫回 SdlDocument)。
- 材質繪製 UI 升級:在畫布上直接筆刷塗 boden.txt 材質。

### Phase 3 — 地形高度/碰撞編輯(依賴逆向工程)

逆向任務與驗證方法(**只在自製複製圖上實驗**,由使用者進遊戲目視驗證,絕不動原廠圖):

1. **BMP 圖層語意**:假設 `vertex.bmp`(257²)編碼高度(可能灰階或單通道)、`boden.bmp` 為預烘焙底色、`emboss/smooth` 為光影。驗證:單點/區塊像素修改 → 進遊戲看地形變化;並跨五張圖統計圖層 vs 水域/山脈特徵相關性。
2. **`collision.bmp` 編碼**:改一小塊測單位通行性。
3. **`DATA/*.dat` 權威性**:逐一清空/移除單檔(如 `way.dat`)觀察遊戲重建或崩潰;`hirarchy.dat` 跨圖相同 → 大概率可永遠原樣複製。
4. 成果依序解鎖:高度筆刷(vertex.bmp + Heightmapstep 換算)→ 碰撞編輯 → minimap 自動重繪。
5. 所有逆向成果寫入新文件 `docs/reverse-engineering/map-formats.md`。

---

## 4. 安全與備份規範(硬性規則)

### 4.1 通用

- **絕不**直接用 `File.WriteAllBytes` 寫遊戲目錄 — 一律 `SafeFileWriter` + `FileRollbackScope`。
- **絕不**修改 repo 內的 `遊戲原始檔案/`。
- 測試**絕不**接觸真實遊戲安裝目錄(沿用現有 SyntheticFixture / BackupZipGameFixture 模式)。

### 4.2 自製地圖的登記與刪除三重防護

- Manifest:`<gamePath>/MAPS/arm_custom_maps.json`,記錄工具建立的資料夾清單。
- 刪除一個地圖資料夾必須**同時**滿足:
  1. 在 manifest 清單中;
  2. 資料夾內有 `.arm_custom_map` 標記檔;
  3. 路徑符合 `MAPS[/\\]ENDL_\d{3}$` 且編號 ≥ 005。
- **任何情況下拒絕刪除 ENDL_000–004** 與 MAPS 以外路徑。
- 自製圖不進 BackupManager 備份基準、不產 .bak(§1.5-2)。

---

## 5. 測試清單(xUnit,加入 `tests/AgainstRomeModifier.Tests` 或新測試專案)

全部用合成 fixture(在 temp 目錄組出假遊戲樹;可從 `遊戲原始檔案/MAPS/ENDL_000` 取樣本檔,或用現有 BackupZipGameFixture 模式):

- **MapClonerTests**:`GetNextFreeSlot` 取 005 / 填補空洞;複製後逐 byte 相同(標記檔與改寫檔除外);標記檔與 manifest 正確;模擬中途失敗(鎖檔)→ 無 `.tmp_arm` 殘留、無半成品正式資料夾。
- **BodenIniDocumentTests**:解壓 → 改 Waterlevel → 壓縮 → 再解壓值正確;未改動行(含德文註解)逐字保留;PFIL header 0x10 大小欄正確。
- **BodenTexturesDocumentTests**:4096 格解析、單格替換 round-trip。
- **PutTextDocumentTests**:CP1251 round-trip、改 titel 保留其他變數與註解、多行字串串接解析、不可編碼字元偵測。
- **SdlDocumentTests**:解析 [settlement]/[objectNNNN] 全欄位、改 team / 整體平移 / 增刪物件後重編號 round-trip、PFIL 往返。
- **CustomMapManifestTests**:三重防護各自缺一即拒刪;拒刪 ENDL_000–004;拒刪 MAPS 外路徑。
- **EndlessAiOrchestratorTests(擴充現有)**:fixture 加 ENDL_005 後 `ResolvePaths` 回傳 6 個 ak_level.bci、`GetExpectedFileCount` 動態正確、Detect/Apply 涵蓋新圖;無自製圖時行為與現在完全相同(回歸)。
- **BackupManagerTests(擴充)**:自製圖 team.dat 不進備份基準。
- CI:三個(以上)專案 build + `dotnet test` 全綠(windows-latest)。

---

## 6. 驗收與進遊戲驗證

### 6.1 自動驗證

- `dotnet build -c Release`(x64)無新增警告;全部測試綠。
- 修改器既有功能零回歸(現有測試全綠即視為通過)。

### 6.2 使用者進遊戲手動驗證(關鍵假設,Phase 1 完成後立即做)

1. 用編輯器複製 ENDL_000 → ENDL_005 並改名 → 開遊戲:無盡模式選單**出現第 6 張圖**(驗證檔案掃描假設)。
2. 顯示名稱為新名稱(briefing.put 生效);任務目標文字正常(驗證 §2.5 的 `endl_NNN_text_*` 編號問題)。
3. 新圖可載入、AI 派系正常出兵發展(ak_level.bci 原樣複製有效)、存檔/讀檔正常。
4. 修改器「完整還原」後:原廠五圖完好;自製圖依選項保留/刪除。
5. 改 Waterlevel/WaterColor 後進遊戲目視水位/水色變化;改 SDL 後聚落佈局變化。

### 6.3 Fallback(若 ENDL_005 不出現在選單)

代表遊戲用固定 0–4 迴圈探測而非目錄掃描。改用「**覆蓋槽位**」模式:編輯器把自製圖存放在 `MAPS/ARM_CUSTOM/<名稱>/`(遊戲不讀),「啟用」時備份原廠 ENDL_00N(`.bak` 或搬到 ARM_CUSTOM/_originals)再把自製圖複製進該槽;「停用」時還原。此模式下 manifest 需額外記錄「哪個槽位目前被哪張自製圖佔用」。UI 與文件其餘規格不變。

---

## 附錄 A:調查證據摘要

- EXE 字串(Against_Rome.exe,2,486,272 bytes):`%sENDL_%03ld/TEXT/%s/briefing.put`、`MAPS` + `*.*` + `SCRIPT` + `%s/boden.txt` 字串簇、UI widget `endl_z%02ld`。無編輯器、無 map list 設定檔(root `misc.cfg`、`SYSTEM/start.ini`、`language.ini` 均無)。
- 跨圖 MD5:`DATA/hirarchy.dat` 與 8 個核心 SDL 在 ENDL_000/001/004 完全相同;其餘檔案每圖各異。
- 所有 `DATA/*.dat` 皆 PFIL 包裝(header 實測);`team.dat` 的 header 0x20 簽名字(`8823af02`)與其他 dat(`d8c2ae02`)不同。
- BMP header 實測:boden/vertex/emboss/smooth = 257×257×24bpp,collision/minimap = 256×256×24bpp,五張圖尺寸一致。
- `boden.ini`/`boden.txt`/SDL 內容 = 本文件引用之實際解壓文字(ENDL_000)。
- `.put` 檔無 PFIL header,為純文字。

## 附錄 B:repo 內可參考的既有程式碼

| 路徑 | 用途 |
|---|---|
| `src/Core/GameLZSS.cs` | PFIL 壓縮/解壓(演算法含遊戲相容性注意事項,見檔內註解) |
| `src/Core/Services/SafeFileWriter.cs`、`src/Core/FileRollbackScope.cs` | 寫入管線 |
| `src/Core/Patches/PatchText.cs` | CP1251 `GameEncoding`、CSV 解析 |
| `src/Core/Patches/ObjdefPatcher.cs` | objdef.dau 解析(取 objdef 名稱清單用) |
| `src/Core/EndlessAi/EndlessAiOrchestrator.cs` | 需一般化的 5 圖硬編碼(§1.5) |
| `src/Core/Services/BackupManager.cs` | 備份基準掃描(§1.5) |
| `src/UI/ModifierForm.cs`(:379 `ShowTabPage`)、`src/UI/UIElements.cs` | 分頁接線與自訂 TabControl |
| `src/UI/ModifierForm.Data.cs`(:23 `GetGamePath`、:38 registry 偵測、:66 `LoadTga`) | 遊戲路徑與 TGA 縮圖 |
| `docs/reverse-engineering/file-formats.md`、`data/game_schema.json` | 既有格式文件(本文件為地圖格式的補充) |
| `tests/AgainstRomeModifier.Tests/`(SyntheticFixture、BackupZipGameFixture) | 測試 fixture 模式 |

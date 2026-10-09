# boden.ini 值域與天候預設稽核

日期：2026-10-08；Codex；獨立 worktree `D:\Github\ARM_wt_weather`、分支 `wt/weather`。未存取或啟動遊戲安裝目錄。程式基線 `080ebc9`。

## 樣本範圍與來源

對 TEMP 遞迴搜尋 `boden.ini` 路徑；唯讀解包使用既有 `GameLZSS.DecompressPfil`，以 `BodenIniSerializer.ParseText` 解析，另按 `[section]` 後的數值行統計所有鍵。鍵名、數值與註解是 ASCII，CP1251/1252 對此資料沒有差異。檔案為 PFIL 容器，不能直接當文字讀取。

| TEMP 相對路徑 | 找到的 boden.ini | 採計方式 |
|---|---|---|
| ArmGameCompare_20261007/ENDL_000、ENDL_005 | 2 | 原版基準 |
| ArmBlankAcceptance_20261008_01..06/game/MAPS/ENDL_000、ENDL_005 | 12 | 與基準解包內容完全一致 |
| ArmDemoAcceptance_20261008_01..10/game/MAPS/ENDL_000、ENDL_005 | 20 | 與基準解包內容完全一致 |
| ArmBlankAcceptance_20261008_05..06/game/MAPS/ENDL_007 | 2 | 自訂驗收地圖；亦與基準一致，不能視為額外原版樣本 |
| ENDL_001..004、KAMP*、HIST* | 0 | 搜尋 TEMP 未找到；未去安裝目錄補資料 |

共解析36檔：34份原版槽位副本（僅兩個不同槽位）＋2份自訂槽位副本。其他 TEMP 的工具展示圖、generated candidates、單元測試 fixture 不作原版證據；不能宣稱已核對六張 ENDL 或全部戰役/歷史地圖。重複副本增加可重現性，沒有增加地圖多樣性。

| 基準檔 | PFIL SHA256 |
|---|---|
| ENDL_000 | `7FE0EC46CC9A004D321473035039D363537D8D4FA870BB630D684BAABFEE1C41` |
| ENDL_005 | `561294841851127F1C171DB3ED6AE31A14FC2C756CB25E024EC51921D9787B62` |

36檔解包 SHA256 均為 `B21DB65F91EF6846D43D51A6C98376552C348408477037AF2F6BC6FFC14D1044`。PFIL 外層雜湊不同不代表設定值不同。解包前後的資料與 scratch 程式只留 ignored `obj/weather-audit`；不提交原版檔案。

## 全部鍵與觀察值域

33鍵全部存在於 ENDL_000/005。**沒有任何鍵在這兩張原版樣本之間變動**；每個數值的 min=max，字串僅一個不同值。表中「原檔註解」與「觀察值」分開：註解允許的範圍不等於本次實測引擎安全範圍。現在 `new BodenIniData()` 的全部33個值均與此表觀察值相同。

| 鍵 | 觀察值／min=max | 地圖間變動 | 原檔註解或限制 | serializer 驗證（本次後） |
|---|---|---|---|---|
| `Waterlevel` | `120` | 無 | 非負；無上限資料 | 有限且 >=0；以前未驗證 |
| `Heightmapstep` | `4` | 無 | >=1 | 有限且 >=1；以前僅 >0 |
| `Skydensspread` | `16` | 無 | 4,8,16,32,64,128,256,512,1024 | 此離散集合；以前未驗證 |
| `Skydensaccuracy` | `2` | 無 | 0..2 | 0..2；以前未驗證 |
| `ShadowMeshAccuracy` | `6` | 無 | 0..7；註解預設5 | 0..7；以前未驗證 |
| `ShadowMeshXZsize` | `16000` | 無 | 8192..16384；註解預設12000 | 8192..16384；以前未驗證 |
| `ShadowMeshYsize` | `5000` | 無 | 4000..16000；註解預設9000 | 4000..16000；以前未驗證 |
| `ShadowMeshMode` | `0` | 無 | 0/1 | 0/1（維持） |
| `HandleSkyDensMap` | `1` | 無 | 0/1 | 0/1；以前未驗證 |
| `HandleVisibleMap` | `1` | 無 | 0/1 | 0/1；以前未驗證 |
| `HandleClipRectMap` | `1` | 無 | 0/1 | 0/1；以前未驗證 |
| `HandleShadowMeshes` | `1` | 無 | 0/1 | 0/1；以前未驗證 |
| `WasserTexturName` | `wassAW` | 無 | 貼圖名稱；沒有名稱集合 | 未驗證素材存在性（維持） |
| `CausticTexturName` | `caustA` | 無 | 貼圖名稱；沒有名稱集合 | 未驗證素材存在性（維持） |
| `SkyTexturName` | `sky` | 無 | 貼圖名稱；沒有名稱集合 | 未驗證素材存在性（維持） |
| `RainDropsOnWater` | `1` | 無 | 0/1：水面雨圈開關 | bool（維持） |
| `WaterWarpShift` | `12` | 無 | 0=無水面動態；10..18；註解預設14 | 0 或 10..18；以前8..20且拒絕0 |
| `WaterBumpAmplitude` | `256` | 無 | 0..1024；註解預設256 | 0..1024（維持） |
| `WaterBumpFrequency` | `4` | 無 | 先列1..16，後又說32為細波；矛盾 | 保守保留1..16；32待查 |
| `FlashPropability` | `8` | 無 | 0..1000，註解稱最大降雨強度時每秒閃電數 | 0..1000（維持）；實際單位未驗證 |
| `FlashObjectDefaultIndex` | `803` | 無 | 目錄索引；沒有上下限或關閉值資料 | 不從單一樣本發明索引範圍（維持） |
| `FlashObjectDefault2Index` | `1099` | 無 | 目錄索引；沒有上下限或關閉值資料 | 不從單一樣本發明索引範圍（維持） |
| `FlashLightDefaultIndex` | `6` | 無 | 目錄索引；沒有上下限或關閉值資料 | 不從單一樣本發明索引範圍（維持） |
| `SnowAlrIndex` | `837` | 無 | 目錄索引；沒有上下限或關閉值資料 | 不從單一樣本發明索引範圍（維持） |
| `SnowShadowIndex` | `152` | 無 | 目錄索引；沒有上下限或關閉值資料 | 不從單一樣本發明索引範圍（維持） |
| `SnowShadowSize` | `7` | 無 | 尺寸；沒有範圍資料 | 不從單一樣本發明尺寸範圍（維持） |
| `HagelShadowIndex` | `152` | 無 | 目錄索引；沒有上下限或關閉值資料 | 不從單一樣本發明索引範圍（維持） |
| `HagelShadowSize` | `7` | 無 | 尺寸；沒有範圍資料 | 不從單一樣本發明尺寸範圍（維持） |
| `MoveListAmplitude` | `127` | 無 | 0..127；註解預設64 | 0..127；以前未驗證 |
| `WaterColor` | `0xffdfbf` | 無 | BGR hex；註解預設0xffbf7f | 6位hex，可選0x前綴（維持） |
| `ShowCollisionMesh` | `0` | 無 | 0/1 | 0/1；以前未驗證 |
| `DayStartTime` | `6` | 無 | 日間開始的小時；無範圍明示 | 有限且0..24；小時域編輯器政策 |
| `DayEndTime` | `20` | 無 | 夜間開始的小時；無範圍明示 | 有限且0..24；小時域編輯器政策 |

所有已建模/serializer 產生的原生鍵都在樣本中存在，沒有需要刪除的虛構原生鍵。原本模型缺省的 Skydensspread=0、Skydensaccuracy=0、MoveListAmplitude=0、索引=-1、尺寸=0、RainDropsOnWater=false、FlashPropability=0、WaterWarpShift=14，現均改為實際樣本值。原版已有雪花素材索引也不表示正在降雪；雨圈開關也不等同已確認的全場降雨強度開關。

`UpdateDocument` 只更新原檔存在的鍵，保留未知鍵、註解及排版，不加入缺失區段。`ParseText`/產生新檔只提供值解析/生成，並非無損文字往返；未知區段在新檔生成時仍可保留其值。已修正大小寫已知鍵在 KnownKeys 與 ApplyValue 之間不一致的解析行為。素材索引=-1仍是模型「未指定」慣例，不當作已確認的引擎關閉值；既有 UpdateDocument 對負索引與非正尺寸不回寫，並未新增一個已驗證的停止降雪/關閉素材功能。

## 五種預設的全部原生值比較

表列預設的差異欄位；其餘全部原生欄位使用上表原版基準值，包括水位120、高度步長4、天空/陰影網格及貼圖名。符號 `→` 表示修改前→本次後，單一值表示維持。新的數值是保守編輯器設計選擇，**不是**由原版地圖推導的五種真實天候配置。

| 原生欄位 | clear_sky | storm | sunset_dusk | dense_fog | mountain_snow |
|---|---|---|---|---|---|
| WaterBumpAmplitude | 140→224 | 720→320 | 240→256 | 80→224 | 60→224 |
| WaterBumpFrequency | 3→4 | 9→5 | 4 | 2→4 | 2→4 |
| WaterWarpShift | 14 | 10→12 | 13 | 15→14 | 16→14 |
| WaterColor (BGR) | 0xffdfbf | 0x5a5040→0xe0c8aa | 0x406090→0xe0d0d0 | 0x788a80→0xf0d8bf | 0xf0d8a0→0xffe8cc |
| RainDropsOnWater | 0 | 1 | 0 | 0 | 0 |
| FlashPropability | 0 | 30→8 | 0 | 0 | 0 |
| DayStartTime | 6 | 5.5→6 | 6 | 6 | 7→6 |
| DayEndTime | 20 | 19.5→20 | 19→20 | 20 | 18→20 |
| ShadowMeshMode | 0 | 0 | 1→0 | 0 | 0 |
| FlashObjectDefaultIndex / Default2 | -1/-1→803/1099 | 同左 | 同左 | 同左 | 同左 |
| FlashLightDefaultIndex | -1→6 | 1→6 | -1→6 | -1→6 | -1→6 |
| SnowAlrIndex / SnowShadowIndex / Size | -1/-1/0→837/152/7 | 同左 | 同左 | 同左 | 1/1/2→837/152/7 |
| HagelShadowIndex / Size | -1/0→152/7 | 同左 | 同左 | 同左 | 同左 |
| MoveListAmplitude / Skydensspread / Skydensaccuracy | 0/0/0→127/16/2 | 同左 | 同左 | 同左 | 同左 |

水波幅度限制於原版256附近的224..320（-12.5%..+25%），頻率4..5、warp12..14；各RGB通道距原版 BGR 色碼最多32/255。這個「附近」是明示的預設設計政策，並非遊戲合法值域。雨圈和Flash=0使用原檔註解支持的關閉值。晝夜6/20與ShadowMeshMode=0維持原版樣本；不把「黃昏」標籤當成已設定遊戲時鐘。

## 其餘 WeatherPresets 欄位與 native 的邊界

以下是全部數值型 profile 欄位（名稱、ID、Description 為介面文案），本次不更動。**在33個原生鍵中均沒有對應鍵，無法對照 boden.ini 觀察範圍**；只用於 AtmosphereProfile/AtmosphereLightingBridge 可提供的編輯器效果資料，不新增 fog、wind、exposure 或 precipitation INI 區段。

| profile 欄位 | clear_sky | storm | sunset_dusk | dense_fog | mountain_snow |
|---|---|---|---|---|---|
| FogEnabled | true | true | true | true | true |
| FogStartDistance / EndDistance | 60/180 | 12/60 | 30/110 | 4/38 | 25/95 |
| FogDensity | .20 | .85 | .50 | .96 | .48 |
| FogColor RGB | .72,.82,.95 | .22,.26,.28 | .85,.45,.30 | .68,.73,.76 | .82,.88,.98 |
| SkyScatteringColor RGB | 1.05,1.02,.98 | .50,.55,.60 | 1.20,.75,.45 | .75,.78,.80 | .90,.95,1.15 |
| AmbientTint RGB | 1,1,1 | .65,.70,.68 | 1.10,.85,.70 | .78,.82,.84 | .92,.96,1.10 |
| Exposure | 1 | .75 | .95 | .85 | 1.12 |
| SunLightColor RGB | 1,.98,.92 | .55,.58,.62 | 1.25,.70,.35 | .70,.72,.75 | .95,.98,1.05 |
| SunLightDirection（向量 Normalize 前） | .4,.85,.3 | .2,.95,.2 | .85,.35,.4 | .3,.9,.3 | .5,.75,.4 |
| Precipitation | None | Rain | None | None | Snow |
| PrecipitationIntensity | 0 | .95 | 0 | 0 | .70 |
| WindSpeed | 4 | 22 | 5 | 2 | 12 |
| WindDirection | 45 | 75 | 200 | 90 | 315 |

`MapEditorForm.Weather.cs` 的 toolbar 實際套用並保存只有8個欄位：水色、bump amplitude/frequency、warp、FlashPropability、DayStart/End、RainDropsOnWater。其餘原生欄位（包括素材索引、陰影模式）與全部 profile 視覺值不經這個選單覆寫；水位/高度步長保留。故選「Mountain Snow」**不能據此宣稱已啟用遊戲全場降雪**，選「Dense Fog」不能宣稱寫了遊戲霧參數；「Sunset Dusk」也不改目前遊戲時鐘。此前 handoff 的storm畫面變暗＋雨點屬舊版本的使用者實機證據，本次數值修改後未進遊戲重新驗證。

Bridge 的Flash模擬間隔 `1000 / FlashPropability` 是現有編輯器啟發式；原檔註解描述的是最大降雨強度時的每秒閃電數，兩者不應混稱原生單位。本次加註此差異，保留預覽演算法，不以單一註解改寫成每秒8次爆閃。

## 驗證與未解問題

自足 inline 測試覆蓋33鍵的兩種parser/default生成（含小寫鍵）、更新原檔保留註解/未知區段/不添加鍵、五種preset全原生鍵及附近範圍、warp關閉值與bridge傳值、非有限數值/註解邊界；宿主測試逐一套用五種preset，保存後比對8個欄位並確認高度/水位未改。無遊戲資產加入測試或提交。

- 缺 ENDL_001..004、KAMP/HIST：取得已授權 TEMP 副本後需擴大表格，不能將本次singleton範圍當成所有原版值域。
- WaterBumpFrequency 註解同時出現1..16及32，需追蹤引擎再決定是否接受32。
- FlashPropability 的實際單位、降雨強度控制、雪/雹的啟用流程及素材索引在其他版本/地圖是否相同尚未驗證。
- 沒有驗證新preset的遊戲內畫面；舊storm實機結果不作本次視覺驗收。

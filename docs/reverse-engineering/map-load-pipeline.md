# 原生地圖載入管線與 DATA 池載入契約

> 狀態：靜態二進位驗證（2026-10-09 Antigravity）。
> 逆向目標：解析地圖載入管線、`DATA/` 24 個池檔案強約束、三段 CHECK 函式語意、`CALCLOW` 游標初始化、以及空白地圖載入卡死之根本原因。

---

## 1. 命令分派與關卡進入點

在主執行檔 `Against_Rome.exe`（SHA256 `6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`）中，引擎模式/命令註冊函式 `0x426980` 於啟動時建立命令分派表：

| 命令 ID | 命令名稱 | 處理常式 VA | 說明 |
|---|---|---|---|
| `0x0b` (11) | `CMD_LOAD_LEVEL` (`0x5dd7d3`) | `0x4310f0` | 一般關卡/地圖載入（單人戰役、無盡模式、自訂地圖） |
| `0x0a` (10) | `CMD_LOAD_GAME` (`0x5dd7e2`) | `0x431410` | 存檔載入 |

在 `0x4310f0` 中，引擎先呼叫 `0x502e00` 與 `0x44e590` 初始化網路與隊伍狀態，隨後將地圖目錄路徑推入堆疊，呼叫核心載入器 **`CLMP_LoadLevel`（`0x4870e0`）**。

---

## 2. 核心載入器 `CLMP_LoadLevel` 管線階段

`CLMP_LoadLevel`（`0x4870e0`，內部檔名 `CLMP\mp_lsave.c`，標籤 `copyprot30`）完整執行順序如下：

1. **`LDFLOORINI` (`0x481de0`)**：載入 `boden.ini`，設定 `[Heightmapstep]`（全域 `0x771880`）與 `[Waterlevel]`（全域 `0x77187c`）。
2. **`LDFLOORTEX` (`0x481ba0`)**：載入 `boden.txt`（64×64 tile 材質表）。
3. **`LDFLOORHMAP` (`0x483410`)**：載入 `boden.bmp`（高度圖，257×257，GREEN channel × `Heightmapstep`，stride `0x202` 之 `int16`）。
4. **`LDFLOORCMAP` (`0x486ec0`)**：載入 `collision.bmp`（碰撞圖，256×256）。
5. **`LDFLOORSMAP` (`0x4835f0`)**：載入 `smooth.bmp`（平滑遮罩）。
6. **`LDFLOORVMAP` (`0x483bf0`)**：載入 `vertex.bmp`（逐頂點 RGB 色彩調色）。
7. **`LDFLOOREMBMAP` (`0x4845b0`)**：載入 `emboss.bmp`（逐頂點光照強度）。
8. **高度衍生快取讀取/重算**：
   - `LDSKYMAP` `skydens.dat` (`0x484880`)
   - `LDVISMAP` `visible.dat` (`0x484eb0`)
   - `LDCLIPMAP` `cliprect.dat` (`0x4853d0`)
   - `LDSHADMESHES` `shadows.dat` (`0x486a00`)
9. **`CL_LoadLevelData` (`0x48e960`)**：依序巢狀載入 `DATA/` 下全部 24 個二進位池檔案。
10. **`CHECK` 三階段 (`0x48746b` - `0x487475`)**：
    - `0x4acfe0` (CHECK 1: HP 計算)
    - `0x4ad110` (CHECK 2: MP/士氣計算)
    - `0x4adf40` (CHECK 3: 動畫相位交錯偏移計算)
11. **`DAYETC` (`0x484460` - `0x49b4e0`)**：天空與全域環境光照。
12. **`BUMPSIN` (`0x4818f0`, `0x49d210`)**：水面漣漪與波紋正弦表。
13. **`CALCLOW` (`0x487503` - `0x487530`)**：執行 10 個池的游標/上限水位（watermark）初始化。
14. **動態與靜態 GFX 快取**：
    - `APT` (`0x4accb0`, `0x4acc30`, `0x4accd0`)：靜態物件 GFX 索引。
    - `ALR` (`0x4acdb0`, `0x4acd20`, `0x4acde0`)：動態模型 GFX 索引。
    - `SHADTEX` (`0x4ace20`)：陰影紋理快取。
15. **`CALCFLOORX` (`0x491bd0`)**：由地形高度場陣列產生世界座標投影矩陣與網格頂點。
16. **隊伍 0..8 初始化 (`0x487687` - `0x487748`)**：依序註冊隊伍 0 至 7，以及中立隊伍 8（`-1`）。
17. **碰撞與通行網格合併**：
    - `COLLMESH` (`0x4d7d50`)
    - `ANIMTIMERRANDOM` (`0x4bcf70`)
    - `MERGELAYERCOLLMESH` (`0x4d8830`)：將 `visible.dat` bit0 未設定之圖塊強制設為阻擋（255）。
    - `CALCISTBETRETBAR` (`0x4ae320`)：計算各格子之通行性。
18. **聚落與物件最後核對**：
    - `BIGLAGERCHECK` (`0x4d4e80`)：核對大型營地/倉庫（`biglager.dat`）容量。
    - `ACTIVEOBJECTS` (`0x4ae470`)：統計全域有效物件數量並存入 `0x771ca8`。

---

## 3. 重大發現：空白地圖卡死根因與 `DATA/` 24 池強約束

### 3.1 `CL_LoadLevelData` (0x48e960) 快速失敗機制

`0x48e960` 依固定順序以 `0x415400` 開啟 24 個檔案：
1. `light.dat` (`0x48ab70` -> `0x7f9ef0`)
2. `gametime.dat` (`0x48b0c0` -> `0x771d30`)
3. `rain.dat` (`0x48b230` -> `0x771ecc`)
4. `hagel.dat` (`0x48b440` -> `0x7b1ed8`)
5. `snow.dat` (`0x48b720` -> `0x7e9ee4`)
6. `flash.dat` (`0x48b910` -> `0x11a0574`)
7. `objects.dat` (`0x48bae0` -> `0xa14bfc`)
8. `position.dat` (`0x48c0c0` -> `0xb1883c`)
9. `anim.dat` (`0x48c250` -> `0xbb9a5c`)
10. `gfxtype.dat` (`0x48c460` -> `0xc2705c`)
11. `action.dat` (`0x48c630` -> `0xf00454`)
12. `objdata.dat` (`0x48c760` -> `0xf5ff94`)
13. `hirarchy.dat` (`0x48cf70` -> `0x114c294`)
14. `formatio.dat` (`0x48d210` -> `0x11ac5e0`)
15. `lager.dat` (`0x48d5b0` -> `0x1232da0`)
16. `engine.dat` (`0x48d6e0` -> `0x771c3c` offset 34)
17. `fow.dat` (`0x48d7d0` -> `0x12330a4`)
18. `fowreq.dat` (`0x48d940` -> `0x12330ac`)
19. `way.dat` (`0x48dae0` -> `0x1233160`)
20. `particle.dat` (`0x48dcf0` -> `0x12355d4`)
21. `explos.dat` (`0x48de80` -> `0x146d644`)
22. `hitex.dat` (`0x48e020` -> `0x14856b0`)
23. `stat.dat` (`0x48e1b0` -> `0x148f430`)
24. `biglager.dat` (`0x48e350` -> `0x14a38b8`)

在反組譯碼中，**每一個檔案開啟失敗時，皆跳轉至 `0x48ea00`**：
```asm
0x48ea00: mov esi, 0x3e8                ; error code 1000
0x48ea05: push edi
0x48ea06: push esi
0x48ea07: push 0x5f4a50                 ; 'err LoadLevelData:%ld (%ld)\n'
0x48ea0c: push 0
0x48ea0e: call 0x4156d0
0x48ea16: add esp, 0x200
0x48ea1f: ret                           ; 直接退出！後續所有池檔案皆未載入！
```

### 3.2 呼叫端未檢查回傳值（Ungated Call）

在 `0x487180`（`CLMP_LoadLevel`）中：
```asm
0x48742f: call 0x48e960                 ; 載入 24 個池檔案
0x487434: add esp, 8
0x487437: call 0x566b50                 ; 取時間
0x487455: call 0x4b7d60
0x48746b: call 0x4acfe0                 ; CHECK 1 直接執行！
0x487470: call 0x4ad110                 ; CHECK 2 直接執行！
0x487475: call 0x4adf40                 ; CHECK 3 直接執行！
```
**呼叫端完全沒有對 `0x48e960` 的回傳值做任何檢查或錯誤處理閘門**。
一旦 `DATA/` 目錄缺少 24 個池檔案中的任何一個（例如建立空白地圖時只放入 `objects.dat`、`objdata.dat`、`position.dat`），`0x48e960` 在遇到第一個缺失檔案時便提早返回，導致後續所有池指標均為未初始化或垃圾記憶體。接著 `CLMP_LoadLevel` 盲目進入後續的 CHECK 與渲染計算，造成無窮迴圈或記憶體存取違規（Access Violation），**這就是空白地圖載入卡死/崩潰的根本原因**。

**結論**：任何有效且可在遊戲中載入的原生地圖，其 `DATA/` 目錄**必須完整具備全部 24 個池檔案**。空白地圖必須基於既有合法地圖（如 `ENDL_000`）複製全部 24 個池檔案，並以安全的重置邏輯將物件池槽位標記為未啟用，而絕不可刪減池檔案。

---

## 4. 三段 CHECK 函式精確語意

在 `0x48746b`–`0x487475` 呼叫的三個 CHECK 函式，其精確用途如下：

### 4.1 CHECK 1 (`0x4acfe0`)：物件生命值與士氣初始化
- 遍歷 14,000 個物件槽位（以 `0x4ab7b0` 判定 active）。
- 取得 `TypeId`（位於物件結構 runtime offset `+0x16`）與 `objdata` 槽位（位於 runtime offset `+0x1a`）。
- 呼叫 `0x4afd80(TypeId)` 驗證 `0 <= TypeId < 2500` 且 Archetype 啟用。
- 呼叫 `0x4ab8d0(objdata)` 驗證 objdata 槽位有效。
- 計算當前生命值：
  $$\text{current\_hp} = \text{Archetype}[\text{TypeId}].\text{max\_hp} (\text{offset } 0x2c) \times \text{objdata}[\text{slot}].\text{hp\_ratio} (\text{offset } +8)$$
  寫入 `objdata[slot].current_hp`（offset +4）。
- 第二迴圈計算士氣比例：
  $$\text{morale\_ratio} = \frac{\text{current\_morale}}{\text{max\_morale}}$$

### 4.2 CHECK 2 (`0x4ad110`)：物件魔法值/精力初始化
- 遍歷 14,000 個物件槽位，對合法 `TypeId` 與 `objdata` 槽位計算：
  $$\text{current\_mp} = \text{Archetype}[\text{TypeId}].\text{max\_mp} (\text{offset } 0x34) \times \text{objdata}[\text{slot}].\text{mp\_ratio} (\text{offset } +0x10)$$
  寫入 `objdata[slot].current_mp`（offset +0x0c）。

### 4.3 CHECK 3 (`0x4adf40`)：動畫相位交錯偏移（Stagger Phase）
- 遍歷 14,000 個物件槽位，取得 `anim` 槽位（runtime `+0x12`）、`position` 槽位（runtime `+0x0c`）與 `TypeId`（runtime `+0x16`）。
- 若 position 或 TypeId 無效，將 `anim[slot]` 偏移 4 設為 0。
- 若兩者皆有效，以物件世界座標與 Archetype 週期常數計算相位差：
  $$\text{phase} = (X_{\text{tile}} + Z_{\text{tile}}) \pmod{(\text{Archetype}.\text{field\_c0} + 1)}$$
  寫入 `anim[slot].offset4`，確保同型物件在畫面上不會在同一影格同步播放相同動作。

---

## 5. `CALCLOW` 10 大池游標（Cursors）初始化

在 `0x487503`–`0x487530` 呼叫的 10 個常式，負責掃描各池並初始化執行期分配游標（Allocation Cursors）：

| 常式 VA | 驗證器 | 水位全域變數 | 池用途 |
|---|---|---|---|
| `0x4ac580` | `0x4ab7b0` | `0x77189c` | objects 最高啟用槽位 |
| `0x4ac5b0` | `0x4ab7b0` | `0x7718a0` | objects 掃描水位 |
| `0x4ac600` | `0x4ab7f0` | `0x7718a4` | position 分配游標 |
| `0x4ac640` | `0x4ab860` | `0x7718a8` | gfxtype 分配游標 |
| `0x4ac690` | `0x4ab830` | `0x7718ac` | anim 分配游標 |
| `0x4ac6d0` | `0x4ab8a0` | `0x7718b0` | action 分配游標 |
| `0x4ac720` | `0x4ab8d0` | `0x7718b4` | objdata 分配游標 |
| `0x4ac760` | `0x4ab910` | `0x7718b8` | hirarchy 群組分配游標 |
| `0x4ac7b0` | `0x4ab940` | `0x7718bc` | formatio 分配游標 |
| `0x4ac7f0` | `0x4ab980` | `0x7718c0` | lager 營地分配游標 |

若池內所有槽位皆為未啟用（如重置為空白地圖），游標保持為 0，分配器即可從槽位 0 開始正常配置。

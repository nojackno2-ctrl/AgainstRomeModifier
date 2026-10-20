# formation-probe-rs

Against Rome 隊形線段取樣、ALR 士兵影格萃取與 PFIL 唯讀傾印工具（純 Rust 實作）。
此 crate 為 `tools/re/formation-probe` (C#) 與 `tools/re/formation-probe/probe.py` (Python) 之完整 Rust 移植替代方案。

## 功能規格

1. **隊形幾何佈局計算 (`--layout`)**：
   - 讀取並解壓縮明確指定之 PFIL `formdef.dau`（若無 PFIL 標頭則視為未壓縮 CSV）。
   - 計算原始檔案的 SHA256 雜湊值。
   - 解析 `[FormationDefault]` 區段中的隊形定義線段。
   - 驗證線段是否匹配內建的原生隊形預設幾何 (`matchesEmbedded`)。
   - 針對人數 1..20，依序取樣計算成員的世界 X/Z 座標偏移。
   - 輸出單行 JSON 格式，完全匹配 C# 原版 probe 與 `compare_native.py` 所需之欄位。

2. **ALR 士兵影格萃取 (Archive Mode)**：
   - 安全開啟明確指定之 `alr.dat` ZIP 封裝。
   - 提取指定資產條目（預設 `SYSTEM/DATA/ALR/gerinf01.alr`）。
   - 解析 ALR 文件，依 native 規則計算待取樣的影格索引（`doc.layout_rows > 14 ? 14 * doc.layout_columns : 0`）。
   - 解碼至多 24 個影格，寫入 `soldier-0.rgba` .. `soldier-23.rgba`（RGBA 4 位元組像素格式）。
   - 計算基準錨點座標 `AnchorX = doc.anchor_width / 2 - offset_x`、`AnchorY = doc.anchor_height / 2 - offset_y`。
   - 輸出 `frames.json` 中繼資料，並在標準輸出印出 `Exported frames`。

3. **PFIL / LZSS 唯讀傾印 (`dump`)**：
   - 格式完全對齊 Python `probe.py` 之 `--dump` 輸出。
   - 輸出首兩行為 `SHA256 <hex>` 與 `payload length <bytes>`。
   - 每 16 位元組輸出一位移列：`<offset:06x> <hex bytes> <python_repr>`。
   - 支援 `--dump START:END` 範圍指定（支援十進位與 `0x` 十六進位）。

## 邊界限制與安全規範

- **不包含 EXE 反組譯**：
  本 crate 不依賴反組譯器引擎（如 Capstone、Zydis 等），因此不支援 `probe.py` 原有的 PE/EXE 反組譯 (`--range`) 功能。二進位反組譯明確保留給外部反向工程工具或 Python/Capstone probe 處理。
- **純顯式路徑 (Explicit Paths Only)**：
  絕不隱式探測、猜測或搜尋任何安裝遊戲目錄（如 `C:\Program Files (x86)\Against Rome`）；所有檔案與封裝路徑必須由命令列明確傳入。
- **全新輸出目錄保護**：
  輸出目錄必須為不存在之全新路徑，若目標路徑已存在任何檔案或目錄，立即中斷並拒絕覆寫。
- **拒絕危險路徑**：
  全面拒絕空路徑、空字元 (`\0`) 以及包含父層遍歷 (`..`) 之越界逃逸路徑。

## 命令用法

```bash
# 1. 隊形幾何佈局計算
cargo run --manifest-path tools/re/formation-probe-rs/Cargo.toml -- --layout <path/to/formdef.dau>

# 2. 士兵影格萃取
cargo run --manifest-path tools/re/formation-probe-rs/Cargo.toml -- <path/to/alr.dat> <NEW_OUTPUT_DIR> [asset_name.alr]

# 3. PFIL 唯讀傾印
cargo run --manifest-path tools/re/formation-probe-rs/Cargo.toml -- dump <path/to/file> [--dump 0:256]
cargo run --manifest-path tools/re/formation-probe-rs/Cargo.toml -- <path/to/file> --dump 0:256
```

## 測試驗證

本專案包含 20 項單元測試，涵蓋合成資料與邊界防護：
```bash
cargo test --manifest-path tools/re/formation-probe-rs/Cargo.toml
```

- `formation_tests` (5 項)：原生隊形匹配、1..20 取樣計算、邊界限制、CSV 解析容錯、合成 PFIL 端到端。
- `pfil_tests` (6 項)：未壓縮資料直通、合成 PFIL 解壓縮 roundtrip、Python bytes repr 模擬、傾印輸出格式、區間解析、SHA256。
- `archive_tests` (2 項)：合成 ZIP ALR 影格解碼與 RGBA 寫入、已存在輸出目錄拒絕覆寫防護、遺失資產防護。
- `safety_tests` (7 項)：路徑安全驗證、空字元/遍歷拒絕、全新目錄驗證、反組譯邊界說明攔截、CLI 整合驅動。

## 根目錄 Cargo Workspace 註冊建議

待審閱根目錄 `Cargo.toml` 既有變更後，可將本 crate 加入根目錄 `[workspace.members]`：

```toml
members = [
    # ...
    "tools/re/formation-probe-rs",
]
```
加入後，即可移除本 crate 自身 `Cargo.toml` 中臨時用於獨立建置之 `[workspace]` 宣告，統一納入根目錄 workspace 管理。

# WP-03 Handoff — Against Rome Modifier 全特徵與設定檔補丁

**更新時間**：2026-10-15  
**工作包**：WP-03 (P1)  
**所有權路徑**：`native/against-rome-modifier/**`  
**狀態**：Ported & Verified (純 Rust 零外部相依，44/44 測試全綠)

---

## 1. 任務完成概述

依據 `docs/rust-rewrite-inventory.md` 第 5 節之 WP-03 規格，完成以下任務：

1. **可擴充特徵登錄系統 (對齊 C# `FeatureRegistry`)**：
   - 實作於 `src/features.rs`。
   - 完整支援 49 項功能特徵（`FeatureRegistry::ALL`），涵蓋 `Stats`、`Compat`、`Language` 三大類別。
   - 支援 `FeatureControlKind`（`Toggle` 42 項、`Specialized` 7 項）。
   - 實作型別安全的 `FeatureValue`（Bool、Int、String）與 `PatchProfile`（大小寫無關快速查表、預設值防護）。
   - 提供 `validate_toggle_ids` 嚴格校驗切換特徵集合。

2. **純 Rust 文字與二進位設定檔 Patcher**：
   - **`cl_script.ini` 補丁 (`src/cl_script.rs`)**：移植自 `ClScriptPatcher.cs`，支援平民快速生產 (`CiviDelay`)、無盡士氣 (`MoralsDec*`、`MoralsIncIdle`)、法術傷害 5 倍、法術治療 10 倍、復活 100%、將領技能 5 倍、各陣營法術半徑乘數及射程 3 倍。
   - **`ress.ini` 補丁 (`src/ress.rs`)**：移植自 `RessPatcher.cs`，支援建築免費建造/升級 (`[objres] Bau*`)、兵種/攻城武器免費生產 (`[objres] Fig*`)、祭司零法術耗損，以及部族科技與兵種升級免費 (`[volkres]`)。
   - **`team.dat` 補丁 (`src/team_dat.rs`)**：移植自 `TeamDatPatcher.cs`，支援人口上限設定（預設 1600）與玩家強制設為羅馬陣營 (`ROM`)。
   - **`cl_epara.ini` 補丁與基準 (`src/epara.rs`)**：移植自 `EparaPatcher.cs` 與 `CleanEparaBaseline.cs`，三倍射程時自動將 `[ProjectileVarianceOnMove]` 散布歸零、將 `[ProjectileInitSpeedFactor]` 乘以 1.08（1.5 → 1.62）校正飛行抵達時間；內建 `CleanEparaBaseline` 完整原文與 PFIL 二進位壓縮產生器。
   - **`partgeo.dau` 補丁 (`src/partgeo.rs`)**：移植自 `PartgeoPatcher.cs`，支援拋射弧度增高，將 7 個武器拋射物條目（標槍、飛斧、投石、箭矢、長矛、火把）之 `ysub` 重力值乘以 2.0，嚴格驗證欄位寬度與筆數。
   - **`objdef.dau` 補丁 (`src/objdef.rs`)**：移植自 `ObjdefPatcher.cs`，支援人口容量 20 倍、倉庫容量 10 倍、主城 HP 10 倍、建造/升級時間除以 10、首領榮譽成長 5 倍、拋射初速 `emit` 2 倍、命中傷害半徑 `drad` 2 倍、兵種 9 項屬性縮放、村民移動速度 5 倍、奔跑不扣血 (`Lpsub` 歸零)、全圖視野 (30000) 覆蓋，並嚴格保持 `objdef.dau` 原文字串長度不變。
   - **純 Rust 引擎 LZSS 與 PFIL 容器處理器 (`src/lzss.rs`)**：完全匹配 `GameLZSS.cs` / `FUN_00565c00` 規格（4096 環狀視窗、前 4078 字元空格、最後 18 字元 0x00、64 位元組 PFIL 標頭保留），支援解壓縮與壓縮。
   - **純 Rust Windows-1251 與 CSV 工具 (`src/patch_text.rs`)**：純查表雙向無損轉碼，處理 `\r\n` / `\n` 換行保留與定長欄位對齊。

3. **二進位兵種設定解析器與配置表 (`src/troops.rs`)**：
   - 移植自 `TroopConfig.cs` 與 `UnitStatParser.cs`。
   - `TroopConfig`: 收錄 43 個兵種之完整元數據（`UnitMetadata`: Faction, Tier, UnitType, Style）、繁體中文名稱對照表、最終 9 項平衡屬性常數表，以及依 Tier 階級與原始索引的穩定排序演算法。
   - `UnitStatParser`: 實作多層屬性融合 (`merge_layers`)、共用武器槽近戰與遠程傷害分類 (`get_melee_and_ranged_damage`)、冷卻時間解析 (`get_melee_and_ranged_reload`)、最大射程計算 (`get_maximum_range`) 及祭司可配置法術半徑判定。

4. **EXE 補丁擴充 (`src/exe_patches.rs`)**：
   - 保留既有 `FOCUS`、`ROMAN_ENDLESS`、`UNIT_RECRUIT_20`。
   - 擴充加入 `CIVI_PRODUCE_20` (0x4fc00)、`CORPSE_RETENTION` (0x110906)。

5. **CLI 工具擴充 (`src/main.rs`)**：
   - 保留原有的唯讀 `inspect` 與安全事務 `apply`（含 Program Files 路徑保護與備份機制）。
   - 新增 `features` 子命令（列出 49 個登錄特徵）與 `troops` 子命令（列出所有兵種階級與中文名稱）。

---

## 2. 測試與驗證證據

### 工具鏈與執行環境
- `CARGO_HOME`：`D:\Github\AgainstRomeModifier\re_workspace\rust-runtime\cargo`
- `RUSTUP_HOME`：`D:\Github\AgainstRomeModifier\re_workspace\rust-runtime\rustup`
- 標靶目錄：`re_workspace/target-modifier`
- **相依特性**：純 Rust `std` 實作，零外部 crates 相依，無 proc-macro / build-script，完全不觸發 Windows 應用程式控制 (4551) 限制。

### 驗證執行命令
```powershell
$env:CARGO_HOME='D:\Github\AgainstRomeModifier\re_workspace\rust-runtime\cargo'; $env:RUSTUP_HOME='D:\Github\AgainstRomeModifier\re_workspace\rust-runtime\rustup'; $env:PATH="$env:CARGO_HOME\bin;$env:PATH"; cargo test --manifest-path native/against-rome-modifier/Cargo.toml --target-dir re_workspace/target-modifier
```

### 測試結果統計
- `src/lib.rs` (單元測試): **29 passed; 0 failed**
- `src/main.rs` (CLI 內部測試): **1 passed; 0 failed**
- `tests/cli.rs` (CLI 整合與參數防護): **2 passed; 0 failed**
- `tests/csharp_parity.rs` (C# 原始碼比對測試): **1 passed; 0 failed**
- `tests/epara_parity.rs` (Clean Epara 與零售光碟 `cl_epara.ini` 往返比對): **3 passed; 0 failed**
- `tests/patchers_roundtrip.rs` (合成資料與零售 `cl_script.ini` / `ress.ini` / `team.dat` / `partgeo.dau` 往返測試): **8 passed; 0 failed**
- **總計：44 passed, 0 failed, 0 ignored (全部通過)**。

---

## 3. 檔案變更清單 (所有權路徑 `native/against-rome-modifier/**`)

- `src/lib.rs`：註冊所有子模組並公開匯出核心 API。
- `src/lzss.rs`：純 Rust LZSS / PFIL 壓縮解壓縮核心。
- `src/patch_text.rs`：Windows-1251 與 CSV / 文字檔案抽象層。
- `src/features.rs`：49 個特徵定義、登錄器與 PatchProfile。
- `src/cl_script.rs`：`cl_script.ini` 補丁器。
- `src/ress.rs`：`ress.ini` 補丁器。
- `src/team_dat.rs`：`team.dat` 補丁器。
- `src/epara.rs`：`cl_epara.ini` 補丁器與 CleanEparaBaseline。
- `src/partgeo.rs`：`partgeo.dau` 補丁器。
- `src/objdef.rs`：`objdef.dau` 補丁器。
- `src/troops.rs`：43 兵種定義、中文名稱、屬性常數與解析器。
- `src/exe_patches.rs`：擴充的 EXE 二進位補丁常數。
- `src/main.rs`：CLI 擴充 `features` 與 `troops` 查詢命令。
- `tests/epara_parity.rs`：針對 CleanEparaBaseline 與光碟零售 baseline 的比對測試。
- `tests/patchers_roundtrip.rs`：全設定檔補丁往返與真實零售檔案測試。
- `HANDOFF.md`：本工作包交接記錄。

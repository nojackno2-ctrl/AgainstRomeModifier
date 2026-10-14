# Particle ALR metadata normalization

2026-10-12，Codex。純 Rust header metadata milestone；只新增
`native/against-rome-data/src/alr_metadata.rs`、同名 integration tests、本報告，
以及 `src/lib.rs` 的一行 module export。既有 ABI layout、FFI exports、C#、host、
build script、handoff 與其他 dirty 均保留；本工作未 commit，未呼叫 REA，未讀寫遊戲或 ISO。

## 證據來源與精確定位

只讀 parent 已完成的 `re_workspace/particle-alr-wrapper-20261012-evidence.json`
之下列五個 target 的 `normalized_result`；檔案含 18 records、0 unknowns。
Export SHA-256：`8366a4dc8aac1f65be3c8bbe606d205a457b64433425d19ad8a20d83df29dca9`。
證據對應 installed EXE SHA-256
`6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`。
本次沒有重新讀取 EXE 驗 hash；此身份由 export artifact/subject 記錄提供。

| Target | Evidence ID | 觀察 |
|---|---|---|
| `0x4af200` | `ev_aff650f2b9155bb91d7791847f9f8cee6119edd75ff2dd59fa6c4f90befd3ee1` | `0x4af204` 設 default 1；`0x4af209..220` 檢查 signed id、3000 上限、table pointer；`0x4af222` 讀 runtime +0x10 |
| `0x4af230` | `ev_52dd3b0865ccc4b94a32683363991df6b42980ec3dcf5c8a5a4e8470e50c3c73` | `0x4af234` 設 default 1；`0x4af239..250` 同樣 guard；`0x4af252` 讀 runtime +0x14 |
| `0x4e3a20` | `ev_ccf69e33fc9e1559df8c7cd2623dbbcb7772e841ea9dd92a8939abc4f6d38542` | header read order、runtime word 映射、末段 normalization |
| `0x4aebc0` | `ev_14550e1b260bd6b2660e518da7dacea363cab6efe16273cdb44acd5de02b4eeb` | id/cache guard、quality 來源與 wrapper 呼叫；失敗時 native 可嘗試 error ALR，API 不仿造替代資產 |
| `0x4e4170` | `ev_26dd90dddef86dd2f05d0df81f4249ea293fe671fcf3f619d182524fdd8bafbd` | 固定 normalize flag 1；quality sampling 重綁 payload aliases |

### Header read order（v4、v5、v6）

`0x4e3a91` 讀 signature，`0x4e3a99` 比對 `0x41524c41`（LE bytes `ALRA`）；
`0x4e3af1` 讀 version，`0x4e3afd/0x4e3b00` unsigned 比對 6/JA。
Native 接受較舊 version 分支；本 API 僅支援現有 inventory parser 的 4..6。
Native read destination 是 runtime local array，而 serialized version/word2 不複製到同序 index。

| Serialized word（byte offset） | Read CALL 位址 | 本次 metadata 解釋／runtime destination |
|---|---|---|
| 0 (0x00) | `0x4e3a91` | signature / runtime word0 |
| 1 (0x04) | `0x4e3af1` | version / scratch |
| 2 (0x08) | `0x4e3b0c` | raw / scratch |
| 3 (0x0c) | `0x4e3b1a` | frame count / runtime word1 |
| 4 (0x10) | `0x4e3b28` | depth / runtime word2 |
| 5 (0x14) | `0x4e3b36` | raw / runtime word3 |
| 6 (0x18) | `0x4e3b44` | raw columns → A / runtime word4 (+0x10) |
| 7 (0x1c) | `0x4e3b59` | raw rows → B / runtime word5 (+0x14)，version >1 |
| 8 (0x20) | `0x4e3b79` | palette variants / runtime word14，version >2 |
| 9 (0x24) | `0x4e3b99` | raw / runtime word15，version >3 |
| 10..16 (0x28..0x40) | `0x4e3ba7`, `0x4e3bb5`, `0x4e3bc3`, `0x4e3bd1`, `0x4e3bdf`, `0x4e3bed`, `0x4e3bfb` | raw / runtime words6..12 |
| 17..20 (0x44..0x50) | `0x4e3c09`, `0x4e3c17`, `0x4e3c25`, `0x4e3c33` | raw / runtime words18..21 |
| 21..22 (0x54..0x58) | `0x4e3cc1`, `0x4e3ccf` | raw / scratch，開始 frame references 前讀取 |

共 23 個 LE u32 / 92 bytes。`0x4e3c50..59` 的 v4/5 size accounting 與
`0x4e3cab` 的 runtime copy 不改本 API 的 raw words；本 API 不重建 runtime memory image。
Readonly source reference：`tools/re/particle-assets-rs/src/alr.rs` 的 signature、version、
depth、count、palette bounds 與 23-word parser；未修改該工具。

### Normalization flags 與 arithmetic

`0x4e3fb7/0x4e3fbf` 檢查 `0x4e3a20` param3；為零則跳過 normalization。
非零時：

1. `0x4e3fc9/0x4e3fcd` unsigned compare A with 1；只有 zero 走 `0x4e4145` 設 A=1。
2. `0x4e3fd7..0x4e3fdd` 同樣讓 zero B=1。
3. `0x4e3ff2` 32-bit IMUL，`0x4e3ff5/0x4e3ff8` unsigned CMP/JBE。
   product <= frame count 保留；product > frame count 時
   `0x4e4001/0x4e4008` 設 B=1、A=frame count。

因此 decompiler 的 unsigned `product >= count && product-count != 0` 與此分支相符；
product == count 不修正，product < count 也不修正，可能有多個 animation groups。
Native IMUL 只保留低 32 bits；API 對 overflow 回傳 `LayoutProductOverflow`，
不宣稱 native 也拒絕 malformed header。可接受範圍內，乘積及 unsigned 比對完全等價；
包含高 bit 但沒有 overflow 的 u32 product，仍依 unsigned 比對處理。
Disabled 模式保留 zero axes，zero product 的 group observation 為 `None`。

`0x4e4197` PUSH 1，`0x4e41a9` 呼叫 loader：
`0x4e3a20(filename, wrapper_param3, 1, references_out)`。
Wrapper 可先呼叫 callback（callback 行為不屬本 API）。wrapper param2 的 packed quality
在 `0x4e41e6..0x4e4205` 以 signed division/remainder 分拆 0x100；
`0x4e42af/0x4e42b9` 等使用 A/B，`0x4e445b/0x4e4462` 複製 0x18 runtime words。
後段保持 A/B，重新建構 frame payload aliases；本次未實作 quality、callback、pixels、
frame0 fallback 或任何 frame selection。實際 runtime aliases 與像素等價仍未驗證。

## 純 Rust API 與安全邊界

`SourceHeaderMetadata::parse(bytes)` 保留不可变的全部 raw words；
`from_header_words(words, source_len)` 供既有 JSON inventory audit 使用，
caller 必須提供同一 member 的 words/length。getter 暴露 version/depth/count/raw axes/variants。
`loaded_metadata(Normalization::{Enabled,Disabled})` 回傳 header-derived `LoadedHeaderMetadata`，
含 A/B、各 zero/collapse change flag、原 source 及 `GroupObservation`。
whole_groups = count/product，remainder_frames = count%product；它們只是算術觀察，
不能證明每個 frame 可選取、存在 payload 或具備完整 animation eligibility。

`observe_native_query(id, Option<&LoadedHeaderMetadata>)` 只模型化已知 query guards。
invalid id（negative 或 >=3000）及 missing metadata 回傳獨立的
`NativeQueryObservation::UnloadedDefault`（A/B=1、原因可辨識），不產生 Parsed/Loaded DTO。
傳入 metadata 的 `HeaderMetadata` variant 也不驗證 native residency、member/id 綁定或資產解析。
解析錯誤不會轉成 default 或 resolved asset。

Strict limits：完整 92-byte header、ALRA signature、version4..6、depth8、
frame count1..100000、palette variants1..4096；至少 `92 + frame_count*4` bytes
能容納 frame reference words。以 u64 計算 extent、不依 count 配置記憶體。
這個 lower bound 可以拒絕明顯 truncated input，**不驗證 frame reference 值、完整
frame payload、palette、scanline、trailing metadata 或 native load success**。
通過 lower bound 的截斷 payload 仍可能產生 header DTO；resolver 必須另行驗證 payload。
防禦 bounds/overflow 拒絕是本 parser 的限制，不能當 native malformed validation 證據。

## 本次驗證結果

Local Rust：workspace `re_workspace/rust-runtime` cargo/rustup，process-local
`CARGO_HOME`、`RUSTUP_HOME`、PATH；`CARGO_TARGET_DIR=re_workspace/alr-metadata-target`。

- `rustfmt --edition 2021` 只格式化新 module/test；同範圍 `rustfmt --check` 通過。
- `cargo test --locked --manifest-path native/against-rome-data/Cargo.toml --no-run`
  exit0，全部 existing/new test targets 編譯成功。
- 正常 `cargo test --locked` 的 SHA256 unit test **1 pass**；新
  `alr_metadata-e0210b2f7ba23666.exe` 尚未執行即遭 Windows application control
  `os error 4551`。正常重試與 sandbox 外同命令均受相同阻擋。
  本次未更改 Windows policy，未宣稱完整 suite pass。
- 新測試包含 **11 個 synthetic tests + 1 個 ignored inventory audit**：versions4/5/6，
  raw word order，zero axes，product小於/等於/大於count，nondivisible groups，
  disabled flag，unsigned high bit，overflow，所有 partial header/minimum extent，
  signature/depth/version/count/palette bounds，unloaded guards。新測試執行仍待 parent 驗證。
- `git diff --check -- native/against-rome-data` 通過；lib diff 只有新增 module export。

### 既有 installed inventory audit（本次未存取 game）

| Input | SHA-256 |
|---|---|
| `re_workspace/particle-assets-parent-20261012.json` | `83af7ec8e45e5443132d52dbdd2e808c2c597a24e2dc6f6024f74a98378f8fbf` |
| `re_workspace/particle-alr-wrapper-20261012-evidence.json` | `8366a4dc8aac1f65be3c8bbe606d205a457b64433425d19ad8a20d83df29dca9` |

PowerShell 只從既有 JSON 的 `target_alr_assets` 提取每個 member 的 length 和
`document.header_words_raw`，以獨立 u64 算術核對 enabled normalization：
**68 members / 11345 frames / 全部 v6 / 0 changed axes / 0 multiple groups / 0 remainders**。
這是 JSON 算術核對結果；沒有重新解析 installed ALR、重驗 source hash 或執行 Rust audit。
68 個 raw layout products 都等於 count，synthetic cases 才覆盖 normalization 分支。

已準備 ignored unique TSV：
`re_workspace/alr-metadata-audit-cf1129f7084c480ebcedaea26ccb0c0e.tsv`。
TSV SHA-256：`9e9e3fcdf98c91d7b62ed64a2357f7df7c20965b961999dd0259c9b801f29d87`。
格式為 member、length、23 comma-separated words、獨立預期 A/B/groups/remainder，四欄以 TAB 分隔。
測試對 raw words、各 member 預期值、duplicate names、68 members/11345 frames 做核對。
本次 Rust audit 因上述測試執行阻擋尚未跑；parent 正常環境可執行：

```powershell
$env:CARGO_HOME = "$PWD/re_workspace/rust-runtime/cargo"
$env:RUSTUP_HOME = "$PWD/re_workspace/rust-runtime/rustup"
$env:PATH = "$env:CARGO_HOME/bin;$env:PATH"
$env:CARGO_TARGET_DIR = "$PWD/re_workspace/alr-metadata-target"
$env:ALR_METADATA_AUDIT_TSV = "$PWD/re_workspace/alr-metadata-audit-cf1129f7084c480ebcedaea26ccb0c0e.tsv"
cargo test --locked --manifest-path native/against-rome-data/Cargo.toml
cargo test --locked --manifest-path native/against-rome-data/Cargo.toml --test alr_metadata installed_inventory_header_audit -- --ignored --nocapture
```

後續由 parent 驗 tests，再接 FFI/frame resolver；此里程碑尚未證明 sprites、quality aliases、
runtime frame payload、host preview 或遊戲像素等價。

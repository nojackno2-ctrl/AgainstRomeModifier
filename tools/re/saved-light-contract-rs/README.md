# saved-light-contract (Rust native CLI)

以 Rust 直接讀取 PE32、SHA-256、iced-x86 區域 disassembly 與 evidence JSON；沒有 Python subprocess／FFI，也不呼叫 REA 或執行遊戲。

## 驗證狀態（2026-10-11）

Parent 已用專案本機 Rust 1.99.0 編譯成功並產生 Cargo.lock。最終 Rust real EXE probe exit 0，746 指令斷言、外部 reference cross-check、source_unchanged=true；結果 `re_workspace/saved-light-rust-parent-final-d30f743449734a6b872b4862ea9330e5.json`。formatter 只略過沒有 encoded operand 的隱含運算元，停用 pseudo-op 顯示，將 xword/tbyte 的 80-bit 名稱正規化。

**2026-10-12 全套測試已通過。** Parent cargo test --locked exit0，8pass／0fail，含全部746地址/bytes mutation、原生解碼與schema負例。前輪5pass/3格式fail及修正後Windows 4551阻擋已解除；未調整安全原則。這仍是 bounded static proof，不是原遊戲runtime驗收。

已執行獨立 PowerShell 靜態核對：凍結 fixture 與最新 strengthened reference 的 assertion／field／region／radius／limitations／evidence 一致；746 unique assertion address 的 bytes 全部符合實際 EXE，9 區域 hash 與4份 evidence hash 一致。這項檢查**沒有執行 Rust 或驗證 Rust mnemonic／operands 解碼**。沒有建立任何 re_workspace 輸出報告。

## 執行（需已有 Windows Rust 工具鏈）

在 repository 根目錄使用 PowerShell：

```powershell
cargo test --manifest-path tools/re/saved-light-contract-rs/Cargo.toml
$result = Join-Path (Resolve-Path re_workspace) ("saved-light-rust-" + [guid]::NewGuid().ToString('N') + '.json')
cargo run --manifest-path tools/re/saved-light-contract-rs/Cargo.toml -- 'C:\Program Files (x86)\Against Rome\Against_Rome.exe' $result '<strengthened-reference.json>'
```

第三個 positional argument 必須明確指定 strengthened reference JSON。CLI 拒絕既有輸出、非 `.json`、子目錄、`..`、Windows device／ADS 名稱與 canonical parent 不在 repository `re_workspace` 的路徑；最後用 create_new 防止覆寫。測試 fixtures 在 ignored target；parent Cargo artifacts 在 ignored re_workspace/saved-light-contract-target。

## 契約與輸出

`src/contract.json` 凍結上述 strengthened reference（SHA-256 `78e389d00a51f042c47f40d03696c6c099ff2f66c7fe3809d71a1f2910e35da0`）的 746 unique assertion、16 欄、9 區域、radius 語意／limitations 及四份 evidence binding。這是程式的固定預期值，執行時仍須與外部 reference 一致。

- 支援 EXE SHA-256 `6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`、x86 PE32、ImageBase `0x400000`。逐區域核對 hash、完整 instruction boundaries，逐 assertion 核對 address、原始 bytes 與實際 mnemonic／operands。讀取 VA 必須完全落在唯一 file-backed section。
- Operand canonicalization 只處理空白、進位數字、等價條件 mnemonic 與 x87 隱含 ST0；保留 memory width、register、正負 displacement、branch target、operand 順序及 explicit x87 register。區域 instruction text 使用 iced-x86；assertions 保留原契約 mnemonic／operands，另加 `native_mnemonic`／`native_operands`。
- 欄位驗證涵蓋 width、runtime／snapshot offset、read helper、store 指令、`0x44` stride、packed 56 bytes／capacity 1024 pool 57352 bytes；外部 reference 全欄與所有 assertions／區域逐項交叉驗證。
- static／transform evidence 不只驗 schema，還把每行 bytes 和解碼語意對回實際 EXE；owner `0x49f8fa..0x49f9d7` 必須完整覆蓋。bundle 驗每筆 subject digest、identity、operation、parameters；原先四檔 SHA binding 均必須一致。bundle 的文字／pseudo-code 本身不宣稱被 Rust 重新反編譯。
- 來源 EXE、reference 與 evidence 在輸出前重讀確認一致。輸出維持原結果主要 key，增加 reference cross-check；`probe_sha256` 為實際 Rust executable hash，`tool_versions` 換成 Rust crate／iced-x86。

## 測試與界限

以下列出原交付的測試範圍；當時「未執行」敘述屬歷史狀態，以檔首2026-10-12 parent全套8pass結果為準。

整合測試涵蓋全部凍結 assertion／區域解碼、每個 assertion address／bytes mutation、16 欄 mapping mutation、schema 缺欄／錯 hash／bool 或無效 count／空資料／錯 instruction bytes／重複地址／錯 transform metadata、PE truncation／architecture／base／VA bounds，以及輸出拒覆寫／越界／create_new race。另有 repository integration test 讀取上述四份 evidence，驗 hash／schema 與 static 1717、transform 560 行的 native decode；搬移 crate 時需保留 repository evidence。這些測試目前**未執行**；首次 `cargo test` 尤其須檢查 iced-x86 與 Capstone 的格式等價性。

本工具僅提供 bounded static proof；未驗證 runtime tick／RNG／FPU／pixels、所有 writers/callers、helper bodies 或 preview 距離座標等價性。沒有修改 Python、C#、既有 docs／handoff、game／ISO，沒有 commit。

API 參考：[iced-x86 1.21.0 Decoder](https://docs.rs/iced-x86/1.21.0/iced_x86/struct.Decoder.html)、[FormatterOptions](https://docs.rs/iced-x86/1.21.0/iced_x86/struct.FormatterOptions.html)。

# Particle asset inventory — 2026-10-12, Codex

本次只新增 `tools/re/particle-assets-rs/` 與本文件。保留既有 dirty；依本次明確限制，不更新 shared handoff、不提交、不啟動遊戲、不寫 installed game／ISO、不呼叫 REA。另一 worker 的 `particle-render-contract-rs` 不在工作權內。

## 範圍與可重現指令

原生 Rust 唯讀 CLI；path dependency 重用 `native/against-rome-data` 的 strict PFIL/LZSS decoder。`Cargo.lock` 已建立。ZIP central/local header 驗證、deflate 解壓、CRC32／length 檢查只針對粒子 gfx 關聯到的 ALR。無圖片、palette 或像素資產匯出；JSON 保留相關表的原始 row bytes（hex，含行尾）、文字欄位值、來源 header bytes／decoded hash、ALR raw header words／frame descriptors／row descriptors／member hash。`raw_values` 保留內部欄位 padding，但整行邊緣 whitespace 被 trim；逐 byte 還原應使用 `raw_bytes_hex`。objdef／cl_alr 全表只統計，raw rows 僅保存解析鏈相關 subset。

在 repository root 的 PowerShell：

```powershell
$env:CARGO_HOME="$PWD/re_workspace/rust-runtime/cargo"
$env:RUSTUP_HOME="$PWD/re_workspace/rust-runtime/rustup"
$env:CARGO_TARGET_DIR="$PWD/re_workspace/particle-assets-target"
$cargo="$env:CARGO_HOME/bin/cargo.exe"
& $cargo fmt --manifest-path tools/re/particle-assets-rs/Cargo.toml -- --check
& $cargo test --locked --offline --manifest-path tools/re/particle-assets-rs/Cargo.toml
$output="re_workspace/particle-assets-20261012-$([guid]::NewGuid().ToString('N')).json"
& $cargo run --locked --offline --manifest-path tools/re/particle-assets-rs/Cargo.toml -- 'C:\Program Files (x86)\Against Rome' $output
```

output 的既存 parent 必須位於 canonical `re_workspace` 內、副檔名 `.json`，`OpenOptions::create_new(true)` 原子拒絕覆寫。所有 generated outputs／target 受既有 `.gitignore` 排除，不修改 ignore 規則。套件初次 fetch 的 sandbox network 失敗；經工具自動核准下載至指定 Cargo cache 後，locked/offline 執行成功。沒有變更全域 Rust environment。

## 實際 header 與 inventory

五個設定檔均為 PFIL、64-byte header、offset 16 declared decoded length；strict decoder 全部成功。decoded bytes 均為 ASCII（所有 bytes <= 0x7f）；不據此猜測更廣泛資產的 code page。遇非 ASCII 時保留來源 fingerprint 並明確回報 encoding unresolved，不做 lossy decode。

| 檔案 | Section | rows | header columns | decoded bytes |
| --- | --- | ---: | ---: | ---: |
| partpar.dau | ParticleParameterDefault | 100 | 16 | 13,865 |
| partgfx.dau | ParticleGrafikDefault | 102 | 9 | 8,265 |
| partgeo.dau | ParticleGeometrieDefault | 102 | 16 | 17,950 |
| objdef.dau | ObjectDefaults | 2,160 | 209 | 3,310,807 |
| cl_alr.ini | AlrNames | 2,074 | 無 header | 112,607 |

解析 column header 的實際名稱，去掉 `name` 周圍裝飾用 `-`，保留 header 原始 bytes。三張表的欄位依實際順序為：

- partpar: `idx, activ, name, maxam, timer, reflec, birth, fadet, windf, floorc, swarm, timdie, swarad, swarmtim, heisen, refint`
- partgfx: `idx, activ, name, lolig, obdef, expde, objpa, objpv, objpp`
- partgeo: `idx, activ, name, exsiz, ezsiz, heigh, exadd, eyadd, ezadd, exvar, eyvar, ezvar, ysub, xadd, yadd, zadd`

objdef 的 zero-based column indices：`alrid=5, alrml=8, name=52, papar=63, pageo=64, pagfx=65`。這是本次 installed header 的事實；[projectile-ballistics.md](projectile-ballistics.md) 的 64／65／66 標示與本次 zero-based indices 不一致。未修改該既有文件。

cl_alr 無欄位 header，僅按目前 catalog 的既有 positional id／filename 契約解析首兩欄；其餘值全部保留 raw，不捏造欄位名稱。檔名解析為 exact case-insensitive `SYSTEM/DATA/ALR/{filename}` member path，重複 ID／member 明確 ambiguous，不採 first-match 或 basename fallback。`-1` 明列無 reference。

成功 inventory：102 gfx `obdef` 關聯全部命中 objdef，102 `objpa` 均為 `-1`；102 primary ALR references 全部命中、102 `alrml` 均為 `-1`。169 個 objdef 有粒子 references，507 個 `papar/pageo/pagfx` references 全部解析；三表各自查自己的 ID，沒有按同名或同 ordinal join。共保留 251 筆相關 objdef raw rows 與 68 筆 cl_alr raw rows。表內無 duplicate IDs 或 column-count issues；範圍內無 missing／ambiguous references。

68 unique ALR members 全部為 version 6、8-bit indexed；共 11,345 frame records，每個 document 的 `columns * rows == frame_count`。shared records 仍計入 serialized frame count。各 frame 保留 dimensions／offset words／packed size／stored size／row descriptors；reference 必須指向較早的 record。palette index 邊界依 **第一個 frame 的 palette** 檢查，後續 frame 可無本地 palette；此點依 `NativeAlrDocument.DecodeFrame` 參照，非自行猜測。全部 document 都有 16 trailing bytes，僅記錄 length／SHA256，不解釋其語義。

`ParRomSch01_Pfeil` objdef 830 的實際鏈：`papar 22 → Pfeil00`、`pageo 23 → Pfeil00`、`pagfx 24 → Pfeil00`；gfx 24 `obdef 227 → FigRomSch01WafA_Pfeil`；objdef 227 `alrid 258 → romwaf01.alr`。該 ALR 有 608 frames、19 columns × 32 rows、anchor 98 × 98。partgeo 23 `heigh=983040`、`ysub=5832704`，保留 raw integer，不在此工具轉換成 runtime units。這支持舊 ballistics 文件的資料關聯，但不驗證其速度、重力或 runtime 型別敘述。

## 驗證與追蹤

第一輪 JSON：`re_workspace/particle-assets-20261012-fddc6a128ede460399a56e33eb824ddb.json`，exit 0、6 sources unchanged；15 ALR parsed／53 unresolved。原因為初版錯用每個 frame 的本地 palette count；經目前 C# reference 確認第一個 frame palette 的規則後修正，新增 bounded fixture，沒有放寬 index／offset 邊界。

修正後 JSON：`re_workspace/particle-assets-final-20261012-c2e0fe7e50214a0c8ff821ea8d85df4e.json`，exit 0、68 parsed／0 unresolved、11,345 frames、6 sources unchanged。11 tests 全 pass；之後新增 ZIP central-directory／deflate truncation、CRC／length 邊界測試與完整性統計，最終結果記於下段。

最終 authoritative JSON：`re_workspace/particle-assets-audit-20261012-e1a7bca83302495dbfb9b6c861043463.json`。上述 `cargo run --locked --offline` exit 0；`inventory_complete_within_scope=true`，6 sources unchanged、102 gfx rows、169 particle-referencing objdefs、68 parsed ALRs、11,345 frames，unresolved gfx-object／ALR layers／emitter links／members、table issues 全部 0。`cargo fmt -- --check` exit 0；`cargo test --locked --offline` **13 passed／0 failed**。deflate 必須到 `Status::StreamEnd` 且完整消費 compressed bytes；測試也同步縮短 declared packed size，確實驗證解壓邊界，不只 ZIP slice 長度。

追加驗證：重用 authoritative output path，CLI exit 1 `output already exists; refusing overwrite`，output SHA256 未變。nonexistent fixture root 產生獨立新 JSON，明列 6 missing/unreadable inputs、`inventory_complete_within_scope=false`；不把零命中當完整盤點。期間共用 dependency 曾暫時出現 `E0583 particle_ffi file not found`，待其他 worker 完成該檔後正常重跑成功；本工具未修改 dependency。

測試涵蓋 PFIL truncated header／body／trailing input、table missing／duplicate header／duplicate IDs／invalid IDs／truncated row／non-ASCII、ALR 全 prefix truncations／signature／version／depth／palette count／row offsets／scanline width／palette index／shared out-of-range／後續 frame 共用首 palette、ZIP missing／duplicate members／path rejection／invalid header／out-of-range directory／local-name mismatch／CRC／deflate size limits。均為小型合成 fixture，不依賴 installed assets 才能測試。

6 個 installed inputs 的 before／after SHA256、length、mtime Unix ns 精確相同。`objdef.dau` mtime 為 2026-10-09，其他來源為 2003；本次沒有將目前 installed objdef 認證為原版。成功 inventory 的指紋：

| 檔案 | bytes | SHA256 |
| --- | ---: | --- |
| partpar.dau | 2,787 | `5390d27f978b2389979be41f77fc4bfd25469db3d3c5503a5a7955de3da55b40` |
| partgfx.dau | 2,080 | `88bff8aaf20e29413d9a8678e227e7739ec945ee4699d25d290a86b4b235ab00` |
| partgeo.dau | 3,584 | `36d7e946cc2f1acbe1a5383d0e46ef55084dbcaa842b962697e630d7d9d45311` |
| objdef.dau | 433,846 | `4471856e6354d73198e12ae79d99abb038949dd460c59df4371272d2340745fe` |
| cl_alr.ini | 21,476 | `de1386f49fe18ec6d71c3560d0917ac97eebbd1c43549fc466bf57aeda28a89e` |
| alr.dat | 175,311,196 | `54fe9e951f5038a29690718100146a6ff740bfdfa334dca1c10406fdf3bed894` |

## 剩餘 mapping gaps

Parent 補驗（2026-10-12 Codex）：最終 crate `cargo fmt -- --check` 與 `cargo test --locked --offline` 均成功，13 pass / 0 fail。實際重跑 CLI 輸出 `re_workspace/particle-assets-parent-20261012.json`，68 ALR / 11,345 frames、scope unresolved 與 table issues 均0、6來源 hash/length/mtime 不變。再次指定同一路徑得到預期 exit1 拒絕覆寫，既有 JSON SHA256 不變。這些檢查仍不驗證 runtime frame 或遊戲像素。

只讀 parent `re_workspace/particle-render-consumers-20261012-evidence.json` 的 target records `analyze_function` 0x4dbe60／0x4dcfc0／0x4b53c0；未重開 parent REA session。0x4dcfc0 的既存 pseudocode 顯示以 objdef stride 0x2a4 讀取資料、另索引 stride 0x3c table、最後呼叫 0x4b53c0。這提供 render-consumer 的研究方向，**不足以在此 inventory 將欄位標籤、signedness、conversion 或 runtime offsets 認證為表 callback 契約**；另一 worker 的 Rust render contract 才是後續交叉驗證入口。

該 parent evidence SHA256：`a2f395d19816bef29883b92d5f542c78a8c60427ce4e618817d99dbad54adcb1`。只按目標 procedure 選取既有 normalized records，未輸出其他 records 或整包 pseudocode。

未完成：`lolig/expde/objpv/objpp` 的跨表角色、全部 text loader callbacks／runtime field types、definition overlays／clengine DefaultPath 選擇、particle saved child 與 gfx／frame／direction／palette／blend 的嚴格連結、16-byte ALR trailing metadata、runtime tick／RNG／FPU／visibility 與原遊戲 pixel equivalence。沒有選擇任何動畫 frame，不做 frame0 fallback；columns／rows 只按 metadata 名稱報告，不視為已驗證 animation／direction 語義。

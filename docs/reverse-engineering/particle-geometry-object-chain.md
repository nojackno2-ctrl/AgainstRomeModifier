# Particle geometry and primary object chain

2026-10-13 Codex。Rust `particle_definitions`現在解析三個particle base tables及objdef選定欄位，依明確ID建立primary definition chain。尚未FFI／host／particle rendering／新DLL部署。

## Static source evidence

installed EXE SHA256 `6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`。REA新export `re_workspace/particle-geometry-object-20261013-evidence.json`：10records，SHA256 `f64dcad0eaccdaf458ed72e9b82c7f55b2cb351837186f060b304056a938db2b`，REA已close。

4DE410完整92bytes（Evidence `ev_28a587a5067c33eda6e0bce6220e20175590329372a4988b6762f44b76b7f59b`）註冊`[ParticleGeometrieDefault]`、callback4DDFA0、512rows／100columns。REA驗4DDF60的16-entry dispatch table與5F6B2D的formats：numeric `%ld`、name `%s`。column0設定ID selector；column1 activ寫819FCC；columns3..15寫+4..+34連續signed32；name另在+38、stride58(hex)。沒有把heigh／ysub等fixed raw值轉成float或推測runtime geometry。

Rust exact callback window4DDFA0+46D與wrapper4DE410+5C已實際解碼，`particle-geometry-callback-20261013.json`。首次4DDF00+640穿過jump table，在4DDF60被decoder拒絕、未產生JSON；改用由table與prologue證明的entry，沒有放寬decoder或重跑相同失敗窗口。

REA read_bytes驗4B1130的209-entry objdef table；Rust exact case windows保存`particle-object-fields-20261013.json`（source bytes再讀相同）：

| source column | callback / writer | runtime address (type stride2A4) | width |
|---|---|---|---|
| 1 activ | 4B14D5 / 4B14FD | C648BC | byte AL |
| 2 alen | 4B150C / 4B1534 | C648C0 | dword EAX |
| 5 alrid | 4B15B1 / 4B15D9 | C648CC | word AX |
| 63 papar | 4B2264 / 4B228C | C64928 | word AX |
| 64 pageo | 4B229C / 4B22C4 | C6492A | word AX |
| 65 pagfx | 4B22D4 / 4B22FC | C6492C | word AX |

4DCFC0既有完整renderer讀同一组欄位，使用gfx.obdef的object type之C648C0作divisor、C648CC作ALR ID。**owner.alen不是此divisor**。其unaligned DWORD+SAR16讀papar/pageo/pagfx與上述word writers吻合。

## Product contract

`geometry_definitions`沿用strict ASCII／PFIL parser，保留14raw dwords；inactive與negative值不改寫。`object_particle_definitions`要求完整209欄base row、ID0..2499、unique ID，且idx/activ/alen/alrid/name/papar/pageo/pagfx位於已證ordinal；只解析六個numeric fields與source name，其餘float/string欄不解釋或驗證。header用作產品schema驗證，不宣稱game按header names dispatch。

`ObjectParticleDefinition.numeric_raw`保持全部signed32輸入；getter用native低byte／低word取值，再將word視為signed16。例activ256→native0、alrid65535→-1、papar65558→22。這是writer-width效果，不把輸入clamp或直接套int32 references。

`definition_chain(owner_type, objects, parameters, geometry, grafiks)`按owner的三個ID各找自己的table，驗產品active guards，然後gfx.obdef找primary object。返回借用原始definitions；不同table同名／不同名字不影響ID解析。gfx.obdef==-1明列`NoPrimaryGrafikObject`，其他缺失／inactive分別回報；不拿owner ALR代替primary物件，也不聲稱所有secondary particle paths因此不存在。

chain.frame_context由parameter timer/swarm/heisen與**gfx object alen**組成，ALR ID亦由gfx object取得。columns/directions/frame_count仍由caller提供matching resident asset observations；沒有把source header或unloaded defaults當原遊戲resident證據。完整visibility、object runtime active、sort、secondary geometry routes與palette/blend尚未閉合。Owner active guard是產品拒絕域，不能視為已證原renderer完整predicate。

## Verification

- 5新Rust tests通過：geometry raw layout、byte/word narrowing、異名ID chain、owner/gfx object divisor差異、missing/inactive/sentinel、selected header/value/duplicate拒絕；包含原回歸共80pass/0fail。Log `re_workspace/particle-definition-chain-tests-20261013.log`；未執行被Windows policy封鎖的ALR metadata test，沒有改policy。
- Actual Rust `--definitions` CLI（offline/locked）解析100partpar＋102partgfx＋102partgeo＋2160objdef rows；三個particle tables所有rows與objdef251-row oracle subset共5048raw words全一致，names亦一致。全部objdef row count已核對；其餘1909objects的words沒有獨立inventory比較，不冒稱全2160word differential。
- `re_workspace/particle-definition-chain-final-20261013.json`：169個帶particle references的owner type，149條primary鏈解析成功、20條明列no_primary_object，沒有其他unresolved。例830→parameter22／geometry23／gfx24→object227→ALR258，parameter lifetime15000、gfx object divisor1000。這是base-definition chain，不是169個native draw/frame驗收。
- 四個installed source檔SHA256／length／mtime不變；沒有game/ISO writes或game launch。
- solution Release build0errors/41warnings；既有.NET RustParticlePoolTests12pass/0skip/0fail，TRX `particle-chain-tests-20261013/23_16_39`。此專項只驗既有pool及installed資料差分，並非新definition chain的C#接線。新Rust source與probe cargo fmt check通過。

接續工作：DefaultPath／overlay確切合併、definitions/frame FFI與host；matching ALR metadata、velocity/direction x87、gfx mode／secondary paths與真遊戲pixel對照仍未完成。

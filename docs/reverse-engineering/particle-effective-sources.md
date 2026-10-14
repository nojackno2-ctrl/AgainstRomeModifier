# Effective particle definition sources

2026-10-13 Codex。Rust新增checked loose-file source snapshot與overlay合併，C#可用`RustParticleBaseDefinitions.LoadGameSources(absoluteRoot)`取得effective catalog。MapEditorForm與particle renderer尚未呼叫此factory，沒有完成particle visuals主張。

## Native evidence and defaults

既有完整480850（`object-definition-source-20261009-evidence.json`）在初始化後先載四表dau，再於counter0..9按objdef／partpar／partgeo／partgfx各載一次layer。Rust raw-byte probe核對5F33A1..5F33CD：`partpar.d%02ld`／`partgeo.d%02ld`／`partgfx.d%02ld`，`particle-overlay-format-20261013.json` source unchanged；非根據symbol中的underscore猜檔名。

新REA `particle-source-overlay-20261013-evidence.json`：13records，SHA256 `accb1e89cbf0dcc527bd846225062dc7d9afd21b98514a1d79c03e790ae9ec5b`。subject EXE SHA256仍`6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`；session已close。

- 4DDE90→4DDEB0→4DDEE0：512parameter records，14dwords全0，name PP_NoNameEntered。reset body Evidence `ev_19130959229dc42f6cc8ce97c8b9dc67d192bf35cf1ece7aaca28dae7d259fa3`。
- 4DE590→4DE5B0→4DE5E0：512geometry records，14dwords全0，name PG_NoNameEntered。`ev_9b34064f7f33a79a1ecda2d52477df21584c71e05c0052f6972f4ceb66e765bb`。
- 4DEA20→4DEA40→4DEA70：512grafik records，numeric `[0,-1,-1,-1,-1,0,0]`，name PX_NoNameEntered。`ev_a3bbf1925966ed0bdff8626616871cef0ea104a9e96492304f20519372a31713`。
- 4AFDC0→4AFDE0→4AFE10：2500objects；selected fields active0、alen1、alrid/papar/pageo/pagfx=-1、name OD_NoNameEntered。`ev_6c5229cb39823c5923acd0f2fe4ff6f9da940f99efe1e722b02c97c7b8d432b5`。只重建本parser選定欄位，沒有重建其餘type fields。
- 567D50會略過delimiter runs；42A230只針對出現的token依column ordinal呼叫callback。`ev_6449ff04bad66279de7a4fa45f5f9af99a36838c6d29d21bd122a3aa470e8607`。`1,,2,Name`對應tokens1/2/Name，空comma token不是「保留這一欄」的CSV語義；省略尾部欄位才保留previous value。

## Source policy and ownership

`particle_definition_sources::load`要求absolute game root，解析raw/PFIL ASCII `SYSTEM/clengine.ini`的DefaultPath。存在config卻缺section／空或歧義prefix會拒絕；config不存在時沿用產品原有`SYSTEM/DATA_MP/DEFAULTS/` fallback。prefix為literal filename concatenation，不自動加separator；slash按host標準化。絕對／root外sources、non-ASCII text／NUL與malformed selected numeric目前產品拒絕，不宣稱這是native VFS的同等限制。

預先初始化所有ID，再套dau及d00..d09；missing optional files是no-op，存在且不可讀或有不合法相關欄位則整體失敗。每layer以staged clone合併，不重新初始化被省略的欄位；同ID重複row依序更新。只處理已驗numeric欄位，未選object fields不解釋；name labels採ASCII token，不能據此聲稱完整native name-buffer重建。Missing target section為native-style no-op。每個實際讀檔保存canonical path／SHA256／length／mtime，load結束全部重查，變更時不交付部分snapshot。snapshot不監看後續檔案變更。

新增ABI `ar_particle_definition_load_utf8`與`ar_particle_definition_source_count`：owned handle／query44bytes／free規則維持原介面；root/source/encoding失敗code226且*out=null。source count含clengine，caller-buffer base open為0。C#factory僅傳UTF8root，不重寫DefaultPath parser／layer數值合併。失敗不影響已持有舊handle；後續需要以新factory重新載入，才反映source變更。

## Verification

4個Rust source tests全pass：custom prefix與d00/d09 precedence、omitted欄保持、新ID初值、delimiter runs、missing-config限定fallback、present corruption拒絕、literal prefix。新增root ABI boundary test與原ABI tests驗invalid UTF8／relative root／null／failure clear／atomic count；全core回歸87pass/0fail，log `effective-definitions-rust-tests-20261013.log`。沒有重跑policy-blocked metadata test或改policy。

Release candidate DLL SHA256 `a23e6e045c998d5edc8cc0ddd54099d597785b376f57bec2870e619c51f8fa66`。實際C#factory讀installed PFIL config及sources，count>=5且830之有效定義與base相同；TEMP custom prefix成功、corrupt d09拒絕新handle、旧handle仍可query。focused Modules3／Host12pass，0skip/0fail，TRX `effective-definition-tests-20261013/23_52_16/19`。Full Modules1073pass/2skip/0fail（23_58_13）；solution build0errors/41warnings，增量final build0errors/15warnings。

這些是Rust source snapshot與真DLL/managed API驗證，不是原game loader執行、VFS／archive backend重建、完整Host suite或particle pixels。接續MapEditorForm生命周期採用此source factory，再補frame FFI／matching assets／velocity-direction FPU／palette-blend與renderer。

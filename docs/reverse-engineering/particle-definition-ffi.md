# Particle definitions C ABI and managed bridge

2026-10-13 Codex。新增四個caller-supplied base file images的owned Rust catalog，與MapEditor.Modules薄C# bridge。這使宿主可呼叫已驗primary definition chain；尚未在MapEditorForm載入流程或particle renderer採用，沒有完整particle visuals主張。

## Contract

- ABI version1，`ArParticleDefinition`44bytes（11個4-byte fields）：struct_size、owner_type、parameter_id、geometry_id、grafik_id、grafik_object、alr_id、lifetime、type_divisor、velocity_branch_raw、skip_first_raw。signed flags保留raw，ALR signed16轉signed32。
- `ar_particle_definition_open(par,par_len,geo,geo_len,gfx,gfx_len,obj,obj_len,out)`接受raw／strict PFIL來源；解析成detached catalogs。caller保留buffers所有權；failure清*out。非空inputs可讀，out可寫且不與inputs重疊，length不得超過isize slice extent。
- `ar_particle_definition_query(handle,owner_type,out)`要求*out.struct_size=44；先完整resolve，再一次填output，任何錯誤不改output。既有null/size/panic codes保留。220/221/222/223分別是parameter/geometry/grafik/object parser拒絕，224是primary chain missing/inactive，225明確表示gfx.obdef=-1。
- `ar_particle_definition_free(handle)` exactly once，null無操作；caller同步free與queries。沒有interior pointers、filesystem reads、source selection、overlays或asset residency inference。
- C header與build-native-data腳本補齊5個真export、ABI／struct size檢查。

`RustParticleBaseDefinitions`只傳byte arrays與接收scalar DTO，不重寫parser／ID join／numeric frame。NativeLibrary resolver接受絕對`ARM_NATIVE_DATA_DLL`，未設定時找AppContext.BaseDirectory旁的DLL。每instance的SafeHandle持有Rust handle及一次library reference，先Rust free再release library；query與Dispose同lock，closed後拒絕query，重複Dispose無操作。constructor失敗回收已開handle及library。224/225為不同diagnostic，沒有owner sprite或frame0 fallback。

## Verification

- Rust新增2ABI tests全pass：detached empty catalogs、錯誤時output全欄不變、bad struct size、null、impossible length、四個parser error codes與failure clears handle、free(null)。含原core回歸82pass/0fail，完整log `re_workspace/definition-ffi-rust-tests-20261013.log`。metadata policy-blocked executable未重跑，未改policy。
- 真Release candidate DLL `re_workspace/particle-definition-ffi-check/against_rome_data.dll` SHA256 `54e899c1472c2da7fc59cd17876b8118d237e1e2904418ae819e8e1a80679508`，builder LoadLibrary/GetProcAddress驗exports與44byte size。
- C#實際DLL tests讀四個installed PFIL檔，逐一query0..2499：149primary／20no-primary，其餘missing/inactive；878為明確no-primary。830回傳22/23/24→227→ALR258／life15000／divisor1000／velocity1／skip0，Marshal field order已由不同值確認。
- 覆寫所有傳入source arrays後query仍相同；Dispose兩次成功、後續query丟ObjectDisposedException；四個installed source SHA256/mtime不變。empty headers由真正Rust parser拒絕。
- focused solution Modules12／Host12pass，0skip/0fail（兩個definitions tests＋原ALR10／原particle pool12），TRX `definition-ffi-tests-20261013/23_27_58`與`23_27_59`。新bridge測試要求ARM_GAME_PATH與ARM_REQUIRE_NATIVE_DATA=1；没有以缺來源的早退作驗證。
- solution build修正新test遺漏`using Xunit`的CS0246後成功，0errors/15warnings。這不是full Host suite或原game runtime／pixel驗收。
- 完整Modules：1072pass/2skip/0fail，TRX `definition-ffi-full-modules/23_29_57`；兩個原有Cinematics skip不作通過。

下一步：宿主明確選取DefaultPath及overlay合併後才採用catalog；補frame resolver FFI、matching resident asset、velocity/direction與palette/blend/visibility，再接particle renderer。caller-supplied base API不能悄悄忽略已存在overlay。

# Particle definition catalog host lifecycle

2026-10-14 Codex。`MapEditorForm.LoadSelectedMap`現在載入Rust effective definitions，null selection與Dispose釋放catalog。選取地圖時實際呼叫DefaultPath／overlay source factory，不再僅有離線與module-level API。尚未在particle renderer採用frame／assets／blend。

新增`MapEditorForm.ParticleDefinitions`partial：每次load重新取得candidate，成功才dispose舊handle並採用；map key為完整路徑去尾separator、Windows case-insensitive。不同map先clear；同map失敗保留舊handle但保存diagnostic，`TryResolveParticleDefinition`先拒絕diagnostic／wrong-map／missing catalog，不會把舊資料當當前有效定義。成功repair reload恢復query並釋放舊handle。全部parser／source選擇仍在Rust，host只處理ownership與候選採用。

初次TEMP/STA測試1pass/1timeout，起初推測corrupt objdef干涉既有map parser；改partpar仍timeout後確認真正原因：新增catch缺少`InvalidDataException`，它不屬於`IOException`，Rust source rejection落入整個map-load錯誤對話框。已補filter，沒有放寬STA timeout或改測試期望來假裝通過。初次objdef歸因不完整，以上定位取代之；沒有宣稱修復既有所有modal load errors。

最終三個lifecycle tests：

- TEMP custom DefaultPath，gfx object divisor1000→objdef.d09覆寫250，完整LoadSelectedMap後見新值／舊handleclosed；Dispose後current也closed，地圖檔案snapshot不變。
- partpar.d09 invalid integer，完整LoadSelectedMap保留same handle但拒絕stale resolution；修復timer5000並reload恢復查詢，old closed。
- 換到另一個TEMP map釋放第一個handle並採用新catalog；null selection清除第二個handle，不能跨map繼續query。

`particle-definition-host-repaired` TRX `re_workspace/particle-definition-host-tests-20261014/00_17_04`：3pass/0skip/0fail，4s，forced native DLL；solutionbuild0errors/7warnings。實際Rust handle透過managed bridge查詢closed時丟ObjectDisposedException，不以field=null alone當釋放證據。只寫TEMP fixture source files，沒有installed game/ISO writes。

宿主回歸filter `MapEditorSaveTransactionTests|RustParticle|RustLight`（forced native/OpenGL）：264pass/0skip/1fail，6m31s（00:25:27），唯一failure為漏設ARM_NATIVE_DATA_DLL的explicit-path gate，未把它當functional regression或全suite綠。補明確path後重驗gate＋final自帶fixture／oldhandle仍可native query斷言的三lifecycle tests：4pass/0skip/0fail（`particle-definition-host-final/00_26_19`），build0errors/4warnings。已通過的長回歸沒有重跑；沒有全Host成功主張。

接續：在closed-family saved particle候選中使用host definition query，補Rust frame resolver ABI、matching asset metadata與pixel modes，處理x87 velocity/direction、visibility／sort與遊戲畫面对照。本階段沒有particle sprite/frame/blend完整驗收，也沒有全Host suite成功主張。

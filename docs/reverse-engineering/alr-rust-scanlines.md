# Rust ALR scanline integration

2026-10-13 Codex。`NativeAlrDocument.DecodeFrame` → `NativeAlrIndexedFrame.Decode` → `RustAlrFrameDecoder` → `ar_alr_decode_argb`，目前編輯器既有 ALR sprite／動畫格的掃描線解碼已改用 Rust。容器讀取、first-record palette、frame selection與anchor仍維持C#既有契約，沒有另猜particle frame或native palette dispatch。

Rust `alr_scanlines` 處理descriptor低20bit offset／bit20雙段／bits21..30前透明gap／bit31第一段zero opaque；雙段prefix第二byte低7bit gap與高bit第二段zero opaque。每段仍檢查所有palette index，即使zero透明；palette `00BBGGRR`轉editor `FFRRGGBB`，未填區域保留ARGB0。這是編輯器既有still-preview契約，不是native遊戲blitter的alpha／half-average／render mode。

新增ABI version1與`ar_alr_decode_argb(width,height,rows,row_count,data,data_len,palette,palette_len,output,output_capacity)`。長度均為元素數；width/height≤2047、rows=height+1、palette1..511。先完整解碼到owned暫存，再寫width×height個caller words；錯誤不寫任何output、tail保留，output可與input重疊。0成功／1null／2容量或slice extent／6panic／210無效scanline。非空region有效性與u32 alignment仍由caller保證。

C#橋接只做輸入公共exception契約、buffers、ABI與回傳碼；無scanline arithmetic fallback。`ARM_NATIVE_DATA_DLL`接受絕對路徑，未設定時載入AppContext.BaseDirectory旁的DLL；成功binding保留一次process-lifetime reference，失敗free並允許重試。舊DLL缺少exports會回報InvalidDataException，現有catalog可按既有失敗路徑拒絕sprite。部署必須使用新版DLL，不能只更新managed assemblies。

## Evidence

- Rust新增4scanline tests成功，含不對稱雙段／兩種zero flags／RGB交換／late error原子輸出／tail／空frame與null／長度邊界。連同原light/particle/pixel/projection回歸共66pass/0fail。未重跑Windows policy阻擋的ALR metadata executable；沒有改policy。
- `tools/build-native-data.ps1 -OutputDir re_workspace/alr-scanline-check`成功，Release DLL SHA256 `2b97108ca7ce8613032d409e690997907d1aaaa0b4fcab4b8e991813a98af999`；真GetProcAddress確認scanline exports與version1。
- `dotnet build AgainstRomeModifier.slnx -c Release --no-restore -p:NativeDataDll=<absolute candidate DLL>`成功，0errors/41warnings。
- focused solution tests：Modules122pass／Host17pass，0fail/0skip；該filter沒有涵蓋partial class中的GL tests，另用MapEditorSaveTransactionTests執行。
- 完整Modules：1070pass/2skip/0fail。Host filter `MapEditorSaveTransactionTests|RustParticle|RustLight`：262pass/0skip/0fail，6m28s；`ARM_REQUIRE_NATIVE_DATA=1`、`ARM_OPENGL_REQUIRED=1`、`ARM_GAME_PATH=<installed root>`與candidate DLL均設定，包含真正OpenGL cases。TRX在`re_workspace/alr-scanline-tests-20261013`（full-modules 22:34:18、host 22:40:47）。這是指定Host回歸，並非全Host suite／原遊戲像素等價。
- 本機`dotnet publish ... --no-build --no-restore -p:NativeDataDll=<candidate DLL>`成功，`re_workspace/alr-scanline-publish-check`內DLL SHA256與candidate相同；非對外release。
- `tools/re/alr-scanline-differential`保存e3a21d9的C#容器／scanline／palette oracle，只作驗證，不在產品路徑。以`particle-assets-parent-20261012.json`的member SHA256核對archive每個entry，拒missing/ambiguous/identity mismatch；逐一比較所有frame及palette variant。
- 實際CLI：`ARM_NATIVE_DATA_DLL=<candidate DLL> dotnet run --project tools/re/alr-scanline-differential/alr-scanline-differential.csproj -c Release -p:NativeDataDll=<candidate DLL> -- <game-root> re_workspace/particle-assets-parent-20261012.json re_workspace/alr-scanline-differential-20261013.json`成功。68ALR、11345frames、14996decodes、10193112pixels、0mismatch；來源alr.dat SHA256/length/mtime不變。JSON只存identity與統計，沒有匯出遊戲pixel assets。

## Remaining scope

這是behavior-preserving Rust decoder移植。完整particle子粒子的saved fields→gfx→frame/direction/palette/blend/visibility接線尚未完成；原game runtime pixels對照也未驗。ALR header/loader normalization與未解trailing bytes沒有因本結果變成已完成。既有dirty其他C#功能與ALR metadata source保留，未纳入本移植提交。

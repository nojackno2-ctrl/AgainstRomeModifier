# Saved particle frame resolution

2026-10-13 Codex。新增`native/against-rome-data/src/particle_frames.rs`純Rust frame core，尚未FFI／host接線，不部署DLL。它是完整particle preview的必要步驟，尚非完整preview或native visibility判定。

## Source evidence

現場REA重新開啟installed `Against_Rome.exe`，SHA256 `6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`。新export `re_workspace/particle-definition-gates-20261013-evidence.json`（6records，SHA256 `4ed7e26fd58784e9da2a4d78d87b9ea81915b82eba78c0d0587f397faaa0b18a`）：

- 4DDE40：parameter ID signed0..511且`80EFCC + id*58`非零，才有效。
- 4DE540：geometry ID signed0..511且`819FCC + id*58`非零；Evidence `ev_633ee848a06a97184258adfe5557374667f536db7cacd5a0095320e45bde7858`。
- 4DE9E0：gfx ID signed0..511且`824FCC + id*3C`非零；Evidence `ev_d30853635918bee5ef060e0d649922e3b4a0172e83b8b8994bae527a7f9d790f`。
- 430330名稱getter另要求parameter active==1，不可把它當以上validator；Evidence `ev_21d6198d052080ce69f2317997b94ec4cba6c0b260fb249c7f8f314190cd67d3`。
- 80EFCC的直接xref 4DD927未被Ghidra收為procedure；一次analyze失敗後用resolve確認not_in_procedure。沒有猜loader entry、修改database或把此處當成功解析。REA已close。

壽命分支使用既有完整4DCFC0（Evidence `ev_af0d6b826e20f7c857c52868bf473845cfb3a5bbdcbbcef3fcde4a64787a6963`），export `particle-render-consumers-20261012-evidence.json` SHA256 `a2f395d19816bef29883b92d5f542c78a8c60427ce4e618817d99dbad54adcb1`。既有assembly／pseudocode重讀核對，沒有重跑整個renderer分析。

## APIs and invariants

`FrameContext`的definition lifetime來自80EFD4、skip-first來自80EFEC、velocity route來自80EFFC（各parameter stride58）；divisor來自gfx object type的C648C0（stride2A4）。columns／directions必須是**已載入資源**4AF200／230的觀察值。caller須建立正確definition／gfx object／asset identity，本API不把同名或ordinal視為關聯，也不接受unloaded default1作為成功解析的證據。

`lifetime_frame(saved_lifetime, context)`只在velocity raw==0且directions==1的壽命路徑回傳frame：

```text
n = signed32(low32((definition_lifetime - saved_lifetime) * columns))
q = signed IDIV n / type_divisor
frame = signed IDIV q % columns
```

SUB／IMUL明確wrapping，不用無限精度或saturating替代；IDIV向零截斷、remainder與dividend同號。divisor0／MIN÷-1回報不同錯誤；negative frame（包括-1）不轉成owner動畫fallback。columns／directions≤0或frame_count0是產品拒絕域；非native validation主張。frame bounds用實際frame_count檢查。

`saved_frame_candidates(snapshot, optional_context)`：inactive emitter無candidate；active count非0..64拒絕整個emitter；已驗context.skip_first>0時從1起，否則從0起；只取count範圍內nonzero child flag。返回ordinal、position/velocity signed SAR16、raw lifetime及frame結果。保存snapshot完全不變，未納入candidate的child/light links仍保留。context缺失時只返回潛在active candidates，frame明列MissingContext，沒有假定skip-first或frame0。

MissingContext／InvalidLayout／VelocityRoundingUnverified／DirectionRoundingUnverified／ZeroDivisor／DivisionOverflow／NegativeIndex／FrameOutOfRange為不同結果。Resolved frame只是索引運算成功，沒有宣稱native renderer一定draw；native sort、asset／object／type gates、palette／mode／visibility仍須完整接線。

## Verification and remaining work

5新Rust tests全pass：life進度與wrap、錯誤分流、count／active／skip-first（正負原值）與source/link保持、negative fractional SAR16。另有x86_64 inline assembly實際執行SUB／IMUL／CDQ／IDIV／CDQ／IDIV，10000組正負divisor及raw32 operands與Rust一致。測試避免divide exception輸入；陷阱域另驗checked Rust拒絕。這是CPU instruction differential，**不是執行原遊戲renderer**。

原target `cargo test --offline --locked --manifest-path native/against-rome-data/Cargo.toml --lib --test light_pool --test particle_pool --test projection --test particle_projection --test particle_pixels --test particle_pixel_ffi --test alr_scanlines --test particle_frames`：71pass/0fail。未重跑受policy阻擋的ALR metadata test，未改policy。solution增量Release build0errors/0warnings。

下一步：驗證三個text loaders把partpar/partgeo/partgfx欄位寫入上述runtime definitions的精確映射，再接owner-type→parameter/gfx object→resident asset。速度／方向路徑仍缺x87 rounding／FPU證據，不能用近似atan或一般round替代；其後補FFI／宿主particle sprites、mode palette/blend、visibility與原game畫面驗收。

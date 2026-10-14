# Particle parameter and gfx loader mapping

2026-10-13 Codex。Rust `particle_definitions`新增base `partpar`／`partgfx` parser與parameter→既有frame resolver context。沒有FFI／宿主／部署改動；完整particle preview仍未完成。

## Binary evidence

installed EXE SHA256 `6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`。新REA export `re_workspace/particle-definition-loaders-20261013-evidence.json`：12records、SHA256 `7f7a1fe388ad5975fb59a6356b4290912cf4de230331ff60d294a4702cecfd00`，session已close。

- 4DDD10（完整92bytes）把section `[ParticleParameterDefault]`、callback4DD8A0、512rows、100columns與delimiter`,`傳42A230。Evidence `ev_f144b29b1d2e66f079c539d32c4d341fc8dfa398ee0f97dfa3b4ce17dacb0c68`。
- 4DE8D0（完整92bytes）同樣註冊`[ParticleGrafikDefault]`、callback4DE690。Evidence `ev_78285c628b08970054e427723d2a259bfa7d8502a1fe444b68da1014f7f01e14`。
- 42A230 token loop把token、row ordinal、column ordinal傳callback。真正record ID由column0的`%ld`寫入global selector，而非row ordinal。Evidence `ev_fa29a1002d7a97cd3f1a33d836e819c46ef67aa4af02176ccb3c26c2d162154d`。
- 4DD8A0 callback dispatch table4DD860有16entries；4DE690 table4DE660有9entries，均由REA read_bytes驗64/36bytes。格式字串numeric=`%ld`（Windows signed32）、name=`%s`；callback不以header names尋址，列號才是實際mapping。

Rust static probe以明確VA與原始EXE bytes解析，沿用既有Pe/iced-x86 library，subject hash不符即拒絕。最終四個windows是4DD8A0+46D、4DDD10+5C、4DE690+235、4DE8D0+5C，均从有效entry到RET。`re_workspace/particle-definition-final-windows-20261013.json`：actual CLI成功，source bytes再讀相同。初探索窗跨padding／從4DE600 instruction中段起的線性解碼含無意義指令，**不採用**該區作語義證據；final windows另存，沒有修改原EXE／Ghidra database或重試前次不存在的procedure。

## Field contract

| Table column | parameter runtime offset | writer VA | renderer use |
|---|---|---|---|
| 1 activ | +0 (80EFCC) | 4DD927 | validator非零 |
| 3 maxam | +4 | 4DD9B7 | raw保留 |
| 4 timer | +8 (80EFD4) | 4DD9FD | definition lifetime |
| 10 swarm | +20 (80EFEC) | 4DDBA1 | >0時跳過child0 |
| 14 heisen | +30 (80EFFC) | 4DDCB9 | 非零選velocity frame branch |
| 15 refint | +34 | 4DDCFF | raw保留 |

parameter stride58(hex)。所有numeric fields以14個signed32 raw words保留（activ + columns3..15，連續+0..+34）；name source token另保留，未模擬native30-byte名稱buffer。

gfx stride3C(hex)，activ +0／column3 lolig +4／column4 obdef +8（824FD4，writer4DE7C7）／column5 expde +C／column6 objpa +10／column7 objpv +14／column8 objpp +18。7個numeric words全保留；`object_definition()`只返回column4，不按同名或ordinal代替ID。

`ParameterDefinition.frame_context(type_divisor, columns, directions, frame_count)`只對nonzero active成功，lifetime／skip-first／velocity直接採上述raw fields；type divisor仍須來自**gfx object type**，resident asset metadata仍須由caller建立identity。這不代替geometry／object／asset gates，不把unloaded default1當已解析資源。

## Parser domain and verification

`parameter_definitions`／`grafik_definitions`接受strict PFIL或raw ASCII。要求一個exact section及exact欄位header、完整column count、unique ID0..511、全部numeric signed32、name nonempty single ASCII token。inactive與negative numeric／sentinel照樣保留；整個catalog有任何錯誤即拒絕，不交付partial結果。header用作產品schema validation，並非原game callback依header names解析。此產品域不模擬原loader對malformed／duplicate／overlong／non-ASCII資料的行為，不處理overlay precedence／native resets/defaults或geometry table。

4新tests全pass：parameter各raw position／nonboolean active／負skip與velocity；gfx column4／inactive／negative sentinel；invalid ID／overflow／duplicate／缺header／column count／encoding／name拒絕；真parser→lifetime_frame結果與inactive context拒絕。包含原light/particle/scanline/frame/pixel回歸共75pass/0fail，log `re_workspace/particle-definitions-tests-20261013.log`。ALR metadata policy-blocked test未執行，沒有policy change。

Actual Rust CLI `--definitions <installed root> re_workspace/particle-assets-parent-20261012.json re_workspace/particle-definitions-final-audit-20261013.json`成功：100parameter＋102gfx rows，2114個raw dword與**之前獨立inventory parser**的raw_values全一致；names亦比對，2source hash/length/mtime不變。沒有native text-loader execution或game pixels主張。probe `cargo fmt --check`成功，兩種CLI均以`--offline --locked`實際執行。

solution Release build0errors/41warnings；既有.NET RustParticlePoolTests12pass/0skip/0fail，TRX `particle-definition-tests-20261013/23_01_38`（只驗既有pool與installed differential，不是新definitions的C#接線）。

下一步：geometry callback與objdef C648C0 lifetime divisor／ALR ID的exact mapping，再接Rust definitions→FFI→host sprite inputs。速度／方向x87、gfx模式palette/blend、visibility／sort與原game畫面驗收仍未完成。

# EldenRingMod

Cargo workspace gộp các mod Elden Ring viết bằng Rust vào 1 repo duy nhất,
mỗi mod là 1 crate `cdylib` riêng, dùng chung 1 crate `common` cho phần
plumbing lặp lại giữa các mod (đọc/ghi ini, log ra file, AOB pattern scan).

## Cấu trúc

```
EldenRingMod/
├── Cargo.toml       # workspace root - liệt kê member crates + dependency dùng chung
├── builds/          # builds/<crate>/ chứa .dll + .ini (+ .zip) của từng mod (git-ignored) - xem `scripts/build-mod.ps1`
├── mods/            # mỗi thư mục con = 1 mod (thường là 1 crate Rust build ra 1 DLL; có thể thêm mod chỉ có dữ liệu Smithbox)
│   ├── autoregen/        # AutoRegen: hồi HP/FP/Stamina theo tick + khi đánh trúng/bị đánh trúng
│   ├── dropmultiplier/   # DropMultiplier: chỉnh tỉ lệ rớt đồ của quái (nhân hệ số hoặc ép tỉ lệ tổng)
│   ├── fasterrevival/    # FasterRevival: rút ngắn thời gian từ lúc hết máu tới hồi sinh (đang nghiên cứu)
│   ├── infiniteailment/  # InfiniteAilment: chỉnh thời lượng + sát thương Poison/Scarlet Rot người chơi gây ra
│   ├── passiverunes/     # PassiveRunes: cộng rune theo thời gian + bonus mốc thời gian
│   ├── risearcher/       # RiseArcher: buff Bow/Crossbow/Ballista/Arrow/Bolt qua SoloParamRepository sống
│   ├── runemultiplier/   # RuneMultiplier: nhân hệ số rune từ mọi nguồn qua 1 hook AddSoul_Call
│   ├── sometweaks/       # SomeTweaks (đã bỏ, không phát triển nữa): gộp nhiều QoL
│   ├── soulsteleport/    # SoulsTeleport: dịch chuyển tới người chơi khác trong Seamless Co-op
│   ├── speedmultiplier/  # SpeedMultiplier: tăng tốc di chuyển / tấn công / thi triển phép
│   ├── spiritmultiplier/ # SpiritMultiplier: nhân số linh hồn (spirit ash) triệu hồi vượt trần 10 con
│   ├── weightmultiplier/ # WeightMultiplier (đổi tên từ ReductionWeight): nhân hệ số Trọng Tải qua 1 hook code
│   └── windowresize/     # WindowResize: kéo viền đổi kích thước cửa sổ Windowed nhỏ hơn mức mặc định
├── shared/          # crate `common` dùng chung: config/logger/memscan/dll_dir, KHÔNG phụ thuộc eldenring-rs
├── scripts/         # công cụ chạy được: build-mod.ps1 (build 1 mod → builds/<mod>/), ida/ (IDAPython qua run.ps1), ghidra/ (script tham khảo)
├── docs/            # tư liệu dùng chung: nexus-page.bbcode, logo/thumbnail prompt, nexus-openapi.yaml
├── dumps/           # dữ liệu dịch ngược, git-ignored (database IDA ở eldenring/<ver>/, ~25 phút/bản nếu mất), ngoại lệ seamless-diff/ được commit
└── tmp/             # repo/DLL tải về, file tạm nghiên cứu (git-ignored, xoá được bất kỳ lúc nào)
```

`shared/` nằm ngoài `mods/` có chủ đích: `mods/` chỉ chứa các mod thật
sự (mỗi thư mục = 1 DLL xuất bản), còn `shared/` là hạ tầng dùng chung,
không phải 1 mod. Tên package Cargo của nó vẫn là `common` (không đổi
tên package theo tên thư mục) — mỗi mod vẫn `use common::...` như cũ.

Mỗi mod Rust (`mods/autoregen`, `mods/dropmultiplier`, ...) có:
- `Cargo.toml` riêng, `common = { path = "../../shared" }` + các dependency
  `eldenring`/`fromsoftware-shared`/`chrono` lấy version thống nhất từ
  `[workspace.dependencies]` ở root qua cú pháp `xxx.workspace = true`.
- 1 file `.ini` cùng tên crate, nhúng vào binary lúc build (`include_str!`) -
  đây là nguồn sự thật duy nhất cho cấu hình mặc định.
- `src/lib.rs` mỏng: chỉ có `DllMain` + gọi `common::config`/`common::logger`,
  logic thật nằm trong module riêng của mod (`src/regen/`, ...).
- `README.md` (trạng thái hiện tại: key, cách hoạt động, giới hạn) + `HISTORY.md`
  (dòng thời gian, mỗi thay đổi 1 dòng) + các phân tích chi tiết ở
  `mods/<mod>/docs/<chủ đề>.md` - quy ước đầy đủ trong `CLAUDE.md`, khuôn mẫu là
  `mods/dropmultiplier/`. Mod chưa được tách vẫn còn README kiểu nhật ký dài.
- `CHANGELOG.md` (ghi chú phát hành cho người dùng, có mục `[Unreleased]`) và
  `nexus_page.bbcode` (mô tả trang Nexus, không chứa changelog) khi mod đã publish.

## Vì sao tách `common` thay vì copy-paste

`config.rs`/`logger.rs`/`memscan.rs` từng bị copy gần như y hệt giữa các mod
(AutoRegen, LifeBetween cũ, và các mod C++ PassiveRunes/ReductionWeight/
RuneMultiplier) - sửa 1 bug ở 1 bản không tự động lan sang các bản còn lại.
Cùng lý do đó, `common` còn có `input::parse_virtual_key` (parse hotkey từ
ini, dùng chung bởi `autoregen`/`sometweaks`/`runemultiplier`) và
`codepatch::install_jmp_hook` (JMP tương đối 5-byte + tìm vùng nhớ gần,
dùng bởi `weightmultiplier` khi patch site không đủ ~12 byte cho absolute
jump).
`common` giải quyết việc đó bằng path dependency trong cùng workspace - khả
thi vì giờ toàn bộ mod nằm chung 1 repo, không còn là các repo Git tách biệt
như trước (không gặp vấn đề "clone 1 mod thì thiếu `../common`").

`common` cố tình **không** phụ thuộc `eldenring`/`fromsoftware-shared`: đây
là plumbing chung chung (ini, log, AOB scan) không liên quan gì đến struct
riêng của Elden Ring, tách biệt hoàn toàn khỏi phần logic đọc/ghi game state
nằm trong từng mod.

## Build

```powershell
cargo build --release              # build tất cả mod trong workspace (ra ở target/release/)
cargo build --release -p autoregen # chỉ build 1 mod
cargo test --workspace             # chạy unit test (chủ yếu là config::migrate)

pwsh scripts/build-mod.ps1 -Mod AutoRegen   # build + copy AutoRegen.dll/.ini vào builds/autoregen/
```

Hoặc dùng task có sẵn trong VS Code (**Ctrl+Shift+B**) - mở danh sách chọn:
`Cargo: Build AutoRegen/SomeTweaks/PassiveRunes/RuneMultiplier/
WeightMultiplier/RiseArcher (Release)` build riêng 1 mod, `Cargo: Build All
(Release)` chạy hết. Mỗi task build xong đều tự copy
`.dll` + `.ini` của mod đó vào `builds/<crate>/` (vd. `builds/autoregen/`,
không lẫn vào hàng trăm file khác trong `target/release/`; file `.zip` khi
`-Zip` cũng nằm cùng thư mục) - mỗi mod 1 thư mục riêng, dễ tìm và copy sang
thư mục mod loader.

## Trạng thái

- `autoregen` - đầy đủ tính năng gốc (Regen theo tick + OnHit) cộng thêm
  OnDamage (hồi máu khi bị đánh trúng).
- `sometweaks` - port của LifeBetween, mới có module Regen; Rune/Spirit/Misc
  trong ini vẫn là placeholder, chưa có code tương ứng.
- `passiverunes` - port của PassiveRunes: cộng rune qua
  `GameDataMan::main_player_game_data.rune_count` (thay AOB pointer chain tự
  dò tay) + bonus mốc thời gian bằng threshold-crossing thay vì so sánh
  `==` tuyệt đối như bản C++ (không bỏ sót mốc nào nếu tick bị lệch nhịp).
- `runemultiplier` - port của RuneMultiplier: giữ nguyên hook `AddSoul_Call`,
  nhưng đổi sang jump tuyệt đối cả 2 chiều (kiểu `attack_hook.rs`) thay vì
  `E9 rel32` + tìm vùng nhớ gần bằng tay (`CodePatch.cpp` bị bỏ hoàn toàn).
- `weightmultiplier` - đổi tên từ ReductionWeight, port giữ nguyên hook +
  AOB pattern gốc. Patch site chỉ có 7 byte (không đủ cho absolute jump),
  nên đây là mod duy nhất còn giữ kỹ thuật `E9 rel32` + tìm vùng nhớ gần
  (`common::codepatch`) - không cần `eldenring`/`fromsoftware-shared`,
  không có hotkey reload (giữ nguyên quyết định thiết kế gốc).
- `risearcher` - giữ **cả 2 cách** song song: sửa tĩnh `regulation.bin` gốc
  (`csv/RiseArcher.MASSEDIT` + `csv/*.csv` + `.smithbox/`, dùng Smithbox)
  và DLL mới đọc/ghi `SoloParamRepository` sống qua `fromsoftware-rs`
  (không cần `libER` như kế hoạch gốc) - DLL không xung đột với mod
  `regulation.bin` khác, mọi hệ số đọc từ ini; 2 cách không tự đồng bộ với
  nhau, xem `mods/risearcher/README.md`. Mod duy nhất không cần `CSTaskImp` lẫn
  hotkey - patch 1 lần lúc `SoloParamRepository` sẵn sàng rồi thôi.

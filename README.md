# EldenRingMod

Cargo workspace gộp các mod Elden Ring viết bằng Rust vào 1 repo duy nhất,
mỗi mod là 1 crate `cdylib` riêng, dùng chung 1 crate `common` cho phần
plumbing lặp lại giữa các mod (đọc/ghi ini, log ra file, AOB pattern scan).

## Cấu trúc

```
EldenRingMod/
├── Cargo.toml       # workspace root - liệt kê member crates + dependency dùng chung
├── build/           # .dll + .ini của mọi mod, gom phẳng 1 chỗ (git-ignored) - xem `scripts/build-mod.ps1`
├── scripts/         # build-mod.ps1: build 1 mod rồi copy .dll + .ini vào build/
├── shared/          # crate `common` dùng chung: config/logger/memscan/dll_dir, KHÔNG phụ thuộc eldenring-rs
└── crates/          # chỉ chứa các mod chính, mỗi thư mục build ra 1 DLL
    ├── autoregen/    # mod AutoRegen: hồi HP/FP/Stamina theo tick + khi đánh trúng/bị đánh trúng
    ├── sometweaks/    # mod SomeTweaks (đổi tên từ LifeBetween): gộp nhiều QoL, hiện có Regen
    ├── passiverunes/  # mod PassiveRunes: cộng rune theo thời gian + bonus mốc thời gian
    ├── runemultiplier/ # mod RuneMultiplier: nhân hệ số rune từ mọi nguồn qua 1 hook AddSoul_Call
    ├── weightmultiplier/ # mod WeightMultiplier (đổi tên từ ReductionWeight): nhân hệ số Trọng Tải qua 1 hook code
    └── risearcher/    # mod RiseArcher: buff Bow/Crossbow/Ballista/Arrow/Bolt qua SoloParamRepository sống
```

`shared/` nằm ngoài `crates/` có chủ đích: `crates/` chỉ chứa các mod thật
sự (mỗi thư mục = 1 DLL xuất bản), còn `shared/` là hạ tầng dùng chung,
không phải 1 mod. Tên package Cargo của nó vẫn là `common` (không đổi
tên package theo tên thư mục) — mỗi mod vẫn `use common::...` như cũ.

Mỗi mod crate (`crates/autoregen`, `crates/sometweaks`, ...) có:
- `Cargo.toml` riêng, `common = { path = "../../shared" }` + các dependency
  `eldenring`/`fromsoftware-shared`/`chrono` lấy version thống nhất từ
  `[workspace.dependencies]` ở root qua cú pháp `xxx.workspace = true`.
- 1 file `.ini` cùng tên crate, nhúng vào binary lúc build (`include_str!`) -
  đây là nguồn sự thật duy nhất cho cấu hình mặc định.
- `src/lib.rs` mỏng: chỉ có `DllMain` + gọi `common::config`/`common::logger`,
  logic thật nằm trong module riêng của mod (`src/regen/`, ...).
- `README.md` riêng (lịch sử dịch ngược/quyết định kiến trúc của mod đó) +
  `DESCRIPTION.bbcode` (mô tả đăng Nexus) khi mod đã publish - port cả 2 file
  này từ repo C++ gốc mỗi khi thêm mod mới vào workspace, không chỉ port code.

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

pwsh scripts/build-mod.ps1 -Mod AutoRegen   # build + copy AutoRegen.dll/.ini vào build/
```

Hoặc dùng task có sẵn trong VS Code (**Ctrl+Shift+B**) - mở danh sách chọn:
`Cargo: Build AutoRegen/SomeTweaks/PassiveRunes/RuneMultiplier/
WeightMultiplier/RiseArcher (Release)` build riêng 1 mod, `Cargo: Build All
(Release)` chạy hết. Mỗi task build xong đều tự copy
`.dll` + `.ini` của mod đó vào `build/` (thư mục phẳng, không lẫn vào hàng
trăm file khác trong `target/release/`) - `build/` gom `.dll`/`.ini` của mọi
mod chung 1 chỗ, dễ tìm và copy sang thư mục mod loader.

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
  nhau, xem `crates/risearcher/README.md`. Mod duy nhất không cần `CSTaskImp` lẫn
  hotkey - patch 1 lần lúc `SoloParamRepository` sẵn sàng rồi thôi.

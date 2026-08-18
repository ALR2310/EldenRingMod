# PassiveRunes

Mod cho Elden Ring: tự động cộng rune theo thời gian thực (mỗi
`IntervalSeconds` giây cộng `RunesPerInterval` rune), kèm bonus mốc thời
gian (`Milestones`).

## Bản Rust hiện tại (2026-08-18)

Đã **viết lại hoàn toàn bằng Rust** (`cdylib`), thay cho bản C++ ban đầu -
sống ở `crates/passiverunes` trong workspace
[`EldenRingMod`](../../README.md), không còn là repo Git riêng. Đây đúng
như README cũ (bên dưới) đã tự kết luận: mod **có lợi nhất** để chuyển
trong số các mod cũ, vì không có ASM hook hay logic patch code nào cả.

- **Rune count**: đọc/ghi thẳng
  `GameDataMan::main_player_game_data.rune_count` qua
  [`fromsoftware-rs`](https://github.com/vswarte/fromsoftware-rs) (crate
  `eldenring`) - không còn tự AOB-scan + giải mã tay pointer chain
  (`GAMEDATAMAN_PATTERN` + `OFFSET_1`/`OFFSET_2`, xem lịch sử dịch ngược
  bên dưới) như bản C++ nữa.
- **Tick loop**: chạy như 1 task đăng ký trên `CSTaskGroupIndex::FrameBegin`
  của chính game, thay vì 1 thread `Sleep` riêng.
- **Sửa 1 lỗi khi port milestone**: bản C++ so mốc thời gian bằng
  `elapsed == milestone.atSeconds` (khớp tuyệt đối) - nếu tick bị lệch nhịp
  (game giật frame, hoặc `IntervalSeconds` không chia hết mốc) thì bỏ lỡ
  vĩnh viễn bonus đó. Bản Rust dùng threshold-crossing
  (`session_elapsed >= milestone.at_seconds`), không thể bỏ sót mốc nào.
- Giữ nguyên toàn bộ ini key gốc (`IntervalSeconds`, `RunesPerInterval`,
  `EnableMilestones`, `EnableLog`, `Milestones`) - không đổi tên, người
  dùng cũ không cần sửa ini. Đọc/ghi ini/log giờ dùng chung crate
  [`common`](../../shared) thay vì code riêng.

Xem `src/rune.rs` cho code hiện tại; `PassiveRunes.ini` cho toàn bộ key
cấu hình.

## Lịch sử dịch ngược (bản C++ gốc, không còn khớp code hiện tại)

Đã build và hoạt động (`bin/x64/Release/PassiveRunes.dll`) trước khi
chuyển sang Rust.

### Cơ chế bản C++

Thread riêng (`RuneEngine::Run`, tạo từ `DllMain`) `Sleep` theo
`IntervalSeconds` rồi cộng thẳng vào con trỏ rune. Con trỏ đó lấy được qua
1 pointer chain **tự dò và giải mã tay** từ 1 DLL cheat khác
(`RunesClock25.dll`, xem `reverse_engineering/RunesClock25_decompiled.txt`):
AOB pattern trỏ tới singleton `GameDataMan`, cộng `+0x8` (dereference 1
lần), rồi `+0x6C` (không dereference) ra địa chỉ rune. Xem chi tiết trong
comment đầu `src/RuneEngine.cpp` (repo C++ gốc).

### [ĐÃ LÀM, xem mục đầu README] Ghi chú cũ: nên chuyển sang Rust

Mục này giữ lại làm lịch sử quyết định - toàn bộ nội dung bên dưới **đã
được thực hiện**, xem "Bản Rust hiện tại" ở đầu file.

Đây là mod **có lợi nhất** để chuyển kiến trúc trong số các project hiện
tại, vì nó chỉ đọc/viết đúng 1 field đã biết (`rune_count`), không có ASM
hook hay logic patch code nào cả - đúng loại việc crate `eldenring`
(github.com/vswarte/fromsoftware-rs) giải quyết sẵn:

- Field `PlayerGameData.rune_count` đã được crate maintain offset (thấy
  dùng trực tiếp trong mod cộng đồng `ActionBasedRunes` —
  `player_game_data.rune_count = player_game_data.rune_count.saturating_add(...)`).
  Không cần tự dò/giải mã `GAMEDATAMAN_PATTERN` + `OFFSET_1`/`OFFSET_2` bằng
  tay như trước - đây chính là việc `AutoRegen/README.md` đã than phiền
  nhiều lần (offset lệch sau mỗi bản game update, phải dò lại từ đầu bằng
  Cheat Engine).
- Vòng lặp `Sleep`-polling thay được bằng `CSTaskImp::run_recurring`
  của crate (`ActionBasedRunes` dùng đúng API này) - chạy đồng bộ theo tick
  thật của game, không cần tự quản lý thread riêng.
- Không có ASM/AOB hook nào trong toàn bộ mod này (khác `SpiritSummonMultiplier`
  cần patch code, hay `AutoRegen`/`AttackHook`) - nên **không có phần nào
  bị mất đi** khi đổi ngôn ngữ, thuần lợi.

# PassiveRunes

Mod cho Elden Ring: tự động cộng rune theo thời gian thực (mỗi
`Rune.Passive.Interval` mili giây cộng `Rune.Passive.Amount` rune), kèm
bonus mốc thời gian (`Rune.Milestone`).

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

## Thêm polling ReloadKey - hot reload chưa thực sự chạy (2026-08-24)

Test thật trong game: nhấn F5 không thấy ini được đọc lại. Nguyên nhân:
lúc đổi sang format ini mới (mục dưới) có thêm key `[General] ReloadKey=F5`
theo đúng quy ước, nhưng `rune.rs` **chưa từng đọc phím này hay gọi lại
`config::load()`** - chỉ copy đúng cái ini, quên phần code tương ứng.

Khác với `SomeTweaks`, nơi module `regen` trong cùng 1 DLL đã tự poll
`ReloadKey` mỗi tick và gọi `config::load()` cho cả process (nên module
`rune` của `SomeTweaks` "ăn theo" được mà không cần tự poll - xem comment
cũ trong `sometweaks/src/lib.rs`), `PassiveRunes` là 1 DLL độc lập, không
có module nào khác trong cùng process lo việc này giúp nó. Sửa bằng cách
thêm đúng đoạn poll `input::is_key_pressed(reload_key)` +
`config::load(&ini_path)` vào đầu tick, y hệt cách `AutoRegen`/`SomeTweaks`
làm trong `regen.rs`/`regen/mod.rs` - `rune::run()` giờ nhận thêm tham số
`ini_path` để có đường dẫn gọi lại.

## Đổi sang format ini chung của dự án (2026-08-24)

Ini cũ (`[PassiveRunes]` với `IntervalSeconds`/`RunesPerInterval`/
`EnableMilestones`/`EnableLog`/`Milestones`) đã lỗi thời - đổi sang đúng
format `Rune.Passive.*`/`Rune.Milestone` mà `SomeTweaks`' Rune Reward module
đã dùng, để 2 module song song này không lệch quy ước:

- `[General] ReloadKey=F5` - phím hotkey đọc lại ini khi đang chạy game
  (như `AutoRegen`/`SomeTweaks`), chỉ ảnh hưởng các giá trị đọc lại mỗi tick,
  không restart thread. **Sửa lại (xem mục "Thêm polling ReloadKey" bên
  dưới)**: lúc thêm key này vào ini, code chưa thực sự đọc phím - đã bổ
  sung ngay sau đó.
- `[Rune Reward] Rune.Passive.Enabled/Interval/Amount` thay
  `IntervalSeconds`/`RunesPerInterval` cũ - **đổi đơn vị `Interval` từ giây
  sang mili giây** để khớp `Regen.PerTick.Interval`. `EnableMilestones` gộp
  vào `Rune.Passive.Enabled` (tắt cả interval lẫn milestone cùng lúc); muốn
  tắt riêng milestone thì để `Rune.Milestone=` rỗng.
- `[Debug] RuneLog=false` thay `EnableLog` cũ - file log giờ luôn được tạo
  (`logger::init` không còn điều kiện), `RuneLog` chỉ gate từng dòng log cụ
  thể, giống cách `AutoRegen`/`SomeTweaks` dùng `RegenLog`/`RuneLog`.
- Toàn bộ comment trong `PassiveRunes.ini` viết bằng tiếng Anh, khớp quy
  ước của `AutoRegen.ini` (khác `SomeTweaks.ini` đang lẫn tiếng Việt).

Vẫn giữ nguyên 2 bug fix ở mục dưới (`wait_for_cs_task` retry, gate
`WorldChrMan.main_player`) khi port qua format key mới.

## Sửa 2 bug: activation race + cộng rune ở màn hình chờ (2026-08-24)

Phát hiện khi test thật với ModEngine2: log game (`modengine_2026-08-24.log`
+ `PassiveRunes.log`) cho thấy 2 vấn đề độc lập, ban đầu tưởng nhầm là do
"để DLL khác ổ đĩa với game" (theo 1 bình luận Nexus) nhưng không phải -
đã lần theo tận source `fromsoftware-rs` để xác nhận.

- **Bug 1 - `CSTaskImp::wait_for_instance` không retry `InvalidRva`**: dù
  gọi với `Duration::MAX`, hàm này chỉ tự retry lỗi `Null` bên trong; lỗi
  `InvalidRva` (RVA lookup chạy trước khi exe game unpack/relocate xong)
  trả về **ngay lập tức, vĩnh viễn không thử lại** -> mod bị tắt hẳn cho cả
  session nếu thread của DLL khởi động quá sớm so với lúc game sẵn sàng.
  Đây là race về thời điểm, không liên quan gì tới ổ đĩa chứa DLL (hàm chỉ
  đọc module của chính game qua `GetModuleHandleA(NULL)`). Sửa bằng cách tự
  bọc thêm 1 vòng lặp retry mỗi 1s quanh `wait_for_instance` (hàm
  `wait_for_cs_task`) thay vì bỏ cuộc ngay lần đầu.
- **Bug 2 - cộng rune cả khi còn ở màn hình chờ**: `add_runes` trước đây
  chỉ check `GameDataMan::instance_mut()` - struct này đã có sẵn ngay khi
  save data được nạp vào bộ nhớ, **trước khi** người chơi thực sự vào lại
  world (còn đứng ở loading/title screen). Log thực tế cho thấy tick
  "+25 runes" chạy đều mỗi 5s dù chưa bấm vào game. Sửa bằng cách gate thêm
  `WorldChrMan.main_player` phải `Some` mới cộng rune - đúng pattern đã
  dùng ở `AutoRegen`/`SomeTweaks` cho heal-over-time. Kèm theo đó, vòng lặp
  milestone trước đây tăng `next_milestone` bất kể `add_runes` có thành
  công hay không, nên nếu vượt mốc thời gian trong lúc chưa vào game thì
  bonus mốc đó bị bỏ lỡ vĩnh viễn - sửa thành chỉ tăng khi cộng rune thành
  công, còn không thì dừng vòng lặp để tick sau thử lại đúng mốc đó.

## Pin fromsoftware-rs 0.14.0, thêm wait_for_system_init + panic safety (2026-08-26)

Áp dụng lại 3 cải tiến ổn định đã làm cho `sometweaks` (xem README của nó
cùng ngày, so sánh với `.docs/UltimatePassiveRegeneration`) sang crate này:

- **Version pin**: `eldenring`/`fromsoftware-shared` giờ khai báo 1 lần ở
  workspace `Cargo.toml` gốc, pin về bản crates.io `0.14.0` thay vì tracking
  git HEAD - áp dụng chung cho toàn bộ workspace, không cần sửa gì riêng ở
  crate này.
- **`wait_for_system_init`**: `wait_for_cs_task()` trong `src/rune.rs` giờ
  gọi `wait_for_system_init_until_ready()` (chờ `CSWindow` hInstance) trước
  vòng lặp retry `CSTaskImp::wait_for_instance` đã có sẵn (fix bug 1 ngày
  2026-08-24 phía trên) - 2 bước riêng biệt, không thể thay cho nhau.
- **Panic safety**: thêm `run_recurring_safe()` bọc quanh tick chính trong
  `rune.rs`, bắt panic mỗi frame thay vì crash cả game. Cần workspace
  `Cargo.toml` bỏ `panic = "abort"` ở `[profile.release]` (mặc định về
  `"unwind"`) thì `catch_unwind` mới có tác dụng ở bản release.

Không đụng ini/hành vi gameplay, chỉ cải thiện độ ổn định.

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

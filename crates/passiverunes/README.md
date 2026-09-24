# PassiveRunes

Mod cho Elden Ring: tự động cộng rune theo thời gian thực (mỗi
`Rune.Passive.Interval` mili giây cộng `Rune.Passive.Amount` rune cố định
+ `Rune.Passive.Percent`% chi phí lên cấp tiếp theo), kèm bonus mốc thời
gian (`Rune.Milestone`).

## Bản Rust hiện tại (2026-08-18)

Đã **viết lại hoàn toàn bằng Rust** (`cdylib`), thay cho bản C++ ban đầu -
sống ở `crates/passiverunes` trong workspace
[`EldenRingMod`](../../README.md), không còn là repo Git riêng. Đây đúng
như README cũ (bên dưới) đã tự kết luận: mod **có lợi nhất** để chuyển
trong số các mod cũ, vì không có ASM hook hay logic patch code nào cả.

- **Rune count**: đọc/ghi thẳng `PlayerGameData::rune_count` (ban đầu qua
  `GameDataMan::main_player_game_data`, **từ 2026-09-23 qua
  `WorldChrMan.main_player.player_game_data`** - xem mục "Bỏ hết phụ thuộc
  `rva::get()`" cuối README) qua
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
`ini_path` để có đường dẫn gọi lại. **Đã thay (2026-09-23)**: poll này đã
bỏ khỏi `rune.rs`, giao cho `common::reload::run()` chạy thread riêng (xem
mục "Bỏ hết phụ thuộc `rva::get()`" cuối README); `rune::run()` không còn
nhận `ini_path`.

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

## Đổi `[Debug]`/`RuneLog` thành `[Logging]`/`LogFile` (2026-09-14)

Đồng bộ tên section/key logging với các mod khác trong repo (`AutoRegen`,
`SomeTweaks`, `RuneMultiplier`, `RiseArcher`) - `[Logging]`/`LogFile` giờ là
quy ước chung cho toàn bộ mod trong workspace này, không riêng gì mod nào.
Không đổi hành vi: vẫn chỉ gate log chi tiết (milestone/tick bonus), file log
`PassiveRunes.log` tự nó luôn được tạo bất kể key này (xem `lib.rs`).
`config::migrate()` tự đẩy `RuneLog` cũ vào `[Legacy]` ở lần chạy đầu sau khi
cập nhật DLL.

## Thêm `Rune.Passive.Percent` - rune theo % chi phí lên cấp (2026-09-23)

Yêu cầu từ comment trên Nexus: số rune cố định (`Rune.Passive.Amount`) nhanh
chóng trở nên vô nghĩa khi lên cấp cao, người chơi phải sửa ini liên tục cho
khớp tiến độ. Thêm key `Rune.Passive.Percent` (số thực, `0` = tắt, mặc định
`0` nên người dùng cũ không đổi hành vi): mỗi interval cộng thêm
`floor(level_up_cost(level) * Percent / 100)` rune, tối thiểu 1 rune khi
`Percent > 0`.

- **Cộng dồn, không thay thế**: ban đầu định cho chọn 1 trong 2 (`Percent >
  0` thì bỏ qua `Amount`), cuối cùng chọn kết hợp - mỗi tick nhận `Amount +
  phần %`, mỗi phần tắt độc lập bằng `0`. Dùng chung `Rune.Passive.Interval`
  (1 đồng hồ duy nhất, dễ tính thời gian lên cấp: `Percent=1`,
  `Interval=10000` = 100 tick = ~16,7 phút/cấp chỉ tính rune thụ động).
- **Level**: đọc `GameDataMan::main_player_game_data.level` (field có sẵn
  cạnh `rune_count` trong `PlayerGameData` của fromsoftware-rs).
- **Chi phí lên cấp**: game không có param cho việc này - là công thức cứng
  trong exe, cộng đồng đã giải: `l = level + 81`, `x = max(0, (l - 92) *
  0.02)`, `cost = floor((x + 0.1) * l^2 + 1)` (cấp 1 -> 2 = 673, khớp màn
  hình lên cấp). **Đã xác minh trong game (2026-09-23)**: level 257 -> 258
  = 573.505 rune, khớp đúng con số màn hình lên cấp hiển thị. Xem
  `level_up_cost()` trong `src/rune.rs`.
- **Luôn tính theo cấp hiện tại**, không theo số rune đang giữ - gom rune
  không lên cấp thì vẫn nhận đều % của cấp kế tiếp mỗi tick.
- Milestone vẫn là số cố định, chưa hỗ trợ `%` (để sau nếu có người hỏi).
- Tiện sửa luôn: `add_runes` giờ kẹp tổng rune ở `999,999,999` (giới hạn
  thật của game) thay vì chỉ `saturating_add` tới `u32::MAX`.

## Bỏ hết phụ thuộc `rva::get()` - không còn khoá theo bản game, dùng `common::task`/`common::reload` (2026-09-23)

Rà lại so với `AutoRegen` (xem mục "AOB thay `rva::get()`..." trong
[README của nó](../autoregen/README.md)): PassiveRunes vẫn còn 2 lớp dính
bảng RVA theo từng phiên bản game của `fromsoftware-rs`
(`eldenring::rva::get()`) - chính là lý do mỗi lần game patch
(1.17, 1.17.1) phải ra bản 2.1/2.2 chỉ để "update for ER x.y":

1. **Đăng ký tick**: tự mang bản `wait_for_cs_task` cũ
   (`wait_for_system_init` - đọc `rva::get().global_hinstance` - rồi
   `CSTaskImp::wait_for_instance` retry `InvalidRva`) + `run_recurring_safe`
   gọi `SharedTaskImpExt::run_recurring` (nội bộ `rva::get().register_task`).
   Thay bằng `common::task::{wait_for_cs_task, run_recurring_safe}` - tra
   `CSTaskImp` theo tên singleton (`"CSTask"`) + đăng ký task qua AOB
   (`common::task_hook`), y như `AutoRegen`/`DropMultiplier`.
2. **Đọc/ghi rune + level**: `GameDataMan::instance()` - `FromStatic` của
   nó là `load_static_indirect(rva::get().game_data_man)`, không phải
   singleton theo tên. Thay bằng `WorldChrMan::instance_mut()` (singleton
   `"WorldChrMan"`) -> `main_player` -> `PlayerIns::player_game_data`
   (`NonNull<PlayerGameData>`, cùng struct `GameDataMan.main_player_game_data`
   trỏ tới), gói trong `with_player_game_data()` ở `src/rune.rs`. Gate "đã
   vào game" (`main_player.is_some()`) giờ có sẵn luôn trong đường đi này,
   không cần check riêng.

Nhân tiện, `ReloadKey` giờ do `common::reload::run()` lo trên thread riêng
(`lib.rs`, giống `DropMultiplier`) thay vì tự poll trong tick - được luôn
banner **"Config reloaded"** trong game (`common::announce`). Mọi key của
`rune.rs` đều đọc lại mỗi tick nên không cần `RELOAD_GENERATION`.

`DESCRIPTION.bbcode` cũng đổi sang đúng format của `AutoRegen`: câu mở đầu
"A DLL mod that...", bỏ khối Configuration (ini đã tự mô tả), mục cài đặt
chung cho mọi mod loader, changelog gói trong `[spoiler]`. Sửa luôn mô tả
lỗi thời "25 runes every 5 seconds" -> đúng mặc định hiện tại của ini (100
rune mỗi 10 giây).

**Chưa test trong game** tại thời điểm viết mục này.

## Cache kết quả AOB/`CSTaskImp` trong `common` - không quét lại mỗi task (2026-09-23)

Test thật bản ở mục trên: `PassiveRunes.log` có **2 dòng** `CSTaskImp
found.` và **2 dòng** `register_task AOB found.`. Không phải lỗi - mod
đăng ký 2 task (`common::reload` + `Rune`), mỗi thread tự gọi
`wait_for_cs_task()` + `task_hook::run_recurring()` - nhưng lộ ra việc
`common` **không cache gì cả**: mỗi lần đăng ký task là 1 lần quét lại
toàn bộ `.text` của exe để tìm `register_task`. Tệ hơn,
`alloc_hook::runtime_heap_allocator()` quét lại AOB **mỗi lần hiện banner**
(mỗi lần bấm F5) - `LOGGED_SUCCESS` cũ chỉ chặn log lặp, không chặn quét
lặp, nên log trông như đã cache.

Địa chỉ code trong exe không đổi suốt vòng đời process (ASLR chỉ chọn base
1 lần lúc khởi động), nên chỉ cần tìm 1 lần. Sửa trong `shared/src`:

- `memscan::CachedAddr` (mới): `Mutex<Option<usize>>` +
  `get_or_resolve(resolve)`. Dùng `Mutex` thay `OnceLock` để thread thứ 2
  **chờ** thread đầu quét xong rồi dùng chung kết quả, thay vì cả 2 quét
  song song lúc khởi động. **Chỉ cache khi thành công** - quét trượt (vd.
  đua với Arxan giải mã code lúc mới vào) để trống cache, lần gọi sau quét
  lại, không bị khoá cứng ở trạng thái lỗi.
- `task_hook`: cache địa chỉ `register_task`.
- `alloc_hook`: cache **địa chỉ biến global** chứa con trỏ allocator, **không
  cache giá trị** - code có sẵn ngay khi AOB khớp nhưng game có thể chưa
  khởi tạo allocator (instance còn `null`), nên giá trị vẫn đọc lại mỗi lần
  gọi (1 lần đọc bộ nhớ). Bỏ `LOGGED_SUCCESS`.
- `task::wait_for_cs_task`: cache `&'static CSTaskImp` (logic poll cũ tách
  ra `poll_cs_task`), `CSTaskImp found.` chỉ log 1 lần.
- `task::run_recurring_safe`: log `{tag}: task registered.` khi đăng ký
  thành công, để log nói rõ có mấy task (vd. `Reload: task registered.` +
  `Rune: task registered.`). Bỏ dòng `Rune tick registered...` riêng trong
  `rune.rs` cho khỏi trùng.

Áp dụng cho **mọi mod dùng `common`** (AutoRegen, DropMultiplier,
InfiniteAilment, SomeTweaks, RiseArcher, PassiveRunes) - hành vi không đổi,
chỉ bớt quét thừa + log gọn hơn. `cargo build --workspace --release` +
`cargo test --workspace` qua. **Chưa test trong game.**

## Log `LogFile` mỗi interval/milestone: thêm rune đang giữ + level (2026-09-23)

Theo yêu cầu người dùng khi test: dòng log interval (chỉ khi `[Logging]
LogFile=true`) giờ ghi đủ `+{amount} runes ({fixed} fixed + {percent} from
{Percent}% of {cost} next-level cost) -> held {held}, level {level}, session
{s}s` - tách phần cố định/phần %, chi phí lên cấp dùng để tính %, rune đang
giữ **sau** khi cộng, level hiện tại. Dòng milestone thêm `-> held, level`
tương tự. `add_runes()` đổi từ `bool` sang `Option<Granted { held, level }>`
để lấy 2 giá trị này ngay trong cùng 1 lần truy cập `PlayerGameData`.

## Đồng hồ session/interval chỉ chạy khi đã vào game (2026-09-23)

Log test thật cho thấy tick rune đầu tiên (16:20:20) đã ghi `session 90s`
dù task đăng ký lúc 16:18:26 - `session_elapsed_ms`/`interval_elapsed_ms`
bắt đầu đếm ngay từ lúc task đăng ký, nên thời gian ở màn hình tiêu đề/menu
và màn hình tải cũng tính vào milestone. `add_runes()` đã gate
`main_player` nên không cộng rune sai lúc đó, nhưng đồng hồ vẫn chạy.

Sửa: gọi `common::player::main_player_chr_ins_ptr().is_none()` (hàm chung
sẵn có, `DropMultiplier`/`SomeTweaks`/`AutoRegen` đều dùng - bản đầu tự
viết `in_game()` riêng, người dùng nhắc là đã có sẵn nên thay luôn) ở đầu
tick, sau check `Rune.Passive.Enabled` - chưa vào game thì return luôn, không cộng `dt_ms` vào đồng hồ nào. **Tạm
dừng, không reset** khi rời world (màn hình tải giữa chừng, thoát ra
title) - reset ở màn hình tải sẽ xoá tiến độ milestone mỗi lần dịch
chuyển. Hệ quả: thoát ra title rồi load save khác vẫn đếm tiếp từ số cũ
(như trước), chỉ không đếm phần thời gian ở ngoài.

## Thêm `Rune.Passive.Interest` - lãi kép theo rune đang giữ (2026-09-23)

Ý tưởng của người dùng: chế độ thứ 3 - rune theo % số rune đang giữ (lãi
kép). Ban đầu đề xuất dạng `Mode=1/2/3` (cố định / % cấp / % rune), bỏ vì
đi ngược quyết định "cộng dồn" của `Rune.Passive.Percent` ở trên - thay
bằng key thứ 3 cũng cộng dồn: mỗi tick nhận `Amount + phần %cấp + phần
lãi`, mỗi phần tắt độc lập bằng `0` (đặt 2 key còn lại = 0 là ra đúng
"mode" mong muốn). Mặc định `Interest=0` nên không đổi
hành vi bản cũ.

- **Tăng theo hàm mũ** (mô phỏng với `Interval=10000`, chỉ phần lãi):
  `Interest=1` gấp đôi mỗi ~70 tick (~11,6 phút) - từ 229 triệu rune chạm
  trần 999.999.999 trong ~25 phút, từ 10.000 rune ~3,2 giờ; `Interest=0.1`
  từ 1.000 rune chạm trần ~38 giờ. Vì vậy ini gợi ý `0.05`-`0.2`. Bản
  đầu có thêm `Rune.Passive.Interest.Max` (trần rune lãi mỗi tick) - **đã
  bỏ ngay trong ngày** theo quyết định người dùng: không cần giới hạn, để
  lãi kép chạy tự do, người dùng tự chọn `Interest` nhỏ nếu muốn chậm.
- **Cộng dồn phần lẻ** (`InterestState::carry`): nếu làm tròn xuống mỗi
  tick thì `0.1%` của 1.000-1.999 rune luôn chỉ ra 1 rune (1.000 tick mới
  gấp đôi thay vì ~694), dưới 1.000 rune thì lãi = 0 vĩnh viễn - không
  phải lãi kép thật. Phần lẻ giữ lại sang tick sau, **reset** khi số rune
  đang giữ thấp hơn sau lần cộng trước (`last_held` - đã tiêu hoặc chết mất
  rune), hoặc khi tắt `Interest`.
- **Không có tối thiểu** (khác `Percent`, vốn tối thiểu 1 rune): giữ 0 rune
  thì lãi 0 - chết mất rune là mất luôn "tiền gốc", rủi ro/thưởng tự nhiên.
  Tính trên rune đang giữ, không tính rune rơi dưới đất.
- Log `LogFile` thêm `+ {interest} from {Interest}% interest on {principal}`
  (principal = rune đang giữ **trước** khi cộng tick này).

`DESCRIPTION.bbcode`: thêm 1 dòng Features + 1 dòng changelog 2.3. **Chưa
test trong game.**

## Đổi mặc định: `Amount` 100 -> 50, `Percent` 0 -> 0.25 (2026-09-23)

Theo quyết định người dùng: mặc định mới mỗi 10s = 50 rune cố định +
0,25% chi phí lên cấp tiếp theo (~400 tick = ~67 phút/cấp chỉ tính phần
%). Đổi ở cả `PassiveRunes.ini` lẫn giá trị fallback trong
`config::get_int/get_double` của `rune.rs`.

Lưu ý với người dùng cũ: `config::load_or_create_default` chỉ **thêm** key
còn thiếu, không ghi đè key đã có - ini cũ giữ nguyên `Amount=100`, nhưng
`Rune.Passive.Percent` là key mới nên sẽ được thêm với giá trị `0.25`, tức
bản 2.3 **bật sẵn** phần % cho cả người dùng cũ (cộng dồn với `Amount` cũ
của họ). Chấp nhận - đây là tính năng chính của bản 2.3.

## Cho phép giá trị âm - trừ rune theo thời gian (2026-09-23)

Theo yêu cầu người dùng: `Amount=-50` thì mỗi interval **trừ** 50 rune,
tương tự cho `Percent`, `Interest` và bonus trong `Rune.Milestone`. Trước
đó mọi giá trị âm đều bị kẹp về 0 (tắt) - `.max(0)` ở `Amount`/`Interval`,
`if percent <= 0.0` ở `Percent`/`Interest`, bonus milestone parse bằng
`u32` nên số âm bị bỏ qua.

- Toàn bộ phép tính đổi sang `i64`; `add_runes(u32)` -> `apply_runes(i64)`,
  kết quả kẹp `0..=999.999.999` - **không bao giờ xuống dưới 0 rune**.
- `Percent` âm: `-(floor(cost * |p| / 100))`, vẫn tối thiểu 1 rune (lần này
  là -1) khi `p != 0`, đối xứng với chiều dương.
- `Interest` âm = suy giảm kép (mất `|p|%` số rune đang giữ mỗi tick). Phần
  lẻ đổi từ `floor` sang `trunc` (làm tròn về phía 0) để đúng cho cả 2
  chiều; `carry` reset thêm khi đổi dấu (hot reload từ `+x` sang `-x`), để
  phần lẻ dương còn dư không bù trừ mấy tick âm đầu. Việc chính `Interest`
  âm làm giảm rune **không** làm reset `carry` (`last_held` ghi sau lần cộng
  đó).
- Các phần vẫn cộng dồn với nhau, vd. `Amount=-50` + `Percent=0.25` = trừ
  50 nhưng cộng 0,25% chi phí lên cấp.
- `Interval` vẫn `.max(0)` - interval âm vô nghĩa.
- Nhân tiện bỏ qua milestone có **số giây âm** (`-10:5000`) - trước đó được
  parse và thưởng ngay khi vào game.
- Log dùng `{:+}` (`+5835 runes (+100 fixed, ...)` / `-50 runes (-50 fixed,
  ...)`), dòng milestone cũng vậy.
- Ini thêm 1 dòng ghi chú chung ngay trên `Amount`: các giá trị bên dưới có
  thể âm để trừ rune, không bao giờ dưới 0.

**Chưa test trong game.**

## Thêm `Range`/`Default` vào ini + kẹp giá trị theo đúng khoảng đó (2026-09-23)

Người dùng thêm mẫu `; Range:` / `; Default:` cho `Amount`/`Percent`/
`Interest` (và bỏ dòng ghi chú chung "can be negative..." ở mục trên - giờ
`Range` tự ghi `(negative takes runes away)`). Điền:

- `Amount`: `-999999999 to 999999999` - vượt quá thì cũng vô nghĩa vì số
  rune đang giữ luôn kẹp trong `0..=999.999.999`.
- `Percent`: `-100 to 100` (100 = đủ 1 cấp mỗi tick).
- `Interest`: `-100 to 100` (-100 = mất hết rune đang giữ trong 1 tick,
  100 = gấp đôi mỗi tick). Sửa `Default` từ `0.25` (người dùng gõ nhầm)
  thành `0` - đúng giá trị mặc định thật.

Trước đó code **không kẹp** `Percent`/`Interest` (đặt `500` vẫn chạy) - thêm
`.clamp(...)` ở chỗ đọc config trong `rune.rs` để khoảng ghi trong ini khớp
hành vi thật.

## `LogFile=false` giờ không tạo file log nữa - sửa trong `common::logger` (2026-09-23)

Người dùng phát hiện (qua PassiveRunes): `LogFile=false` nhưng file `.log`
vẫn được tạo. Đúng là trước đó `logger::init` được gọi vô điều kiện trong
`lib.rs` - file luôn tạo, `LogFile` chỉ gate log chi tiết - trái với tên
key và với cách RiseArcher/RuneMultiplier vốn làm. Người dùng xác nhận hành
vi đúng: `LogFile=true` mới tạo file; muốn có log sẵn thì deploy với mặc
định `LogFile=true` (mod này mặc định `false`).

Sửa tập trung trong `shared/src/logger.rs`: `init` chỉ ghi nhớ đường dẫn,
file được tạo (truncate) ở dòng log đầu tiên khi `LogFile=true`, `LogFile`
đọc lại mỗi lần ghi - bật/tắt bằng `ReloadKey` có hiệu lực ngay. Comment
"log is always on" trong `lib.rs` đã bỏ; mô tả key trong ini đổi thành
"Write PassiveRunes.log next to the DLL (for troubleshooting). Off = no log
file". Các mục cũ hơn trong README nói "file log luôn được tạo bất kể
`LogFile`" giờ đã lỗi thời.


## Đường dẫn DLL có ký tự không phải ASCII làm mod bỏ qua ini - sửa `common::dll_dir` (2026-09-24)

Bug phát hiện qua AutoRegen (người dùng ME3, thư mục profile tên tiếng
Trung): `common::dll_dir()` dùng `GetModuleFileNameA` (code page ANSI) rồi
giải mã như UTF-8, nên đường dẫn có ký tự không phải ASCII (tiếng Trung,
tiếng Việt có dấu...) bị hỏng, không tìm thấy ini/log cạnh DLL, và mod âm
thầm chạy với cấu hình mặc định. Đã đổi sang `GetModuleFileNameW` +
`from_utf16_lossy`, buffer tự tăng cho đường dẫn dài. Mod này dùng chung
`dll_dir` nên cũng được sửa. Chi tiết xem mục cùng ngày trong
`crates/autoregen/README.md`.

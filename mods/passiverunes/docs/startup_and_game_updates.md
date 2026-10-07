# PassiveRunes: khởi động, cổng "đã vào game" và chuyện bản vá game

**Status 2026-09-23: ĐÃ SỬA, mod không còn phụ thuộc bảng RVA theo phiên bản
game.** Chưa ghi nhận test in-game riêng cho phần bỏ `rva::get()` trong tài liệu
cũ. Code: `mods/passiverunes/src/rune.rs`, `src/lib.rs`.

## Bỏ phụ thuộc `rva::get()` (2026-09-23)

So với `AutoRegen`, PassiveRunes vẫn có hai lớp dính bảng RVA theo từng phiên bản
game của fromsoftware-rs (`eldenring::rva::get()`). Đó chính là lý do mỗi lần game
patch (1.17, 1.17.1) phải ra bản mới chỉ để "update for ER x.y" (changelog 2.0.1 và
2.0.2):

1. **Đăng ký tick:** mod mang bản `wait_for_cs_task` cũ (`wait_for_system_init` đọc
   `rva::get().global_hinstance`, rồi `CSTaskImp::wait_for_instance` retry
   `InvalidRva`) và `run_recurring_safe` gọi `SharedTaskImpExt::run_recurring` (bên trong
   là `rva::get().register_task`). Thay bằng `common::task::{wait_for_cs_task,
   run_recurring_safe}`: tra `CSTaskImp` theo tên singleton (`"CSTask"`) và đăng ký
   task qua AOB (`common::task_hook`), như `AutoRegen`/`DropMultiplier`.
2. **Đọc/ghi rune và level:** `GameDataMan::instance()` có `FromStatic` là
   `load_static_indirect(rva::get().game_data_man)`, không phải singleton theo tên. Thay
   bằng `WorldChrMan::instance_mut()` (singleton `"WorldChrMan"`) →
   `main_player` → `PlayerIns::player_game_data` (`NonNull<PlayerGameData>`, cùng struct
   mà `GameDataMan.main_player_game_data` trỏ tới), gói trong `with_player_game_data()`.
   Cổng "đã vào game" (`main_player.is_some()`) có sẵn luôn trong đường đi này.

`ReloadKey` cũng chuyển sang `common::reload::run()` chạy thread riêng, được luôn banner
"Config reloaded" (`common::announce`). Mọi key của `rune.rs` đều đọc lại mỗi tick nên
không cần `RELOAD_GENERATION`.

## Cache kết quả AOB / `CSTaskImp` trong `common` (2026-09-23)

Log test thật có hai dòng `CSTaskImp found.` và hai dòng `register_task AOB found.`.
Không phải lỗi (mod đăng ký hai task: `common::reload` và `Rune`, mỗi thread tự gọi
`wait_for_cs_task()` và `task_hook::run_recurring()`), nhưng lộ ra việc `common`
**không cache gì**: mỗi lần đăng ký task là một lần quét lại toàn bộ `.text` để tìm
`register_task`, và `alloc_hook` quét lại AOB mỗi lần hiện banner (mỗi lần bấm F5).
Sửa trong `shared/src` (áp dụng cho mọi mod dùng `common`): `memscan::CachedAddr`
(`Mutex<Option<usize>>`, chỉ cache khi thành công), cache địa chỉ `register_task` ở
`task_hook`, cache **địa chỉ biến global** của allocator ở `alloc_hook` (không cache giá
trị vì game có thể chưa khởi tạo), cache `&'static CSTaskImp` ở `task`. `run_recurring_safe`
log `{tag}: task registered.` để biết có mấy task.

## Hai lỗi khởi động ban đầu (2026-08-24)

Phát hiện khi test thật với ModEngine2. Ban đầu tưởng do "để DLL khác ổ đĩa với game"
(theo một bình luận Nexus), nhưng không phải; đã lần theo source fromsoftware-rs để
xác nhận.

- **`CSTaskImp::wait_for_instance` không retry `InvalidRva`.** Dù gọi với `Duration::MAX`,
  hàm chỉ tự retry lỗi `Null`; `InvalidRva` (RVA lookup chạy trước khi exe game unpack và
  relocate xong) trả về **ngay lập tức, không bao giờ thử lại**, nên mod tắt hẳn cho
  cả phiên nếu thread của DLL khởi động quá sớm. Đây là race về thời điểm, không liên quan
  ổ đĩa chứa DLL (hàm chỉ đọc module của chính game qua `GetModuleHandleA(NULL)`). Sửa
  bằng vòng retry mỗi 1 giây quanh `wait_for_instance`; sau đó thêm
  `wait_for_system_init` (chờ `CSWindow` hInstance) trước vòng đó (2026-08-26). Hai bước
  riêng biệt, không thay được cho nhau.
- **Cộng rune cả khi còn ở màn hình chờ.** `add_runes` chỉ kiểm tra
  `GameDataMan::instance_mut()`, mà struct này có sẵn ngay khi save data được nạp, **trước**
  khi người chơi thật sự vào world; log thực tế cho thấy tick "+25 runes" chạy đều mỗi 5
  giây dù chưa bấm vào game. Sửa bằng cách gate thêm `WorldChrMan.main_player` phải `Some`
  (cùng pattern với `AutoRegen`/`SomeTweaks` cho heal-over-time). Kèm theo đó, vòng lặp
  milestone chỉ tăng `next_milestone` khi cộng thành công (xem `rune_math.md`).

## `ReloadKey` chưa từng chạy (2026-08-24)

Test thật: nhấn F5 không thấy ini được đọc lại. Khi đổi sang format ini mới có thêm key
`[General] ReloadKey=F5` nhưng `rune.rs` **chưa từng đọc phím này hay gọi
`config::load()`**: chỉ copy đúng file ini, quên phần code. Khác `SomeTweaks` (module
`regen` cùng DLL tự poll `ReloadKey` và nạp config cho cả process, nên module `rune` của
nó "ăn theo"), PassiveRunes là DLL độc lập nên không có module nào lo giúp. Sau đó phần
poll này được thay bằng `common::reload::run()` (xem trên).

## An toàn khi panic (2026-08-26)

Tick chính được bọc `run_recurring_safe` (bắt panic mỗi frame thay vì crash cả game; cần
workspace bỏ `panic = "abort"` ở `[profile.release]` thì `catch_unwind` mới có tác dụng ở
bản release). Cùng đợt pin `eldenring`/`fromsoftware-shared` về bản crates.io `0.14.0` ở
workspace thay vì theo git HEAD.

## Lịch sử dịch ngược (bản C++ gốc, không còn khớp code)

Bản C++ chạy một thread `Sleep` rồi cộng thẳng vào con trỏ rune. Con trỏ lấy qua một
pointer chain tự dò và giải mã tay từ một DLL cheat khác (`RunesClock25.dll`): AOB trỏ
tới singleton `GameDataMan`, cộng `+0x8` (dereference một lần), rồi `+0x6C` (không
dereference) ra địa chỉ rune. Offset `0x6C` này khớp độc lập với `[RCX+0x6C]` trong
`AddSoul_Call` của `runemultiplier` và với field `rune_count` của fromsoftware-rs. Đây là
mod **có lợi nhất** để chuyển sang fromsoftware-rs vì chỉ đọc/ghi đúng một field đã biết,
không có ASM hay patch code nên không mất gì khi đổi ngôn ngữ; việc đó đã làm ở bản 2.0.0.

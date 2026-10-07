# RiseArcher: ba sự cố khi khởi động và đọc ini

**Status 2026-09-17: CẢ BA ĐÃ SỬA.** Hai lỗi đầu phát hiện ngày 2026-09-08 khi
test in-game lần đầu bản DLL; lỗi thứ ba ngày 2026-09-17. Code: `src/lib.rs`,
`src/weapon.rs`, `src/bullet.rs`, `RiseArcher.ini`.

## 1. Crash lặng lúc khởi động: `SoloParamRepository` sẵn sàng quá sớm

**Triệu chứng:** mod hoàn toàn không hoạt động (Black Bow không mở khoá được Ash
of War, số mũi tên vẫn 99). `RiseArcher.log` dừng ngay sau dòng
`SoloParamRepository ready, applying weapon buffs...`, không có dòng `Applied
to...` lẫn dòng lỗi nào. Đây là dấu hiệu đặc trưng của một thread Rust panic mà
không ai bắt: dưới `panic = "unwind"` của workspace, panic ở thread tạo bằng
`std::thread::spawn` tự chết im lặng, không crash game và không log gì.

**Nguyên nhân** (đọc thẳng source fromsoftware-rs):
`SoloParamRepository::instance_mut()` trả `Ok` ngay khi object tồn tại, nhưng
**sớm hơn** lúc file resource của từng param (`EquipParamWeapon`, `Bullet`...)
load xong. Gọi `rows_mut::<EquipParamWeapon>()` lúc đó rơi vào
`get_param_file_mut`'s `.expect("Expected param holder to have exactly one res
cap")` và panic, thay vì trả `Err` để retry. Giả định cũ trong code ("param chỉ
load một lần lúc khởi động") đúng về việc chỉ load một lần nhưng sai về thời điểm.

**Cách sửa:**
- Dùng `common::player::wait_for_solo_param_repository`, hàm này chờ thêm điều
  kiện `WorldChrMan::main_player` đã tồn tại (tức là người chơi đã vào world, lúc
  đó param đã load xong). Cùng bài học đã gặp ở `sometweaks::drop_rate`
  (2026-08-24/25): tính năng mở khoá Ash of War của `sometweaks` chạy được chính
  nhờ gate này.
- Lớp phòng hờ thứ hai, `apply_with_retry` trong `lib.rs`: bọc lần `apply` đầu bằng
  `catch_unwind`, tự thử lại tối đa 30 giây nếu vẫn panic. An toàn để thử lại vì
  panic luôn xảy ra trước khi dòng nào bị sửa.
- Các `Mutex` giữ giá trị gốc (`ORIGINALS`) phục hồi mutex bị "poison" bằng
  `unwrap_or_else(into_inner)`, để lần retry sau không panic dây chuyền ngay ở bước
  lock.

## 2. `RiseArcher.ini` không hề có tác dụng

**Triệu chứng:** chỉnh ini không đổi gì; mọi giá trị luôn chạy bằng default
hardcode trong Rust. Chỉ `ReloadKey` và `LogFile` hoạt động.

**Nguyên nhân:** `weapon.rs`/`bullet.rs` đọc key **có tiền tố** (`Bow.DamageMultiplier`,
`Bullet.SpeedMultiplier`...) nhưng ini ghi key **trần** (`DamageMultiplier`,
`MaxQuantity`...) từ commit đầu của bản DLL (2026-08-18). `common::config` là map
phẳng, bỏ qua section header hoàn toàn, nên không key nào từng khớp. Bug tồn tại từ
đầu mà không ai thấy vì chưa ai kiểm chứng việc chỉnh ini có ăn không.

**Cách sửa:** viết lại ini với đúng key có tiền tố khớp code. Người dùng nâng cấp
từ bản cũ: `config::migrate` thêm mọi key tiền tố mới (giá trị mặc định) và dời key
trần cũ vào `[Legacy]`, nên không mất dữ liệu, nhưng giá trị tuỳ chỉnh cũ (vốn
không có tác dụng) cần chỉnh lại theo tên key mới.

**Bài học:** tên key trong ini và trong code phải khớp tuyệt đối, và `[Section]`
chỉ là nhãn hiển thị. Cùng đợt, section được tổ chức lại theo **loại tuỳ chỉnh**
(chung, hệ số sát thương, phần dùng chung, đạn, log) thay vì theo loại vũ khí, và
bộ key được viết chi tiết hơn: tách trọng lượng/giá bán theo từng loại thay vì một
cặp dùng chung, `Bow.AllowAshOfWar` thành `Bow.UnlockAOW`, `MaxQuantity` của đạn
thành `Bullet.Arrow.MaxQuantity`/`Bullet.Bolt.MaxQuantity` (field thật vẫn là
`max_arrow_quantity` trên `EquipParamWeapon`, do `weapon.rs` áp).

## 3. Bỏ cuộc vĩnh viễn nếu chưa vào world trong 5 phút (2026-09-17)

`run()` gọi `wait_for_solo_param_repository` với timeout 300 giây; hết giờ mà
player chưa vào world thì `return`, **bỏ luôn cả phần đăng ký task theo dõi
`ReloadKey`**. Mod tắt vĩnh viễn cho phiên đó, kể cả bấm F5 (khác `drop_rate` và
`grace_menu::unlock_shop` của `sometweaks`, hai cái đó vẫn đăng ký reload dù lần
chờ đầu thất bại).

Sửa tập trung ở `common::player::wait_for_solo_param_repository`: bỏ hẳn tham số
`timeout`, chờ vô hạn (như `common::task::wait_for_cs_task`), nên `run()` gọi thẳng
không còn nhánh `else { return }`.

# DropMultiplier

Mod cho Elden Ring: chỉnh tỉ lệ rớt đồ của quái (`ItemLotParam_enemy`), theo
2 cách loại trừ nhau qua `Mode` - `0` = nhân hệ số lên tỉ lệ mỗi item, `1` =
ép tỉ lệ tổng ("có rớt gì đó") của mỗi dòng về đúng 1 con số cố định, giữ
nguyên tỉ lệ tương đối giữa các item cùng dòng.

## Tách ra từ `sometweaks::drop_rate` (2026-09-14)

Port nguyên bản tính năng `Drop Rate` từ [`sometweaks`](../sometweaks)
(`src/drop_rate/mod.rs`) thành 1 mod độc lập - cùng thuật toán/công thức
hoàn toàn, không đổi hành vi gameplay. `sometweaks` vẫn giữ nguyên tính
năng này song song (không xóa) - ai đang dùng `sometweaks` không cần đổi gì
cả, đây chỉ là thêm 1 lựa chọn dùng riêng lẻ.

Khác biệt so với bản gốc trong `sometweaks`:

- **Bỏ tiền tố `DropRate.`** trong ini: `DropRate.Enabled`/`Mode`/
  `Multiplier`/`ChancePercent` → `Enabled`/`Mode`/`Multiplier`/
  `ChancePercent` - không cần tiền tố vì cả file ini của mod này chỉ nói về
  đúng 1 tính năng (khớp quy ước `weightmultiplier`/`runemultiplier`: mod
  đơn tính năng không cần namespace key).
- **Dùng chung [`shared`](../../shared) (`common`)** thay vì tự có bản
  `player.rs`/`task.rs`/`reload.rs` riêng - xem README của `autoregen`, mục
  "Tách `task_hook.rs`.../Gộp crate `engine` ngược vào `shared`" (cùng ngày)
  để biết đầy đủ câu chuyện: các module này (`task_hook`/`alloc_hook`/
  `task`/`player`/`reload`) từng là 1 crate riêng tên `engine` trong vài
  giờ, rồi gộp ngược vào `shared`/`common` cùng ngày - `DropMultiplier` chỉ
  còn biết tới `common::*`, chưa từng thấy dạng `engine` độc lập. Tách ra
  ban đầu vì lúc port mod này nhận ra bộ này sắp bị copy lần thứ 2 (sau
  `sometweaks`/`risearcher`, cả 2 đều tự có bản `task.rs`/`player.rs`/
  `reload.rs` y hệt nhau). `DropMultiplier` là mod **đầu tiên trong
  workspace này khởi tạo mà dùng thẳng bộ này ngay từ đầu**, không phải
  migrate từ code riêng như `autoregen` đã làm.
- Nhờ dùng `common::task`, mod này **không mang theo rủi ro `rva::get()`
  version-lock** mà bản gốc trong `sometweaks` vẫn còn (xem README
  `autoregen`, mục "AOB thay `rva::get()`...") - `common::task::wait_for_cs_task`
  đã dùng cách tra `CSTaskImp::instance()` theo tên (không qua RVA) ngay từ
  đầu.

Không đổi: toàn bộ thuật toán `scale_row`/công thức `ChancePercent`, cách
snapshot `ORIGINAL_BASE_POINTS` để hot-reload không cộng dồn, cách gate
`SoloParamRepository`/`WorldChrMan.main_player` trước khi đọc param - xem
comment đầu `src/drop_rate.rs` cho chi tiết đầy đủ (giữ nguyên từ bản gốc).

## Bỏ prefix "DropMultiplier:" trong log, bỏ timeout 5 phút chờ vào world (2026-09-17)

Test thật trong game phát hiện 2 việc:

- **Log dư thừa**: mọi dòng log đều tự thêm `"DropMultiplier: "` ở đầu - vô
  nghĩa với 1 mod đơn tính năng (log file của chính nó đã là
  `DropMultiplier.log` rồi, không như `sometweaks` gộp nhiều tính năng
  chung 1 log cần tiền tố để phân biệt). Bỏ hết tiền tố này.
- **Race condition thật, tái hiện được**: `wait_for_solo_param_repository`
  (khi đó còn nhận `timeout`) hard-code chờ tối đa 300s (5 phút) rồi bỏ
  cuộc, log `ERROR ... disabled for this session` - nếu người chơi mở game
  xong bận việc khác >5 phút mới thật sự vào world, tính năng chỉnh tỉ lệ
  rớt đồ **không tự áp dụng**, chỉ phục hồi được nếu người dùng tự bấm
  `ReloadKey` (và biết là cần bấm). Sửa tại **hàm dùng chung**
  `common::player::wait_for_solo_param_repository` (không riêng gì
  `DropMultiplier` - `sometweaks`/`risearcher` cũng dính đúng lỗi này, xem
  README của chúng cùng ngày): bỏ hẳn tham số `timeout`, chờ **vô hạn**
  (giống `common::task::wait_for_cs_task` không bao giờ bỏ cuộc), chỉ log
  nhắc nhở mỗi 30s (không phải warning - chờ vài phút để chọn save/load vào
  world là bình thường, khác với `CSTaskImp` chậm bất thường).

## Banner "Config reloaded" trong game khi bấm ReloadKey (2026-09-17)

Hỏi được xác nhận: `AutoRegen` có banner cuộn chữ trên đầu màn hình lúc
reload, `DropMultiplier` thì không. Hàm đó (`show_announcement`, dùng
widget thông báo có sẵn của game, kiểu "Autosaving...") trước đó chỉ là
hàm riêng trong `autoregen/src/regen.rs`, không dùng lại được - chuyển
sang `common::announce::show_announcement` (xem README `autoregen`, mục
cùng ngày) rồi gọi thẳng từ `common::reload::run()` (watcher `ReloadKey`
dùng chung mà mod này đã gọi sẵn) - không cần sửa gì thêm trong
`drop_rate.rs`/`lib.rs`, banner **"Config reloaded"** tự xuất hiện mỗi lần
bấm F5.

## `LogFile=false` giờ không tạo file log nữa - sửa trong `common::logger` (2026-09-23)

Người dùng phát hiện (qua PassiveRunes): `LogFile=false` nhưng file `.log`
vẫn được tạo. Đúng là trước đó `logger::init` được gọi vô điều kiện trong
`lib.rs` - file luôn tạo, `LogFile` chỉ gate log chi tiết - trái với tên
key và với cách RiseArcher/RuneMultiplier vốn làm. Người dùng xác nhận hành
vi đúng: `LogFile=true` mới tạo file; muốn có log sẵn thì deploy với mặc
định `LogFile=true` (mod này mặc định `true`).

Sửa tập trung trong `shared/src/logger.rs`: `init` chỉ ghi nhớ đường dẫn,
file được tạo (truncate) ở dòng log đầu tiên khi `LogFile=true`, `LogFile`
đọc lại mỗi lần ghi - bật/tắt bằng `ReloadKey` có hiệu lực ngay. Comment
"log is always on" trong `lib.rs` đã bỏ; mô tả key trong ini đổi thành
"Write DropMultiplier.log next to the DLL (for troubleshooting). Off = no log
file". Các mục cũ hơn trong README nói "file log luôn được tạo bất kể
`LogFile`" giờ đã lỗi thời.


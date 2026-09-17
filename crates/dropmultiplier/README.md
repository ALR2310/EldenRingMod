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
- **Dùng chung crate [`engine`](../../engine)** thay vì tự có bản
  `player.rs`/`task.rs`/`reload.rs` riêng - xem README của `autoregen`, mục
  cùng ngày, để biết `engine` là gì và vì sao nó tách ra đúng lúc mod này
  cần lần thứ 2 (sau `sometweaks`/`risearcher`, cả 2 đều tự có bản
  `task.rs`/`player.rs`/`reload.rs` y hệt nhau). `DropMultiplier` là mod
  **đầu tiên trong workspace này khởi tạo mà dùng thẳng `engine` ngay từ
  đầu**, không phải migrate từ code riêng như `autoregen` đã làm.
- Nhờ dùng `engine::task`, mod này **không mang theo rủi ro `rva::get()`
  version-lock** mà bản gốc trong `sometweaks` vẫn còn (xem README
  `autoregen`, mục "AOB thay `rva::get()`...") - `engine::task::wait_for_cs_task`
  đã dùng cách tra `CSTaskImp::instance()` theo tên (không qua RVA) ngay từ
  đầu.

Không đổi: toàn bộ thuật toán `scale_row`/công thức `ChancePercent`, cách
snapshot `ORIGINAL_BASE_POINTS` để hot-reload không cộng dồn, cách gate
`SoloParamRepository`/`WorldChrMan.main_player` trước khi đọc param - xem
comment đầu `src/drop_rate.rs` cho chi tiết đầy đủ (giữ nguyên từ bản gốc).

# DropMultiplier: panic `rows_mut()` với regulation của Convergence

**Status 2026-09-27: ĐÃ SỬA (phát hành 1.0.2), test in-game trên 1.16.2 + Convergence.**

Mod không có tác dụng với Convergence 3.0.2.0: log dừng ngay sau `snapshotting
ItemLotParam_enemy...` bằng panic `param_repository.rs:357:22: called
Option::unwrap() on a None value`. Gốc rễ nằm ở `fromsoftware-rs`, không riêng
mod này, nên cách sửa là quy tắc chung cho cả workspace: **không dùng
`ParamFile::rows()`/`rows_mut()` của fromsoftware-rs, dùng
`common::params::for_each_row_mut`** (`shared/src/params.rs`).

## Triệu chứng

Báo lỗi trên Nexus (bản 1.0.1, game 1.17 + Convergence 3.0.2.0 + Seamless
Co-op; người báo nói vanilla cũng lỗi nhưng không tái tạo được). Bấm ReloadKey
vẫn hiện banner (banner đến từ `common::reload`, một task riêng), nhưng thread
của mod đã chết trước khi đăng ký task reload của chính nó, và mutex snapshot
bị poisoned, nên drop rate không bao giờ đổi.

Tái tạo: vanilla 1.17 chạy bình thường (5135 dòng); 1.16.2 + Convergence panic
y hệt.

## Nguyên nhân

Dòng 357 nằm trong `ParamFile::rows_mut()`:
`get_row_by_index_mut(lookup.index).unwrap()`. Hàm này duyệt **lookup table**
(cặp `(param_id, index)` sắp theo ID) mà game gắn sau mỗi param file, nhưng
định cỡ bảng theo `row_count` trong header file (`u16`).

Log chẩn đoán cho thấy header của `ItemLotParam_enemy` trong Convergence ghi
**4631** dòng, còn số dòng runtime (`u32` trong metadata 0x10 byte trước file,
thứ game thật sự dùng để dựng lookup table) là **4630**. Game dựng bảng thiếu
1 phần tử so với header (nhiều khả năng file của Convergence có 1 ID dòng bị
trùng), fromsoftware-rs đọc lố 1 phần tử ra vùng nhớ rác, ra index vượt
`row_count` rồi panic.

## Giả thuyết đã thử và bỏ

- **Tràn `u16` của `row_count`** (Convergence có thể có hơn 65535 dòng): bỏ, vì
  header chỉ có 4631 dòng.
- **Bố cục param khác nhau giữa 1.16.2 và 1.17**: bỏ, slot 20 đúng là
  `ItemLotParam_enemy` / `ITEMLOT_PARAM_ST` ở cả hai bản.

## Cách sửa (`shared/src/params.rs`)

- `for_each_row_mut::<P>()`: duyệt theo index qua row descriptor của chính file
  (`get_row_by_index_mut` tới khi trả `None`), không đụng lookup table.
- `check::<P>()`: xác minh slot thật sự chứa đúng param trước khi ghi
  (fromsoftware-rs chỉ kiểm tra bằng `debug_assert!`, bản release không có).
- `describe::<P>()`: in tên resource/struct, paramdef version, số dòng header
  và runtime, để log báo lỗi tự trả lời được.
- `row_ids::<P>()` (thêm sau, xem `dropmultiplier_materials.md`): đọc ID thẳng
  từ row descriptor, tự đối chiếu data offset với `get_row_by_index`; lệch thì
  trả `None` chứ không ra ID sai.

Trong `drop_rate.rs`: snapshot đổi từ `HashMap<u32, Points>` (theo ID) sang
`Vec<Points>` (theo index, ổn định trong một phiên); mutex dùng
`unwrap_or_else(into_inner)` để không panic dây chuyền; `apply` trả
`Option<usize>`, `None` (kèm log `[ERROR]`) nếu `check` không qua.

Dòng log mới khi chạy đúng:
`snapshotting ItemLotParam_enemy (slot 20): resource 'ItemLotParam_enemy', struct 'ITEMLOT_PARAM_ST' v4, 4631 row(s) (runtime: 4630)...`

## Phạm vi còn lại

Các mod khác vẫn dùng `rows()`/`rows_mut()` (chưa đổi): `risearcher`
(`EquipParamWeapon`), `sometweaks` (`ShopLineupParam`, `EquipParamGem`...),
`infiniteailment` (`SpEffectParam`). Chúng chỉ panic nếu đúng param đó bị lệch
header/runtime trong regulation đang dùng; người báo lỗi xác nhận Infinite
Ailments chạy tốt trên Convergence. `sometweaks::drop_rate` (code gốc của mod
này) đã được sửa cùng lúc.

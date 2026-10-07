# InfiniteAilment: dòng thời gian

Mỗi dòng một thay đổi, mới nhất ở dưới cùng. Chuyện nào có phân tích dài thì
link sang `docs/`. Bản nhật ký chi tiết cũ (trước 2026-10-07) nằm trong lịch
sử git của `README.md`.

| Ngày | Thay đổi | Chi tiết |
|---|---|---|
| 2026-09-21 | Tạo mod (tách riêng khỏi `sometweaks`): ý định sửa dòng chung `505` thành `EffectEndurance=-1`, suy ra từ CSV, chưa test | [docs](docs/row_discovery.md) |
| 2026-09-22 | Bỏ hướng ghi dòng `505` vì nghi ngờ suy luận từ CSV (chuỗi damage dùng chung với Bleed); chỉ đọc và log để chẩn đoán | [docs](docs/row_discovery.md) |
| 2026-09-22 | 1.0.0: lọc trực tiếp `SpEffectParam` đang chạy theo chữ ký field (175 dòng Scarlet Rot, 203 dòng Poison), ghi `Duration`/`PercentDamage`/`FixedDamage`; xác nhận trong game | [docs](docs/row_discovery.md) |
| 2026-09-23 | `LogFile=false` không còn tạo file log (sửa `common::logger`) | |
| 2026-09-24 | 1.0.1: sửa đường dẫn DLL có ký tự không phải ASCII làm mod bỏ qua ini (`common::dll_dir`) | |
| 2026-09-26 | Log: `[INFO ]` thành `[INFO]`; thêm phiên bản game và danh sách DLL đã nạp (`common::diag`) | |
| 2026-10-05 | Version thật trong thuộc tính file DLL (`build.rs` + `winresource`); thêm key `ReloadBanner` | |
| 2026-10-07 | Đổi thư mục `crates/` thành `mods/`; tách README thành README + HISTORY + docs/ | |

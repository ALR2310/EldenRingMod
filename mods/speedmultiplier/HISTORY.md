# SpeedMultiplier: dòng thời gian

Mỗi dòng một thay đổi, mới nhất ở dưới cùng. Chuyện nào có phân tích dài thì
link sang `docs/`. Bản nhật ký chi tiết cũ (trước 2026-10-07) nằm trong lịch
sử git của `README.md`.

| Ngày | Thay đổi | Chi tiết |
|---|---|---|
| 2026-09-29 | Nghiên cứu ban đầu: tìm các "núm" tốc độ (`animation_speed`, HKS, ...), loại các hướng SpEffect/RideParam/sửa TAE; IDA không lần ra ai ghi `animation_speed` | [docs](docs/animation_speed_research.md) |
| 2026-09-29 | SpeedProbe và hai đợt test: `animation_speed` giữ qua các frame, `motion_multiplier` bị reset, `hks_root_motion_mult` là đích của HKS | [docs](docs/animation_speed_research.md) |
| 2026-09-29 | Bộ lọc theo nhóm hành động (`speed.rs`): key tổng, Cast theo prefix TAE, nhóm Item (mở rộng dần, tách `050190` ra Other), đổi tên key | [docs](docs/action_groups.md) |
| 2026-09-29 | Gỡ phần ép field của probe; 1.0.0 phát hành (Nexus 11173) với mặc định do người dùng chốt | [docs](docs/animation_speed_research.md) |
| 2026-10-02 | Chuyển cấu hình sang TOML (`common::toml_config`) để chứa được danh sách quy tắc | [docs](docs/config_toml.md) |
| 2026-10-02 | `[[Override]]`: tốc độ khác khi người chơi có SpEffect | [docs](docs/config_toml.md) |
| 2026-10-02 | Nhóm riêng cho đòn chí mạng; điều tra uống bình khi chạy (giới hạn của game, không sửa) | [docs](docs/action_groups.md) |
| 2026-10-02 | Tách Movement thành Walk/Run/Sneak; bố cục `[Player]`/`[Torrent]`, nhóm nhảy; migration cấu hình theo phiên bản; key `ReloadBanner` | [docs](docs/config_toml.md) |
| 2026-10-02 | Mặc định chỉ nằm trong template, cấu hình xếp lớp (nhánh `feat/modmenu`, việc làm menu đang tạm dừng) | [docs](docs/config_toml.md) |
| 2026-10-03 | 1.1.0: sửa tốc độ không có tác dụng với Seamless Co-op (hook lại getter của `animation_speed`) | [docs](docs/seamless.md) |
| 2026-10-05 | Version thật trong thuộc tính file DLL (`build.rs` + `winresource`) | |
| 2026-10-06 | 1.2.0: nhóm thang, điều kiện equip load trong `[[Override]]`, sửa tia Placidusax's Ruin lệch nhịp | [docs](docs/action_groups.md) |
| 2026-10-07 | Đổi thư mục `crates/` thành `mods/`; tách README thành README + HISTORY + docs/; bảng TAE chuyển vào `docs/`; kiểm tra bằng IDA: mẫu AOB của phần Seamless duy nhất trên 2.6.2.0, 2.7.0.0, 2.7.1.0 | [docs](docs/seamless.md) |

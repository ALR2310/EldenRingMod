# TODO

Việc cần làm, chia theo mod. Lý do / chi tiết kỹ thuật ghi trong
`crates/<mod>/README.md` (nhật ký phát triển), không ghi ở đây.

## SpiritMultiplier

- [x] GhostColor=false không có tác dụng với ELDEN RING Reforged (Nexus, Quantum240, 2026-09-30) - sửa ở 1.1.2 (tắt vfx màu, không gỡ SpEffect)
- [ ] Spirit dịch chuyển tới người chơi khi bị bỏ lại (hoãn từ trước 1.0.0; chỉ chỉnh `buddyWarp_*` không có tác dụng)
- [ ] Quyết định giữ hay bỏ log `stones:` trong SlotProbe (thêm để dò lỗi MultiSpirit/SummonAnywhere)
- [ ] `EnemyProbe` dễ crash - sửa hoặc bỏ hẳn
- [x] Bug: bấm lại Ash đã nâng cấp (+1..+10) gọi thêm spirit thay vì cho về - MultiSpirit (Nexus bug report "unsummon", MonkeyDLuffy2426, 2026-10-01, Mimic Tear / Lhutel +4, bản 1.1.2) - sửa ở 1.1.3 (làm tròn `100 * (id / 100)` như `DoSummon`)

## SpeedMultiplier

- [ ] Tách tốc độ đi bộ / chạy / chạy nhanh (sprint) thành key riêng, cho cả player lẫn Torrent (Nexus, cfzlbj, 2026-10-01)
  - Hiện tại `PlayerMovement` và `Torrent` mỗi bên chỉ có 1 hệ số chung - cần dò đuôi anim từng kiểu: player đã thấy `020010`/`020110`/`020210` trong log nhưng chưa rõ cái nào là đi bộ / chạy / sprint; Torrent có `020000`-`020004`

## RuneMultiplier

- [ ] Nghiên cứu: hệ số riêng cho từng nguồn rune, vd. giết địch x2, bán đồ x0.5 (Nexus, julianpratt, 2026-08-31)
  - Hiện tại 1 hệ số chung cho mọi nguồn (giết địch, dùng item, bán đồ...) - cần xem các nguồn có đi qua cùng 1 hàm cộng rune không, và có phân biệt được nguồn tại đó không

## Chung

- [ ] Đồng bộ `version` trong `Cargo.toml` của các mod cũ với version mới nhất trên Nexus (vd. `autoregen` vẫn `2.0.0`)
- [ ] Đăng `windowresize` lên Nexus

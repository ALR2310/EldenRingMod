# TODO

Việc cần làm, chia theo mod. Lý do / chi tiết kỹ thuật ghi trong
`crates/<mod>/README.md` (nhật ký phát triển), không ghi ở đây.

## SpiritMultiplier

- [x] GhostColor=false không có tác dụng với ELDEN RING Reforged (Nexus, Quantum240, 2026-09-30) - sửa ở 1.1.2 (tắt vfx màu, không gỡ SpEffect)
- [ ] Spirit dịch chuyển tới người chơi khi bị bỏ lại (hoãn từ trước 1.0.0; chỉ chỉnh `buddyWarp_*` không có tác dụng)
- [ ] Quyết định giữ hay bỏ log `stones:` trong SlotProbe (thêm để dò lỗi MultiSpirit/SummonAnywhere)
- [ ] `EnemyProbe` dễ crash - sửa hoặc bỏ hẳn

## RuneMultiplier

- [ ] Nghiên cứu: hệ số riêng cho từng nguồn rune, vd. giết địch x2, bán đồ x0.5 (Nexus, julianpratt, 2026-08-31)
  - Hiện tại 1 hệ số chung cho mọi nguồn (giết địch, dùng item, bán đồ...) - cần xem các nguồn có đi qua cùng 1 hàm cộng rune không, và có phân biệt được nguồn tại đó không

## Chung

- [ ] Push `main` lên remote
- [ ] Đồng bộ `version` trong `Cargo.toml` của các mod cũ với version mới nhất trên Nexus (vd. `autoregen` vẫn `2.0.0`)
- [ ] Đăng `windowresize` lên Nexus

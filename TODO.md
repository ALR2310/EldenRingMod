# TODO

Việc cần làm, chia theo mod. Lý do / chi tiết kỹ thuật ghi trong
`crates/<mod>/README.md` (nhật ký phát triển), không ghi ở đây.

## SpiritMultiplier

- [x] GhostColor=false không có tác dụng với ELDEN RING Reforged (Nexus, Quantum240, 2026-09-30) - sửa ở 1.1.2 (tắt vfx màu, không gỡ SpEffect)
- [ ] Spirit dịch chuyển tới người chơi khi bị bỏ lại (hoãn từ trước 1.0.0; chỉ chỉnh `buddyWarp_*` không có tác dụng)
- [ ] Quyết định giữ hay bỏ log `stones:` trong SlotProbe (thêm để dò lỗi MultiSpirit/SummonAnywhere)
- [ ] `EnemyProbe` dễ crash - sửa hoặc bỏ hẳn
- [ ] Reforged: Spirit Fury / Fortune of the Spiritcaller hỏng - "cannot resummon/aggro the spirits", Spiritcaller ring không hoạt động (Nexus, puffintoast, 2026-10-01; chưa rõ bản mod họ dùng)
  - Cơ chế Reforged (CHANGELOG.txt của ERR): bấm lại Ash khi spirit đang ở ngoài = **kích nộ (Spirit Fury)**, không phải cho về; cho về bằng item Spirit-Severing Blade. "Aggro" = Spirit Fury, "un-aggro'ed" = trạng thái bình tĩnh mặc định
  - Nghi MultiSpirit: chiếm đúng thao tác bấm lại Ash (patch `DoSummon`, `DisappearAll`, `GetBuddyState`/`CanUseItem`/UI) - có thể đè lên hook của `reforged.dll` ở cùng hàm. Đã nhờ họ thử `MultiSpirit=false` (F5), chờ phản hồi
  - Hướng sửa lâu dài: nhận diện Reforged (vd. `reforged.dll` trong module đã nạp) rồi nhường thao tác bấm lại Ash cho Spirit Fury; cần tự tái hiện (cần fortune Spiritcaller, rơi từ boss Road's End Catacombs)
- [ ] Reforged: làm rõ đề xuất "summon spirits un-aggro'ed" (cùng người báo) - chờ họ giải thích
- [ ] Reforged + MultiSpirit: kích Spirit Fury riêng cho từng Ash (hiện Fury 1 nhóm thì mọi Ash đều xám) - cần tìm cách Reforged đánh dấu "đang Fury" và cái gì làm Ash xám (SpEffect trên người chơi/spirit? `reforged.dll`?); lưu ý cân bằng
- [ ] Reforged: NoRestResummon không có tác dụng sau khi cho về bằng Spirit-Severing Blade - Ash đã gọi trước đó không gọi lại được tới khi nghỉ (người dùng test 2026-10-01)
  - Là luật của Reforged gốc (người dùng xác nhận: sau Blade ô Ash xám tới khi ngồi grace), không phải do mod; NoRest chỉ gỡ khoá vanilla (`summonedEventFlagId` của bia đá)
  - Không phải SpEffect triệu hồi chuẩn: `9540/9541/9545/9547/9560` (GameSystemCommonParam `onBuddySummon_*`) trong Reforged gần như rỗng, `9560` "Spirit Summon Disappeared" không có tác dụng → nghi event flag của script Reforged (EMEVD/ESD) hoặc `reforged.dll`
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

# Giới hạn 60 nhân vật active và kẻ địch biến mất

**Status 2026-10-08: sửa xong, luôn hoạt động (không có key bật/tắt), đã test ở Stormgate với 60 spirit. Mã: `src/activate_limit.rs`.**

## Triệu chứng và nguyên nhân

Gọi 60 spirit trước Stormgate thì lính ở đó biến mất. Nguyên nhân: spirit chiếm suất trong giới hạn **60 nhân vật active** của engine.

- Không phải omission (`ChrIns::omission_mode`, budget `WorldChrMan+0x1F220/+0x1F224`, `sub_14050FEA0`): chỉ hạ tần suất update, làm giật chứ không làm biến mất.
- Không phải ngưỡng khoảng cách (`sub_1403FBE70`, `chr_activate_threshold +0x40C`, cờ `+0x1CA` bit 2): không đếm số lượng.
- Thủ phạm: `sub_140510970` gom ứng viên activate vào vector `WorldChrMan+0x1F1D0` và tăng bộ đếm `WorldChrMan+0x1E618` cho mọi nhân vật đã active; `sub_14050F9E0` chỉ activate `limit - bộ_đếm` con đầu, phần còn lại bị cắt (`sub_1403E93A0(chr, 2)`).
- `limit` nằm trong singleton `qword_143D6A208` (constructor `sub_145AE6B75`): `+0xE8 = 1` (bật), **`+0xEC = 60`, `+0xF0 = 60`**, `+0xF4` = cờ chế độ thay thế, `+0xF8/+0xFC = 40`. Chỉ áp dụng khi `+0xE8` hoặc `+0xF4` khác 0.

## Cách sửa

AOB `48 8B 05 ?? ?? ?? ?? 80 B8 E8 00 00 00 00 75 0D 80 B8 F4 00 00 00 00 0F 84` (`0x14050FAEC`, duy nhất) → địa chỉ singleton. Mỗi giây đếm slot có nhân vật trong `summon_buddy_chr_set` từ `BAND_START` (20) trở lên (bỏ Torrent, tính cả spirit người chơi co-op hiện trên máy mình) và đặt `+EC/+F0/+F8/+FC = giá trị gốc + số đó`. Chỉ đọc con trỏ entry, không đọc `ChrIns`. Không có spirit thì giữ đúng vanilla (không tốn CPU thêm). Giá trị gốc `[60, 60, 40, 40]` được chụp lần đầu thấy singleton.

`ActiveCharacterLimit` (ini, debug, mặc định 0): `> 0` ghi đè giá trị cố định (test 120 → lính không biến mất lại; F5 về 60 → biến mất lại; về 0 → trả giá trị gốc). Comment ini ghi "DO NOT CHANGE".

## Bài học

- `EnemyProbe` (`src/enemy_probe.rs`, ini mặc định `false`) **gây crash 2 lần**: đọc `ChrIns` của entry `Unloaded` (con trỏ cũ đã free), rồi vẫn crash khi duyệt `chr_sets` lúc map đang stream. Không dùng nếu chưa đổi cách duyệt (vd. `chr_inses_by_distance` của game).
- `WorldChrMan+0x1E618` là số nhân vật đã activate trong frame (game gốc đã chạm 60 sẵn); bit 9 của `load_state` **không** có nghĩa "bị cắt" (đã đính chính), đừng dùng làm bằng chứng.
- Agent tìm cộng đồng không thấy báo cáo công khai nào về triệu chứng này, cũng không có mod nào nâng giới hạn.
- Ban đầu có key `KeepEnemySlots`; bỏ 2026-09-26 vì kẻ địch biến mất là lỗi của mod, không phải tính năng để bật/tắt.
- Chi phí thật là game xử lý thêm nhân vật khi giới hạn được nâng (CPU nặng hơn khi vừa nhiều spirit vừa nhiều kẻ địch).

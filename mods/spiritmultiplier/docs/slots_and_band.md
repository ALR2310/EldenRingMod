# Slot, band và trần số spirit

**Status 2026-10-08: hoạt động, đã test offline (100 spirit) và Seamless Co-op 2 người. `MaxSpirits` mặc định và tối thiểu là 60. Mã: `src/band.rs`, `src/chain.rs`, `src/probe.rs`.**

## Kết luận

- Spirit nằm trong `WorldChrMan.summon_buddy_chr_set` (`ChrSet<ChrIns>`, chứa cả spirit lẫn Torrent). Capacity do game tạo là **80** (`sub_1404954B0`: `alloc(0x500)`, `capacity = 0x50`, 3 imm32). Slot #0 = Torrent (có sẵn từ lúc vào world), #1..#19 dành cho Torrent người chơi khác, band người chơi local bắt đầu ở **#20**.
- Trần 10 con của vanilla nằm ở `sub_140493380` (spawn 1 spirit): `band_start = 10 * (player_index + 2)`, `cursor %= 10`, duyệt 10 slot tìm entry trống, hết thì trả null (spirit bị bỏ). `sub_140493D60` tính cursor kế: `(slot - 20 + 1) % 10`. `SummonBuddyManager.last_buddy_slot = +0xBC`.
- Offset (bản 2.7.1.0): `WorldChrMan.summon_buddy_chr_set = +0x10F90`, `summon_buddy_manager = +0x1E538`.
- Dismiss/despawn không duyệt band hard-code (test 30 con ở #20-#49, dọn sạch cả slot >= #30).

## Cách mod vượt trần (`src/band.rs`, cài 1 lần lúc khởi động)

1. AOB `0F B6 C2 83 C0 02 8D 04 80 44 8D 14 00 B8 67 66 66 66 41 F7 E9` (`0x1404933AF`, duy nhất): thay cả vòng tìm slot bằng stub; band dài `MaxSpirits` cho người chơi index 0, kiểm tra `slot < capacity`, thoát đúng 2 lối của vanilla.
2. AOB `83 E8 14 78 22 8D 48 01 B8 67 66 66 66 F7 E9 C1 FA 02` (`0x140493D7C`): next-cursor trả `slot - 19` không wrap; stub tự đổi về vị trí trong band của người gọi `(cursor - (start - 20)) mod len`.
3. Chỉ khi `MaxSpirits > 60`: AOB `CAPACITY_INIT_AOB` (`0x140495534`) ghi `20 + MaxSpirits` vào 3 imm32 nới capacity (chạy trước khi world được tạo). Patch lỗi thì band tự lùi về 60.

`MaxSpirits` đọc 1 lần lúc khởi động (cần restart, F5 không áp dụng). Kẹp `60..=1000`: ở 60 mảng slot 80 của game giữ nguyên như vanilla. Test `MaxSpirits=100` offline: 100 sói ở #20-#119, load dần ~2 giây, dọn sạch, không crash; FPS tụt theo số spirit; **không tìm tiếp trần heap nhân vật** của engine.

`src/chain.rs` kéo dài chain SpEffect → BuddyParam (`WorldChrMan.summon_buddy_manager.trigger_speffect_to_buddy_map`) theo `Multiplier`/`Amount`, cap `band_len()`; copy từ `sometweaks/src/spirit/summon_count.rs` (không đưa lên `common`, người dùng chọn 2026-09-25). Hot reload; đặt về mặc định thì chain trả độ dài vanilla.

## Seamless Co-op

- Seamless tự nới capacity lên **1000** trên mọi máy. `sub_140493210` (tạo spirit nhận qua mạng) **không kiểm tra slot < capacity**, ghi thẳng `entries[slot]` và nếu slot đã có nhân vật thì trả nhân vật đó → band của mọi người chơi phải nằm gọn trong capacity của mọi máy và không chồng nhau.
- Stub chia band theo capacity thật (`band_layout`, có test): index 0 `[20, 20 + MaxSpirits)`; index 1..5 `part = (capacity - 20) / 6`, bắt đầu `20 + index × part`, dài `min(MaxSpirits, part)`. Capacity 80 (co-op vanilla) → `part = 10` → đúng band vanilla. Capacity 1000 → band bắt đầu `20, 183, 346, 509, 672, 835`.
- Test 2 người cùng cài mod (`Amount=30`): người join gọi đủ 30 con ở band #183.., cả 2 máy thấy đủ 60 spirit, dọn khớp nhau.
- Giới hạn: người chơi không cài mod vẫn dùng band vanilla #30..#79 (trùng band rộng của host); `MaxSpirits > 163` với Seamless thì band index 0 chồng band index 1; giả định mọi máy cùng capacity.
- Còn nghi vấn: đôi khi chỉ đồng bộ 1 phần spirit (11/30, 13/30) ngay sau khi 1 bên load lại khu vực; nghi do đồng bộ mạng của engine/Seamless (hoặc 2 game chung máy qua Sandboxie), chưa xác nhận.

## Hướng đã cân nhắc / bỏ

- B. Cấp phát lại mảng `entries` của ChrSet (rủi ro cao, engine có thể cache con trỏ/index) - không cần vì capacity 80 đã đủ cho 60.
- C. Spawn NPC ngoài hệ thống buddy - bỏ, mất AI "theo người chơi"/despawn theo Ash.
- Ghidra quá nhiễu (270 hàm khớp), chuyển sang IDA.

## Chẩn đoán

`SlotProbe` (ini, mặc định `false`): log capacity, histogram độ dài chain vanilla (`{1: 58, 2: 18, 3: 8, 4: 4, 5: 6}`), bố cục slot khi đổi, `last_buddy_slot`, và `stones:` (`+0x38`/`+0x3C`, cờ `+0xB5`/`+0xB7`).

## Tham chiếu

- Xung đột: hook "buddy slot-reserve" của Solid Uncapper (`0x1404933A9`) nằm trong đúng `sub_140493380` → không dùng chung khi họ bật Max Spirits Out / Multi Spirit.
- [activate_limit.md](activate_limit.md): vì sao 60 spirit làm kẻ địch biến mất.

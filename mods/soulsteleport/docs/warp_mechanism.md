# SoulsTeleport: cơ chế dịch chuyển (warp) tới một vị trí bất kỳ

**Status 2026-10-07: ĐANG PHÁT HÀNH (1.1.0), đã chứng minh trong game** (xuyên
overworld, `m18_00` và `m12_02` dưới lòng đất; cả trong Seamless Co-op mà không làm rớt
phiên). Code: `mods/soulsteleport/src/warp.rs`.

## Mục tiêu và các cách đã bỏ

Dịch chuyển nhân vật tới đúng chỗ một người chơi khác đang đứng, dù ở xa tới đâu. Lệnh
`/warpto` của SoulsChat chỉ warp tới grace cuối của người kia (event Lua `Lua_Warp_1` =
4085, id = grace - 1000), nên ý tưởng là cải tiến thành dịch chuyển thẳng tới người chơi.

Đã cân nhắc và bỏ:
- **Ghi thẳng `CSChrPhysicsModule.position`:** chỉ an toàn khi vùng đích đã load; trong
  Seamless Co-op người chơi thường ở rất xa nhau nên nhân vật rơi xuống địa hình chưa có.
- **Warp tới grace gần người kia nhất rồi đặt lại tọa độ:** grace gần nhất vẫn có thể
  ngoài vùng load (vùng trống lớn, khác tầng, block không có grace).

## Cơ chế đã chọn: dùng lại đúng trình tự game tự có

Phân tích tĩnh `eldenring.exe` (bắt đầu từ các trường `GameMan` trong fromsoftware-rs
`cs/game_man.rs`) tìm ra trình tự game dùng khi kết thúc phiên multiplayer để trả bạn về
đúng chỗ đang đứng (`sub_1405F36E0` / `sub_1405F34A0`, trong 2.7.1.0):

1. `RequestMoveMap` (`sub_14067BA20`) ghi `GameMan+0x14` = block đích, có chuẩn hoá block
   overworld (area 50..88).
2. `SetCustomSpawn` (`sub_14067B970`) ghi `GameMan+0xC90` tọa độ trong block (w = 1.0),
   `+0xCA0` hướng, và bật cờ `+0xCB0`.
3. `GameMan+0x10` (`warp_requested`) = 1: game hiện loading rồi chuyển map.

Sau khi load, hàm chọn chỗ xuất hiện (`sub_140AFE280`) thấy cờ `+0xCB0` thì lấy `+0xC90`
cộng gốc block đích làm vị trí spawn; `sub_14067A680` tự xoá cờ.

**Bước 3 cố tình ghi thẳng `warp_requested`** thay vì gọi hàm kích hoạt của game
(`sub_1405F89C0`): hàm đó còn gọi vào session manager khi đang online (`sub_1409F9A50`) và có
thể làm rớt phiên Seamless Co-op. Đã xác nhận trong game: ghi thẳng không làm rớt phiên.

Tọa độ lưu lấy thẳng từ `PlayerIns.block_position` và `current_block_id` (fromsoftware-rs),
đã là tọa độ trong block nên không cần tự đổi từ tọa độ thế giới.

## Chi tiết cài đặt

- `SetCustomSpawn` đọc hai vector bằng `movaps`, nên struct `Vec4` phải `#[repr(align(16))]`
  (sai là crash).
- Hướng quay dựng theo đúng layout game dùng cho `multiplay_join_orientation`
  (`sub_1406FC370`): `(0, yaw, 0, 0)`.
- Bản đầu lệch `+1` m theo trục x để hai nhân vật không đứng lồng vào nhau, đã bỏ vì game tự
  đẩy hai nhân vật đè lên nhau ra.
- **`warp::warp_pending`** (`GameMan.warp_requested` đang bật) dùng để bên warp không warp
  khi chính mình đang có move-map chờ, và để bên trả lời biết mình đang chuyển map.

## AOB thay cho RVA, không khoá theo phiên bản (2026-09-24)

Test đầu tiên trên exe 2.7.0.0 thất bại ngay khi khởi động: panic `Unsupported game version
2.7.0.0` tại `rva.rs`. Nguyên nhân: `GameMan::instance()` của fromsoftware-rs đi qua bảng
RVA theo từng phiên bản (`rva::get()`) mà chỉ có 2.7.1.0, khác `WorldChrMan`/`CSMenuManImp`
(tìm singleton theo tên). Đổi sang:

- `RequestMoveMap` / `SetCustomSpawn` tìm bằng AOB qua `common::memscan::find_pattern_in_module`.
  Mẫu cố tình gồm luôn các offset `GameMan` mà hàm dùng (`+0xB28`, `+0xC90/+0xCA0/+0xCB0`), nên
  nếu một bản sau đổi layout thì AOB hụt và **mod tự tắt, không ghi sai chỗ**.
- Con trỏ `GameMan` lấy từ chính lệnh `mov rax, [rip+disp32]` bên trong `SetCustomSpawn`
  (không cần RVA nào).
- `warp_requested` ghi thẳng `GameMan+0x10` (setter của game `sub_14067BCF0` là `mov [rax+10h], cl`).

### Kiểm tra tương thích (IDA, 2026-10-07)

Cả hai mẫu duy nhất ở cả ba bản exe:

| Bản exe | `RequestMoveMap` | `SetCustomSpawn` |
|---|---|---|
| 2.6.2.0 | `0x67ABD0` | `0x67AB20` |
| 2.7.0.0 | `0x67BA20` | `0x67B970` |
| 2.7.1.0 | `0x67BA20` | `0x67B970` |

## Đã xác nhận trong game (2026-09-24)

Test offline trên exe 2.7.0.0: mọi lần warp đều hiện loading rồi đặt nhân vật đúng chỗ đã
lưu. Các block đã thử: `0x3C2A2400` (overworld `m60_42_36`, Limgrave), `0x12000000` (`m18_00`,
dungeon mở đầu), `0x0C020000` (`m12_02`, Siofra, dưới lòng đất, tọa độ lớn cỡ
`(1459, -813, 1547)`). `RequestMoveMap` trả block y hệt đầu vào ở cả ba (chuẩn hoá overworld
không đổi gì với block lấy từ `current_block_id`).

## Giới hạn

Chưa chặn teleport khi người kia đang cưỡi Torrent trên không, trong trận boss hay đang chết.
Chưa thử warp khi bản thân đang cưỡi Torrent theo kế hoạch test ban đầu.

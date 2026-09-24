# SoulsTeleport

Addon **không chính thức** cho SoulsChat (mod chat của 2Pz cho Seamless
Co-op): mục tiêu là thêm lệnh `/teleport <player>` dịch chuyển (qua màn hình
loading) tới đúng vị trí của người chơi khác. Tên hiển thị dự kiến trên
Nexus: "Souls Teleport" (chưa đăng).

Hiện tại vẫn ở giai đoạn thử nghiệm - chỉ có phím tắt, chưa nối vào SoulsChat:

- `SaveKey` (mặc định `F7`): lưu block + tọa độ hiện tại của nhân vật.
- `WarpKey` (mặc định `F8`): warp (qua loading) về đúng chỗ đã lưu.
- `ReloadKey` (`F5`), `LogFile` như mọi mod khác.

Không khoá version: hàm game + `GameMan` tìm bằng AOB (xem mục 2026-09-24 thứ 2).

## Bối cảnh và lý do chọn cách này (2026-09-24)

Phân tích SoulsChat.dll (IDA): lệnh `/warpto` của nó chỉ warp tới grace cuối
của người kia (`CSLuaEventMan` event `Lua_Warp_1 = 4085`, id = grace - 1000).
Ý tưởng cải tiến là dịch chuyển thẳng tới người chơi.

Đã cân nhắc và bỏ:
- **Ghi thẳng `CSChrPhysicsModule.position`**: chỉ an toàn khi vùng đích đã
  load; trong Seamless Co-op người chơi thường ở rất xa nhau → rơi địa hình.
- **Warp tới grace gần người kia nhất rồi đặt lại tọa độ**: grace gần nhất vẫn
  có thể ngoài vùng load (vùng trống lớn, khác tầng, block không có grace).

Chọn: dùng lại đúng cơ chế game tự có. Phân tích tĩnh
`eldenring.exe` 2.7.1.0 (bắt đầu từ các trường `GameMan` trong fromsoftware-rs
`cs/game_man.rs`) tìm ra trình tự game dùng khi kết thúc phiên multiplayer để
trả bạn về đúng chỗ đang đứng (sub_1405F36E0 / sub_1405F34A0):

1. `sub_14067BA20` (`RequestMoveMap`) → `GameMan+0x14` = block đích (có chuẩn
   hoá block overworld, area 50..88).
2. `sub_14067B970` (`SetCustomSpawn`) → `GameMan+0xC90` tọa độ trong block
   (w = 1.0), `+0xCA0` hướng, bật cờ `+0xCB0`.
3. `GameMan+0x10` (`warp_requested`) = 1 → game hiện loading, chuyển map.

Sau khi load, hàm chọn chỗ xuất hiện (sub_140AFE280) thấy cờ `+0xCB0` thì lấy
`+0xC90` cộng gốc block đích làm vị trí spawn; `sub_14067A680` tự xoá cờ.

Bước 3 cố tình ghi thẳng `warp_requested` thay vì gọi hàm kích hoạt của game
(`sub_1405F89C0`): hàm đó còn gọi vào session manager khi đang online
(`sub_1409F9A50`), có thể làm rớt phiên Seamless Co-op.

Tọa độ lưu lấy thẳng từ `PlayerIns.block_position` + `current_block_id`
(fromsoftware-rs) - đã là tọa độ trong block, không cần tự đổi từ tọa độ
thế giới.

Chi tiết cài đặt (`src/warp.rs`):
- (Bản đầu dùng RVA cứng cho 2.7.1.0 - đã bỏ, xem mục dưới.)
- `SetCustomSpawn` đọc 2 vector bằng `movaps` → struct `Vec4` phải
  `#[repr(align(16))]`, sai là crash.
- Hướng quay dựng theo đúng layout game dùng cho `multiplay_join_orientation`
  (sub_1406FC370): `(0, yaw, 0, 0)`.

**Trạng thái:** build được, **chưa test trong game**. Cần thử: cùng block (gần),
khác ô overworld (xa), trong/ra legacy dungeon, đang cưỡi Torrent; sau đó mới
thử trong Seamless Co-op.

## Bỏ khoá version: AOB thay cho RVA + `GameMan::instance()` (2026-09-24)

Test đầu tiên trên exe **2.7.0.0** (bản ở `ELDEN RING Tarnished Edition`)
thất bại ngay khi khởi động: log `PANIC ... rva.rs:50:33: Unsupported game
version 2.7.0.0`. Nguyên nhân: `GameMan::instance()` của fromsoftware-rs đi
qua bảng RVA theo từng version (`rva::get()`), bảng đó chỉ có 2.7.1.0 - khác
`WorldChrMan`/`CSMenuManImp` (tìm singleton theo tên, không khoá version,
các mod khác vẫn chạy được trên 2.7.0.0).

Đổi sang:
- `RequestMoveMap` / `SetCustomSpawn` tìm bằng AOB qua
  `common::memscan::find_pattern_in_module`. Đã kiểm tra offline: mỗi mẫu
  khớp **đúng 1 chỗ** trong cả 2.6.2.0, 2.7.0.0 và 2.7.1.0. Mẫu cố tình
  gồm luôn các offset `GameMan` hàm đó dùng (`+0xB28`, `+0xC90/+0xCA0/+0xCB0`)
  nên nếu 1 bản sau đổi layout thì AOB hụt → mod tự tắt, không ghi sai chỗ.
- Con trỏ `GameMan` lấy từ chính lệnh `mov rax, [rip+disp32]` bên trong
  `SetCustomSpawn` - không cần RVA nào.
- `warp_requested` ghi thẳng `GameMan+0x10` (setter của game sub_14067BCF0
  là `mov [rax+10h], cl`).

**Trạng thái:** build được, chờ test lại trên 2.7.0.0.

## Đã xác nhận trong game (2026-09-24)

Test offline trên exe 2.7.0.0 (bản AOB ở trên): mọi lần `WarpKey` đều hiện
màn hình loading rồi đặt nhân vật đúng chỗ đã lưu. Block đã thử (theo log):
- `0x3C2A2400` - overworld `m60_42_36` (Limgrave);
- `0x12000000` - `m18_00` (dungeon mở đầu game);
- `0x0C020000` - `m12_02` (Siofra, dưới lòng đất, tọa độ lớn cỡ
  `(1459, -813, 1547)`).

`RequestMoveMap` trả block y hệt đầu vào ở cả 3 (chuẩn hoá overworld không
đổi gì với block đã lấy từ `current_block_id`).

Cơ chế "load block + spawn đúng tọa độ" coi như đã chứng minh. Việc tiếp
theo cho addon SoulsChat: lấy block + tọa độ của người chơi khác trong
Seamless Co-op (còn trong `player_chr_set` khi ở xa không?), rồi hook lệnh
`/teleport`. Chưa thử trong Seamless Co-op.

## Thêm `ListPlayersKey` để khảo sát Seamless Co-op (2026-09-24)

`ListPlayersKey` (mặc định `F9`) log mọi entry trong
`WorldChrMan.player_chr_set`: tên nhân vật (`PlayerGameData.character_name`),
`chr_type`, `current_block_id`, `block_position` và tọa độ Havok
(`CSChrPhysicsModule.position`). Mục đích: trong Seamless Co-op, người chơi ở
xa (vùng chưa load) có còn entry ở đây với block/tọa độ vẫn cập nhật không -
quyết định addon có phải tự đồng bộ tọa độ qua mạng hay không. Chưa chạy thử.

## Đổi tên `teleporttest` → `soulsteleport` (2026-09-24)

Sau khi cơ chế warp đã được xác nhận, crate chuyển từ bản thử nghiệm sang
dự án phát triển thật, đặt tên theo hướng addon cho SoulsChat. Đã cân nhắc:
- "SoulsChat: Teleport" - bỏ: kiểu "Mod gốc: Tính năng" dễ bị hiểu là module
  chính thức của 2Pz (báo lỗi nhầm chỗ, gắn tên tác giả khi chưa xin phép).
- "Souls Teleport" - chọn: cùng "họ" với SoulsChat nhưng không mượn nguyên
  tên; mô tả Nexus sẽ ghi rõ "unofficial addon for SoulsChat".

Đổi: thư mục `crates/teleporttest` → `crates/soulsteleport`, package
`soulsteleport`, `[lib] name = "SoulsTeleport"` → `SoulsTeleport.dll`,
`TeleportTest.ini` → `SoulsTeleport.ini`, log `SoulsTeleport.log`. Key ini
giữ nguyên (`SaveKey`/`WarpKey`/`ListPlayersKey`). Các mục phía trên viết
lúc còn tên TeleportTest - nội dung kỹ thuật vẫn đúng.

## Test Seamless Co-op lần 1: 2 người đứng gần nhau (2026-09-24)

2 instance trên 1 máy (tài khoản Steam thứ 2 chạy trong Sandboxie), cùng
phiên Seamless Co-op, đứng cạnh nhau ở `m60_42_37`. `F9` ra:

```
Player #1: 'AnLe -2' type Local block 0x3C2A2500 local (15.28, 110.75, 113.58) havok (7.28, 6.75, 1.58)
Player #2: 'ALR'     type Local block 0xFFFFFFFF local (0.00, 0.00, 0.00)     havok (8.13, 6.67, 1.52)
```

Kết luận:
- Đồng đội **có** trong `player_chr_set`.
- `PlayerIns.current_block_id` / `block_position` **chỉ được cập nhật cho
  chính mình** - của đồng đội là `-1` / `0`, không dùng được.
- Tọa độ Havok (`CSChrPhysicsModule.position`) của đồng đội **vẫn sống và
  chính xác** (cách nhau ~0.9 m, khớp thực tế).
- Seamless Co-op báo cả 2 là `chr_type Local` (không phải phantom).

Hệ quả: khi ở gần, đích warp tính được từ chính mình:
`block = block của mình`, `local = local của mình + (havok đồng đội - havok của mình)`.
Khi ở xa thì chưa biết - có thể Havok của đồng đội nằm ngoài vùng Havok
đang load. Đã thêm vào log `F9` các trường của `ChrIns`: `block_id`,
`block_origin`, `chunk_position` - biết đâu các trường này còn giữ block
của đồng đội. Cần test lần 2: 2 người ở rất xa nhau.

## Test Seamless Co-op lần 2: ở xa → mất tọa độ; sửa bắt phím (2026-09-24)

Log lần 2 (host `AnLe -2`, người join `ALR`):
- Gần nhau: Havok của `ALR` cập nhật liên tục (`(7.66, 6.59, 6.53)` →
  `(6.28, 6.62, 6.57)` khi người join nhích đi).
- Sau khi người join dịch chuyển đi xa: `ALR` **vẫn còn** trong
  `player_chr_set` nhưng Havok về `(0, 0, 0)` - máy host không còn biết vị trí.
- `ChrIns.block_id` của `ALR` luôn `0x3C2A2500` kể cả khi đã đi xa (không đáng
  tin), `chunk_position` luôn `0`.

→ **Kết luận: addon phải tự gửi vị trí qua mạng.** Mỗi instance đọc block +
tọa độ của chính mình (`PlayerIns.current_block_id`/`block_position` - luôn
đúng cho main player) rồi gửi cho người khác; công thức Havok ở mục trên chỉ
còn là dự phòng khi ở gần.

Cũng trong lần test này: bấm `F9` ở cửa sổ người join nhưng log lại ra ở host,
log của bản join (trong sandbox) không có dòng nào. Nguyên nhân: mod dùng
`common::input::is_key_pressed` (`GetAsyncKeyState & 1`) - bit "đã bấm từ lần
gọi trước" là cờ chung toàn hệ thống, instance nào poll trước thì "ăn" mất.
Sửa: đổi sang `eldenring::util::input::is_key_pressed` (fromsoftware-rs,
`GetKeyState` - trạng thái phím theo luồng, chỉ cập nhật khi cửa sổ của chính
process nhận phím), giống `common::reload`. `common::input::is_key_pressed`
cũng được sửa riêng cho các mod poll từ thread thường (xem WeightMultiplier).

## Đồng bộ vị trí qua Steam P2P + `WarpToPartnerKey` (2026-09-24)

Theo kết luận test lần 2 (ở xa là mất tọa độ), thêm đồng bộ vị trí:

- `src/steam.rs`: binding tối thiểu tới `ISteamNetworkingMessages`, lấy bằng
  `GetProcAddress` từ `steam_api64.dll` mà game đã nạp sẵn (flat C API - không
  cần Steamworks SDK/crate). Chỉ đọc 2 trường đầu của
  `SteamNetworkingMessage_t` (con trỏ + kích thước payload), giải phóng qua
  export `SteamAPI_SteamNetworkingMessage_t_Release` - không phụ thuộc layout
  sâu hơn của struct. Người gửi nằm luôn trong payload thay vì đọc
  `m_identityPeer`.
- `src/sync.rs`: mỗi 500 ms gửi block + tọa độ trong block + yaw + tên nhân vật
  của chính mình (gói 66 byte, magic `STP1`) tới mọi người khác trong
  `CSSessionManager.players` (singleton tra theo tên - không khoá version;
  mỗi entry có `steam_id`, `steam_name`, `is_local_player`), unreliable, kênh
  riêng `0x5354` (SoulsChat dùng 42 - không đụng nhau). Nhận mỗi frame, lưu
  theo SteamID; quá 5 s không có gói mới thì coi như không biết.
  `AcceptSessionWithUser` được gọi lại mỗi lần gửi, để gói từ 1 người chưa
  từng gửi cho mình không bị Steam bỏ.
- `WarpToPartnerKey` (mặc định `F10`): warp tới đồng đội có vị trí nhận mới
  nhất, đúng tọa độ của họ - bản thử thay cho `/teleport <player>` trước khi
  hook SoulsChat. (Bản nháp đầu lệch `+1` m theo trục x để không đứng lồng
  vào nhau - đã bỏ: game tự đẩy 2 nhân vật đè lên nhau ra.)
- `F9` log thêm danh sách `Session:` (SteamID, tên Steam, `(local)`) và
  `Synced:` (vị trí nhận được, bao nhiêu giây trước).
- `warp.rs`: tách `warp_to(fns, &Spot)` dùng chung cho `F8` và `F10`;
  `SavedSpot` → `Spot` (pub, dùng chung với `sync`).

Chưa chạy thử. Chưa biết Seamless Co-op có dùng `CSSessionManager.players`
của game hay tự quản lý phiên riêng - `F9` sẽ cho biết.

**Quyết định thiết kế (2026-09-24):** gửi vị trí định kỳ 500 ms cho mọi người
chỉ là **cách tạm cho giai đoạn test** (đơn giản, luôn có sẵn dữ liệu để
khảo sát). Bản thật phải làm kiểu **hỏi khi cần**: `/teleport X` gửi gói
`WHERE` tới SteamID của X, máy X trả `HERE` với vị trí lúc đó, rồi mới warp;
quá ~2 s không trả lời thì báo lỗi. Lý do: không phát vị trí liên tục khi
chẳng ai teleport, vị trí luôn mới nhất, và không lộ vị trí của người chơi cho
mọi người cài mod (sau có thể thêm tuỳ chọn chặn người khác teleport tới mình).

## Test Seamless Co-op lần 3: teleport tới đồng đội ở xa - THÀNH CÔNG (2026-09-24)

2 instance (host `AnLe -2` / Steam `ALRIP`, join `ALR` / Steam `BinPham`):
- `CSSessionManager.players` **có dùng được trong Seamless Co-op** - cả 2 bên
  liệt kê đúng người trong phiên, đánh dấu đúng `(local)`.
- Đồng bộ chạy 2 chiều; vị trí nhận được chỉ cũ 0.0-0.4 s.
- Khi ở xa (host `m60_42_37`, join `m60_46_40`): `Synced:` trên máy host khớp
  đúng tọa độ người join tự log.
- Host bấm `F10` → màn loading → xuất hiện cạnh `ALR` ở `m60_46_40`. **Phiên
  co-op vẫn giữ nguyên sau khi warp** - ghi thẳng `warp_requested` (không gọi
  `sub_1405F89C0`) không làm rớt phiên, đúng như lo ngại ban đầu cần tránh.

Toàn bộ luồng "lấy vị trí đồng đội → warp qua loading → đứng cạnh họ" đã
chứng minh chạy được. Còn lại: đổi sang kiểu hỏi khi cần (WHERE/HERE), lọc
chỉ cho teleport tới đồng đội, và nối vào lệnh chat của SoulsChat.

Lưu ý từ lần test này: `CSSessionManager` còn liệt kê cả người lạ
(`cyan desan`, `TAMARI`, `wingobear`) và `player_chr_set` có cả kẻ xâm nhập
(`Fire Knight`, `chr_type Duelist`) - do chưa đổi `cooppassword` của Seamless
Co-op, người lạ vào được phiên. Bản test broadcast đã gửi vị trí cho cả họ
(không cài mod nên gói bị bỏ qua) - thêm 1 lý do cho kiểu hỏi khi cần, và cho
việc lọc: chỉ đồng đội (host + người co-op), không phải invader/người lạ.

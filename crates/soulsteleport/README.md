# SoulsTeleport

Mod độc lập cho **Seamless Co-op**: bấm `MenuKey` (Tab) mở menu ImGui liệt
kê mọi người trong phiên kèm vai trò, bấm 1 người để dịch chuyển (qua màn
hình loading) tới đúng chỗ họ đứng, dù ở xa tới đâu. Vị trí được hỏi trực
tiếp người đó qua Steam P2P (`WHERE` → `HERE`), nên người được tới cũng phải
cài mod. Tên hiển thị trên Nexus: "Souls Teleport" (chưa đăng).

Ban đầu (2026-09-24) định làm addon cho SoulsChat (lệnh `/teleport`) - đã bỏ
hướng đó ngày 2026-09-25, xem các mục bên dưới.

Cấu hình (`SoulsTeleport.ini`):
- `MenuKey` (mặc định `0x09` = Tab): mở/đóng menu chọn người để dịch chuyển.
- `[Menu]` `MenuScale` (mặc định `1.0`): phóng to/thu nhỏ cả menu, nhân thêm
  trên phần co giãn tự động theo độ phân giải.
- `[Menu]` `MenuX`/`MenuY`/`MenuWidth`/`MenuHeight`: vị trí + kích thước menu,
  mod tự ghi khi người chơi kéo/đổi cỡ menu (-1 = mặc định).
- `LogFile`: ghi `SoulsTeleport.log` cạnh DLL.

(Các phím thử nghiệm `SaveKey`/`WarpKey`/`ListPlayersKey`/`WarpToPartnerKey`
và `ReloadKey` nhắc tới ở các mục bên dưới đã bỏ - xem mục 2026-09-25.)

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

## Hỏi khi cần (WHERE/HERE) thay cho broadcast + chỉ đồng đội + chọn theo tên (2026-09-24)

Làm đúng quyết định thiết kế ở trên, sau khi test lần 3 xác nhận cả luồng:

- `src/sync.rs` (broadcast 500 ms) → xoá, thay bằng `src/net.rs`:
  - `F10` gửi `WHERE` (16 byte: magic `STW1`, SteamID người hỏi, request id)
    tới **đúng 1** đồng đội; chờ tối đa 10 s (bản nháp đầu 2 s - quá ngắn, đồng
    đội có thể đang loading/hồi sinh và chỉ trả lời khi vào lại world).
  - Máy được hỏi trả `HERE` (70 byte: magic `STH1`, SteamID, request id,
    block, x/y/z/yaw trong block, tên nhân vật) - chỉ khi người hỏi là đồng
    đội. Nếu `WHERE` tới lúc mình đang loading/hồi sinh thì giữ lại và trả
    lời ngay khi vào lại world (trong 10 s); quá hạn thì bỏ, bên hỏi báo
    "... did not respond".
  - Gói `HERE` trễ / của request cũ bị bỏ qua (so SteamID + request id).
  - `AcceptSessionWithUser` cho mọi đồng đội mỗi 2 s - nếu không, `WHERE`
    đầu tiên từ 1 người mình chưa từng gửi gì sẽ bị Steam bỏ.
  - Danh sách phiên chỉ được đọc khi cần (đến hạn accept, hoặc có gói tới),
    không phải mỗi frame.
- `src/party.rs`: ghép `CSSessionManager.players` (SteamID, tên Steam) với
  `player_chr_set` (tên nhân vật, `chr_type`) qua
  `PlayerIns.session_manager_player_entry.steam_id`. **Đồng đội** = người khác
  mình, có `PlayerIns`, `chr_type` không thuộc nhóm thù địch (`Duelist`,
  `BloodyFinger`, `Recusant`, `FesteringBloodyFinger`, `BloodyFingerNpc`,
  `RecusantNpc`). Người không có `PlayerIns` → không tin (chưa rõ vai trò).
  Áp dụng cả 2 chiều: không teleport tới, và không trả lời `WHERE` của họ.
- Ini key mới `TeleportTarget`: 1 phần tên nhân vật hoặc tên Steam, không
  phân biệt hoa thường; trùng khít tên nhân vật được ưu tiên; để trống = đồng
  đội duy nhất (nhiều người thì banner liệt kê tên để chọn). Đây là phần
  "chọn theo tên" sẽ dùng lại cho `/teleport <tên>`.
- `F9` giờ log từng người trong phiên kèm tên nhân vật, `chr_type`, và
  `[partner]` nếu được tính là đồng đội.

Chưa chạy thử.

## Test Seamless Co-op lần 4: WHERE/HERE 2 chiều; nhớ vai trò đồng đội (2026-09-24)

- **Host → join (xa): thành công.** `WHERE` → `HERE` sau **33 ms** → warp tới
  block `0x3C2A2500` của người join.
- **Join → host: lần bấm đầu bị từ chối nhầm.** Host log
  `Ignored WHERE from 'BinPham': not a co-op partner (None)`. Người dùng xác
  nhận lúc đó **host đang ở màn hình loading** (cố ý bấm thử khi đang load).
  Trong lúc load, `player_chr_set` phía host tạm thời không có `PlayerIns` của
  người join → vai trò `None` → luật "chưa rõ vai trò = không tin" loại ngay
  ở bước kiểm tra đồng đội, **trước** bước "chưa ở trong world thì giữ lại trả
  lời sau" - nên yêu cầu bị bỏ thay vì được hoãn. (Ban đầu đọc log tưởng là
  do host ở xa - sai, là do đang loading.)
- Lúc 17:40:35 cả 2 bản cùng gửi `WHERE` trong 1 giây và cùng warp tới chỗ
  của nhau (đổi chỗ). Người dùng xác nhận chỉ bấm `F10` ở **1** cửa sổ →
  phím vẫn lọt sang bản kia (xem mục ngay dưới).

Sửa trong `src/party.rs` (cũng đúng cho trường hợp ở xa, nếu game bỏ
`PlayerIns` của người ở xa): `KNOWN_ROLES` nhớ `chr_type` cuối cùng thấy được
của từng SteamID (cập nhật mỗi lần đọc danh sách phiên, tức ít nhất 2 s/lần
qua bước accept). Người không còn `PlayerIns` thì dùng vai trò đã nhớ; chỉ
người **chưa từng thấy** mới bị coi là chưa rõ. Đồng đội Seamless Co-op luôn
xuất hiện cạnh host khi vào phiên, nên vai trò được ghi nhận ngay từ đầu.
Với trường hợp đang loading: kiểm tra đồng đội giờ qua được (vai trò đã
nhớ), rồi mới tới bước hoãn → host trả `HERE` ngay khi load xong.

## Phím lọt sang cả 2 instance: bỏ `GetKeyState` của fromsoftware-rs (2026-09-24)

Bấm `F10` ở cửa sổ thứ 2, **cả 2** bản game cùng dịch chuyển. Nhận định trước
đó (mục "Test Seamless Co-op lần 2") rằng `eldenring::util::input::is_key_pressed`
chỉ nhận phím gửi tới cửa sổ của chính process là **sai**: hàm đó dùng
`GetKeyState` - trạng thái phím theo hàng đợi thông điệp của luồng gọi - mà
mod gọi nó từ task `FrameBegin` của game, luồng đó không có hàng đợi input
riêng của cửa sổ, nên thấy trạng thái phím chung của cả desktop. 2 instance
(1 trong Sandboxie) trên cùng desktop cùng thấy 1 lần bấm. Không phải lỗi của
Sandboxie.

Sửa: quay lại `common::input::is_key_pressed` - bản đã sửa hôm nay (xem
WeightMultiplier README): kiểm tra cửa sổ foreground thuộc chính process
trước khi đọc phím, instance không được focus không đọc phím chút nào. Gọi
được từ bất kỳ luồng nào (`GetAsyncKeyState`), kể cả task của game.

Cùng lỗi này nhiều khả năng có ở `ReloadKey` của mọi mod dùng
`common::reload`/AutoRegen/RuneMultiplier (đều gọi thẳng
`eldenring::util::input::is_key_pressed`) - chưa sửa, đề xuất riêng.

## Lệnh `/teleport <tên>` trong SoulsChat v0.1.2 (2026-09-24)

Người dùng cập nhật SoulsChat lên **v0.1.2** (bản phân tích trước là v0.0.9 -
build lại nên mọi địa chỉ đổi hết; vẫn nén UPX, vẫn **không** có API plugin:
chỉ export `DllMain` + 2 hàm hot-patch). Phân tích lại bằng IDA (bản đã
giải nén):

- Hàm UI/xử lý lệnh: `sub_18000CA1B`. Khi bấm Enter (nhãn `LABEL_159`): lấy
  ô nhập (`app+24` con trỏ, `app+32` độ dài), `str::trim` (`sub_18009B504`),
  chép ra `String`. **Rỗng → chỉ xoá ô nhập**, không in, không gửi. Bắt đầu
  bằng `/` hoặc `\` → chuỗi so sánh tên lệnh (`sub_18001B864`); không khớp →
  in lỗi qua `sub_180010771` (hàm đưa tin "System" vào khung chat). Còn lại
  → gửi như tin chat.
- `sub_18001772D` chỉ là gợi ý tên người chơi cho `/warpto` + `/sendrunes`.

Cách hook (`src/soulschat.rs`): hook `sub_18009B504` (trim dùng chung ở 22
chỗ), nhưng chỉ xử lý khi địa chỉ trả về đúng là chỗ gọi lúc Enter
(`call` ở `0x18000DFF9`, trả về `0x18000DFFE`). Nếu dòng là
`/teleport ...` (hay `\teleport ...`) → ghi lại tên người cần tới, trả về
chuỗi rỗng → SoulsChat tự xoá ô nhập như chưa gõ gì. Mọi lần gọi khác chạy
thẳng hàm gốc.
- 12 byte đầu của trim là đúng 8 lệnh `push` (không phụ thuộc vị trí) → patch
  `mov rax, imm64; jmp rax` 12 byte không cắt ngang lệnh nào; 12 byte gốc
  chép sang buffer + nhảy về `trim+12`.
- Cả 2 mẫu AOB (thân trim, chỗ gọi lúc Enter) đều khớp **đúng 1 chỗ** trong
  v0.1.2, quét trên bộ nhớ đã giải nén (`UPX0` có cờ thực thi) qua hàm mới
  `common::memscan::find_pattern_in_named_module`. SoulsChat đổi code →
  AOB hụt → `/teleport` tắt (log), phần còn lại (`F10`) vẫn chạy.
- Yêu cầu từ luồng UI của SoulsChat được chuyển sang task của game qua 1
  `Mutex`, rồi chạy đúng luồng `WHERE`/`HERE` như `F10`. Phản hồi hiện bằng
  banner của game (chưa in vào khung chat SoulsChat - gọi hàm in của SoulsChat
  từ luồng game có thể đụng luồng UI của nó).

Chưa chạy thử.

## Bỏ hướng addon SoulsChat → mod độc lập có menu ImGui riêng (2026-09-25)

Hook lệnh `/teleport` vào SoulsChat (mục 2026-09-24 ở trên) chạy được lúc
Enter, nhưng muốn có gợi ý lệnh/tên như `/warpto` thì phải can thiệp sâu hơn
vào dữ liệu và code nội bộ của mod người khác - dừng hướng đó. Đổi sang mod
độc lập với GUI riêng. Tên giữ nguyên "Souls Teleport".

- Xoá `src/soulschat.rs`; hoàn tác 2 thay đổi trong `common` chỉ dùng cho nó
  (`memscan::find_pattern_in_named_module`, `codepatch::alloc_near` public).
- Tham khảo cách làm GUI của 2 mod khác: QuestPath (Dear ImGui 1.90.9 +
  MinHook, hook DXGI/`Present`/`ExecuteCommandLists` của D3D12, có đường dự
  phòng cửa sổ trong suốt riêng) và ZB_BossReviver (không hook DirectX: cửa sổ
  Win32 riêng nổi trên game, vẽ bằng GDI). Chọn **ImGui qua hudhook** (0.9.3,
  chỉ feature `dx12`) - giống SoulsChat, chạy mọi chế độ màn hình.
- `src/ui.rs`: `MenuKey` (lúc đó mặc định `F10`, thay cho `WarpToPartnerKey` +
  `TeleportTarget` - đã bỏ) mở/đóng cửa sổ "Souls Teleport" liệt kê đồng đội
  (`party::Member::is_partner`), mỗi người 1 nút (tooltip: tên Steam); bấm →
  `WHERE`/`HERE` như cũ → warp, menu tự đóng. Trạng thái ("Locating...",
  "... did not respond.") hiện ngay trong menu; nút bị khoá khi đang chờ.
- Luồng: menu vẽ trên luồng render của game (trong `Present` đã hook); mọi
  thứ đụng tới phiên/Steam/warp vẫn ở task `FrameBegin`. Hai bên chỉ chia sẻ
  1 snapshot danh sách đồng đội (task làm mới mỗi 500 ms khi menu mở) + trạng
  thái, và SteamID người được bấm.
- Khi menu mở: `MessageFilter::InputAll` (chặn phím/chuột/raw input tới cửa
  sổ game) + ImGui tự vẽ con trỏ (game ẩn con trỏ hệ thống). Chưa biết có
  chặn hết input game không (game có thể đọc DirectInput/XInput riêng).
- Font: `segoeui.ttf` (hoặc arial/tahoma) với dải Latin + Latin mở rộng +
  tiếng Việt; chưa có chữ CJK.
- `steam.rs`: gửi **reliable** (cờ `8 | 32`, như SoulsChat dùng `8`) thay vì
  unreliable - mỗi lần teleport chỉ 1 `WHERE` + 1 `HERE`, rớt gói là phải chờ
  hết 10 s.
- Bỏ `party::find_target`/`Target` (chọn theo tên) - menu chọn trực tiếp.

Chưa chạy thử. Rủi ro chính cần test: chạy chung với SoulsChat (cả 2 đều
hook `Present` qua hudhook/MinHook).

## Rút gọn cấu hình: chỉ còn `MenuKey` + `LogFile` (2026-09-25)

Theo yêu cầu người dùng, bỏ mọi phím còn sót từ giai đoạn thử nghiệm:
- Bỏ `SaveKey` (`F7`), `WarpKey` (`F8`) - lưu/warp về 1 chỗ đã lưu, chỉ dùng để
  kiểm chứng cơ chế warp - cùng code `save_current_spot`/`warp_to_saved`/
  `current_spot`/`SAVED`.
- Bỏ `ListPlayersKey` (`F9`) và `list_players` (log nghiên cứu - đã xong việc).
- Bỏ `ReloadKey` và luồng `common::reload`: với 2 key còn lại không còn gì
  đáng reload lúc đang chơi.
- `MenuKey` mặc định đổi từ `F10` sang **Tab** (`0x09` - `parse_virtual_key`
  không hiểu tên "TAB", nên ghi mã hex). Ini có link bảng mã phím để người
  dùng tự đổi. Lưu ý: bản mặc định của SoulsChat cũng gán Tab cho
  `MatchmakingUIKey` (giữ để xem bảng lobby) - ai dùng cả 2 mod với mặc định
  sẽ bị trùng phím.

Ini cũ của người dùng: các key đã bỏ tự được `config::migrate` chuyển xuống
block `[Legacy]` lần chạy đầu, không mất gì.

## Menu tự co giãn theo độ phân giải game (2026-09-25)

Trước đó chữ cố định 22 px, cửa sổ 420×360 px → nhỏ xíu ở 4K, to ở 720p. Giờ
`ui.rs` lấy chuẩn là cửa sổ game 1080p và nhân mọi thứ theo
`chiều cao cửa sổ game / 1080` (kẹp trong 0.6-3.0), đọc từ
`io.display_size` mỗi frame - đổi độ phân giải lúc đang chơi cũng theo kịp:
- Font nạp 1 lần ở 40 px (đủ nét khi phóng lên 4K), hiển thị ở
  `22 px × scale` qua `font_global_scale`.
- Style (padding, khoảng cách, bo góc...) `scale_all_sizes(scale)` từ 1 bản
  style gốc lưu lúc khởi tạo - không áp chồng lên bản đã scale, tránh bị
  nhân dồn.
- Khi scale đổi, lần mở menu kế tiếp đặt lại kích thước/vị trí cửa sổ 1 lần
  (`Condition::Always`), sau đó người chơi vẫn kéo/đổi cỡ tự do.

Không thêm key ini (giữ đúng 2 key `MenuKey`/`LogFile`). Chưa chạy thử.

## Theme riêng cho menu (2026-09-25)

Người dùng chạy thử: menu chạy nhưng trông quá thô (theme mặc định của ImGui -
nền đen, viền xanh dương). Thêm `apply_theme` trong `ui.rs`, áp 1 lần lúc khởi
tạo, **trước** khi lưu `base_style` (nên co giãn theo độ phân giải vẫn đúng):
- Màu theo tông menu của game: nền đen ấm hơi trong suốt, chữ kem, điểm nhấn
  vàng đồng (viền, separator, tiêu đề mục, dòng trạng thái); nút nâu tối, sáng
  dần sang đồng khi rê chuột/bấm.
- Bo góc (cửa sổ 10, nút 6), padding rộng hơn, tiêu đề cửa sổ căn giữa.
- Bố cục: tiêu đề "Co-op partners (N)", các dòng trống ("Not in a co-op
  session."...) căn giữa, dòng trạng thái đổi màu đỏ nhạt khi lỗi
  ("did not respond", "no longer available"...).

Chưa chạy thử.

## Hiện mọi người trong phiên + vai trò; tiêu đề là tên mod (2026-09-25)

Theo yêu cầu người dùng:
- **Bỏ lọc kẻ xâm nhập.** Trước đó `party::Member::is_partner` loại các
  `chr_type` thù địch (và cả người chưa từng thấy nhân vật) khỏi danh sách và
  không trả lời `WHERE` của họ. Giờ `is_teleport_target` = mọi người trong
  `CSSessionManager.players` trừ chính mình - cả 2 chiều (menu liệt kê, và
  máy mình trả `HERE` cho bất kỳ ai trong phiên hỏi). `is_hostile` bỏ.
  Hệ quả cần biết: người lạ/kẻ xâm nhập cài mod này cũng hỏi được vị trí của
  mình - đổi `cooppassword` của Seamless Co-op để tránh người lạ vào phiên.
- **Vai trò** hiện bên phải mỗi dòng (`Member::role`): `Host` (từ
  `SessionManagerPlayerEntry.is_host`), rồi theo `chr_type`: `Co-op`
  (Seamless Co-op báo mọi người là `Local`), `Cooperator`, `Invader`,
  `Hunter`, `Bloody Finger`, `Recusant`, `Arena`, `Other`; `Unknown` nếu chưa
  từng thấy nhân vật. Màu: Host vàng, thù địch đỏ nhạt, Unknown/Other xám.
  `KNOWN_ROLES` giờ chỉ để hiện vai trò khi `PlayerIns` tạm biến mất.
- **Tiêu đề:** bỏ thanh tiêu đề mặc định của ImGui; đầu menu là
  "Souls Teleport" căn giữa màu vàng + nút `X` đóng ở góc phải; dưới là
  "Players (N)".

Chưa chạy thử.

## Nhúng font Noto Sans thay cho đường dẫn font cố định (2026-09-25)

`FONT_CANDIDATES` cũ ghi cứng `C:\Windows\Fonts\segoeui.ttf`/`arial`/`tahoma`
→ hỏng nếu Windows cài ổ khác, máy thiếu font, hay chạy **Proton/Steam Deck**
(`C:\Windows\Fonts` trong Wine thường không có font Windows). Không crash
(rơi về font mặc định ImGui) nhưng font đó chỉ có ASCII → tên có dấu thành `?`.

Giờ:
- **Font chính nhúng trong DLL**: `assets/NotoSans-Subset.ttf` (Noto Sans
  Regular từ `notofonts/notofonts.github.io`, cắt bằng `pyftsubset` còn dải
  U+0020-024F, U+0300-036F, U+1E00-1EFF, U+2000-206F, U+20AC - 1007 glyph,
  91 KB; DLL 797 → 881 KB). Giấy phép SIL OFL 1.1 (`assets/NotoSans-OFL.txt`,
  không có Reserved Font Name nên giữ nguyên tên khi cắt). Khi đăng Nexus cần
  ghi công font + kèm file giấy phép.
- **CJK tuỳ chọn**: tìm thư mục Windows thật bằng `GetWindowsDirectoryW`, lấy
  font đầu tiên có trong `msyh`/`msjh`/`meiryo`/`msgothic`/`simsun`, ghép vào
  (merge) với dải `chinese_simplified_common` (Hán thông dụng + kana). Không
  có thì bỏ qua. **Chưa hỗ trợ tiếng Hàn**: dải Hangul của ImGui là toàn bộ
  11k âm tiết, quá lớn cho atlas ở cỡ raster 40 px.
- Không còn `Box::leak`: `FontAtlas::add_font` tự chép dữ liệu font vào atlas.

Chưa chạy thử.

## Sửa: menu không có chuột; warp tới người đang loading bị văng ra title (2026-09-25)

Test trong game (người dùng):

1. **Mở menu nhưng không có chuột để bấm.** Game ẩn + khoá con trỏ và đọc
   chuột qua raw input, nên đường `WM_MOUSEMOVE` của hudhook không có gì
   dùng được. Sửa (`ui.rs` `feed_mouse`, chạy trong `before_render` khi menu
   mở và cửa sổ game đang được chọn): `ClipCursor(NULL)` mỗi frame để thả con
   trỏ, tự đọc `GetCursorPos` → `ScreenToClient`, quy đổi từ pixel client sang
   kích thước swapchain (`display_size`) - cũng xử lý luôn trường hợp Windows
   scale 150% (game không DPI-aware nên 2 kích thước này có thể khác nhau) -
   rồi đưa vào ImGui; nút trái/phải đọc `GetAsyncKeyState`, chỉ gửi khi đổi
   trạng thái, và nhả ra khi đóng menu. Chưa biết game có tự kéo con trỏ về
   giữa màn hình mỗi frame không (nếu có thì phải hook `SetCursorPos`).
2. **Warp tới người đang ở màn hình loading → người warp bị văng ra màn hình
   chính** (thử 2 lần, cả 2 chiều host/join; warp tới người đã load xong thì
   bình thường). Nguyên nhân: lúc đang load, `WorldChrMan` vẫn có main player
   cũ nên máy được hỏi trả `HERE` ngay với vị trí ở map đang rời đi. Sửa
   (`net.rs`): chỉ trả lời khi đã "ở trong world" liên tục ≥ 2 s
   (`SETTLE_TIME`), với "ở trong world" = có main player **và**
   `GameMan.warp_requested` = false (không có move-map đang chờ - hàm mới
   `warp::warp_pending`) **và** `CSSessionManager.protocol_state` không phải
   pha (tải) lại (`WaitInitData`/`WaitReloadWait`/`WaitReload`/`WaitReload2`/
   `WaitReentryToMap`). Chưa đủ điều kiện → giữ yêu cầu lại như cũ, trả lời
   ngay khi đủ (trong `REPLY_TIMEOUT` 10 s). Bên warp cũng không warp khi chính
   mình đang có move-map chờ.

Chưa chạy thử. Chưa xác minh Seamless Co-op có cập nhật `protocol_state`
không - nếu không, `warp_requested` + 2 s vẫn là lớp chặn chính.

## Chuột lần 2 (học QuestPath), giữ dòng trạng thái, bỏ toast (2026-09-25)

Test lần 2 của người dùng:

1. **Bỏ toast "Teleporting to <player>"** (banner hệ thống của game) - trạng
   thái đã hiện ngay trong menu.
2. **Dòng trạng thái vàng bị xoá khi đóng/mở lại menu** (`toggle` gọi
   `set_status("")` mỗi lần mở) - trong khi nút vẫn khoá đúng vì request đang
   chờ. Giờ `toggle` chỉ đảo `MENU_OPEN`, dòng trạng thái giữ nguyên.
3. **Vẫn không có chuột.** Tìm ra 2 nguyên nhân:
   - hudhook tự xử lý raw input chuột của game (dạng dịch chuyển tương đối)
     thành `mouse_pos + delta`, cộng lên giá trị "không có vị trí" của ImGui
     (-FLT_MAX) → vị trí không bao giờ hợp lệ → ImGui không vẽ con trỏ; các sự
     kiện này còn đè lên vị trí tuyệt đối `feed_mouse` đưa vào. Sửa:
     `before_wnd_proc` trả `Break` cho `WM_INPUT` + mọi `WM_MOUSE*` khi menu mở
     (chỉ bỏ phía ImGui; phía game vẫn do `message_filter` chặn), chuột chỉ
     lấy từ `feed_mouse`. Phím vẫn đi qua ImGui như cũ.
   - Game liên tục khoá lại (`ClipCursor`) và kéo con trỏ về giữa
     (`SetCursorPos`) để xoay camera. QuestPath (phân tích import + chuỗi:
     "cursor-unpin" qua MinHook, cùng `RegisterRawInputDevices`,
     `DirectInput8Create` hook để chặn tay cầm) hook 2 hàm này. Làm theo:
     `install_cursor_hooks` dùng MinHook có sẵn trong hudhook (`MhHook`), hook
     `user32!SetCursorPos` (menu mở → không di chuyển, giả vờ thành công) và
     `user32!ClipCursor` (menu mở → luôn truyền `NULL`).

Chưa chạy thử. Chưa làm phần chặn tay cầm (DirectInput/XInput) như QuestPath.

## Chặn điều khiển game khi menu mở (2026-09-25)

Chuột đã hiện (bản trước), nhưng nhân vật/camera vẫn phản ứng khi menu mở -
`message_filter` không có tác dụng vì game **không đọc input qua tin nhắn cửa
sổ**: import của `eldenring.exe` chỉ có `DINPUT8!DirectInput8Create` (bàn
phím, chuột, tay cầm DirectInput) và `XINPUT1_4` theo ordinal (tay cầm Xbox).
QuestPath cũng hook DirectInput (log của nó: "dinput8 vtable ... pad may leak
while menu open").

`src/input_block.rs` (MinHook có sẵn trong hudhook; cài ngay sau DX12 hook):
- Tạo 1 `IDirectInput8` + thiết bị bàn phím tạm của riêng mình để đọc vtable,
  hook 2 hàm nó trỏ tới - dinput8 dùng chung 1 cài đặt thiết bị (1 vtable mỗi
  bản A/W) cho mọi loại thiết bị, nên trúng luôn thiết bị của game. Hook cả
  bản W lẫn A nếu khác nhau.
  - `GetDeviceState` (slot 9): menu mở → vẫn gọi hàm gốc, rồi xoá về 0 nếu là
    định dạng bàn phím (256 byte) hoặc chuột (`DIMOUSESTATE`/`2`). **Không**
    xoá định dạng joystick: `DIJOYSTATE` toàn 0 = cần gạt bị đẩy hết cỡ, không
    phải ở giữa.
  - `GetDeviceData` (slot 10): menu mở → báo 0 sự kiện.
- `XInputGetState` (ordinal 2) / `XInputGetStateEx` (100) của `xinput1_4`:
  menu mở → xoá phần gamepad về 0 (với XInput, cần gạt ở giữa = 0), giữ
  `dwPacketNumber`.
- Vẫn gọi hàm gốc trước khi xoá để bộ đệm thiết bị của game được rút cạn -
  đóng menu không bị "dồn" lại các phím đã bấm lúc mở.

Phím của chính mod (`MenuKey`) và chuột của menu đọc qua `GetAsyncKeyState`/
`GetCursorPos`, không đi qua các hàm bị hook → vẫn hoạt động.

Chưa chạy thử. Chưa hỗ trợ điều khiển menu bằng tay cầm.

## Dòng trạng thái tự xoá; chỉ chặn chuột; chẩn đoán mất con trỏ ở cửa sổ nhỏ (2026-09-25)

Test tiếp của người dùng:

1. **Dòng chữ vàng có lúc bị treo.** Log cho thấy 1 nguyên nhân rõ: sau warp
   thành công, "Teleporting to X..." không bao giờ bị xoá. Ngoài ra lúc
   12:35:58 người dùng bấm teleport khi lần warp trước chưa load xong →
   `warp_to` từ chối (có move-map đang chờ) và chỉ để lại 1 dòng lỗi. Sửa:
   `Shared.status` giờ đi kèm thời điểm đặt (`status_set`, chỉ ghi qua
   `set_status`), tự xoá sau `STATUS_TTL` = 15 s - trừ khi đang có request
   chờ (`busy`). Thông báo lỗi đó đổi thành "loading or not in the world".
2. **Không thấy con trỏ ở chế độ cửa sổ 1680×945 trở xuống.** Chưa suy ra
   được từ code (quy đổi client → swapchain đúng ở cả 2 trường hợp DPI-aware
   và không). Thêm log chẩn đoán: 3 frame đầu mỗi lần mở menu ghi vị trí con
   trỏ trong client, kích thước client, `display_size`, vị trí sau quy đổi,
   `io.mouse_pos` của ImGui và `mouse_draw_cursor` (hoặc "not the foreground
   window"). Gợi ý: 1680×945 là cỡ cửa sổ lớn nhất vừa màn hình 2560×1440 ở
   scale 150% (1707×960 logic) - có thể liên quan tới cách game chuyển giữa
   cửa sổ thường và không viền.
3. **Chỉ chặn chuột, vẫn cho di chuyển.** Theo yêu cầu: `input_block.rs` giờ
   chỉ xoá trạng thái định dạng chuột (`DIMOUSESTATE`/`2`) và chỉ bỏ sự kiện
   `GetDeviceData` của thiết bị đã được nhận ra là chuột (thiết bị gọi
   `GetDeviceState` với định dạng chuột). Bàn phím và tay cầm vẫn tới game;
   hook XInput bỏ. Nút chuột cũng bị chặn → bấm trong menu không làm nhân vật
   tấn công.

Chưa chạy thử.

## Tìm ra lỗi mất con trỏ ở cửa sổ nhỏ: `ScaleAllSizes` làm tròn cỡ con trỏ (2026-09-25)

Log chẩn đoán ở 1680×945: `cursor client (650, 541) client 1680x945 display
1680x945 -> mouse (650, 541), imgui mouse_pos (650, 541), draw_cursor true` -
vị trí đúng hoàn toàn, khớp việc người dùng rê chuột mù vẫn bấm trúng. Vậy lỗi
ở phần **vẽ** con trỏ, không phải vị trí.

Nguyên nhân: `ImGuiStyle::ScaleAllSizes` có
`MouseCursorScale = ImFloor(MouseCursorScale * scale_factor)` (imgui.cpp,
imgui-sys 0.12). Scale theo độ phân giải: 1080 px → 1.0 → 1; 1440 px → 1.48 →
1 (vẫn thấy); 945 px → 0.875 → **0** → con trỏ vẽ với cỡ 0. Sửa: sau
`scale_all_sizes`, đặt lại `mouse_cursor_scale = gốc × scale` không làm tròn.

Log `Menu diag` đã gỡ sau khi người dùng xác nhận hết lỗi (2026-09-25).

## Chuẩn bị phát hành 1.0.0 (2026-09-25)

Người dùng xác nhận mọi thứ đã ổn (con trỏ hiện ở mọi cỡ cửa sổ). Việc làm để
chuẩn bị phát hành:
- Gỡ log chẩn đoán `Menu diag`.
- `Cargo.toml` version `0.1.0` → `1.0.0` (Nexus chưa có trang mod này, nên
  không có changelog thật để đối chiếu - bản đầu tiên).
- `DESCRIPTION.bbcode` mới theo khung của các mod khác: giới thiệu, Features,
  Requirements (Seamless Co-op - Nexus mod 510, đã kiểm tra qua API; mọi
  người muốn teleport tới đều phải cài mod), Usage, Installation, Credits (ghi
  công Noto Sans + giấy phép OFL, hudhook, Dear ImGui), Changelog 1.0.0.
- `scripts/build-mod.ps1`: khi `-Zip`, gói thêm mọi `*.txt` trong
  `crates/<crate>/assets/` - để `NotoSans-OFL.txt` luôn đi kèm font nhúng như
  OFL yêu cầu. Các mod khác không có thư mục `assets/` nên zip không đổi (đã
  thử với AutoRegen).

## Lưu vị trí + kích thước menu vào ini (2026-09-25)

Yêu cầu người dùng: menu kéo/đổi cỡ ở đâu thì lần chơi sau vẫn ở đó.
- `common::config::set_values(ini_path, &[(key, value)])` - **hàm mới trong
  `common`** (trước đó cả workspace không có hàm nào ghi giá trị vào ini,
  `migrate` chỉ ghi lại cả file từ template). Sửa đúng dòng `key=value` tại
  chỗ, giữ nguyên comment/section/các dòng khác, key chưa có thì nối cuối
  file; cập nhật luôn map cấu hình trong bộ nhớ. Có unit test.
- 4 key mới trong mục `[Menu]`: `MenuX`, `MenuY`, `MenuWidth`, `MenuHeight`,
  mặc định `-1` (= bố cục mặc định). Lưu theo **đơn vị 1080p** (chia cho UI
  scale) nên đổi độ phân giải menu vẫn nằm đúng tỷ lệ chỗ cũ, không văng ra
  ngoài màn hình.
- `ui.rs`: mỗi frame đọc vị trí/kích thước thật của cửa sổ ImGui; khi khác
  bản đã lưu ≥ 1 đơn vị **và** người chơi đã nhả chuột trái thì ghi ini 1 lần
  (không ghi mỗi frame trong lúc kéo). Mở menu lần đầu không ghi gì - ini giữ
  `-1` tới khi người chơi thật sự di chuyển menu.
- Ini cũ của người dùng tự được `migrate` thêm mục `[Menu]` lần chạy đầu.

Về các câu hỏi trước khi phát hành (người dùng trả lời): đã chạy chung với
QuestPath ổn; đã thử cả 3 chế độ màn hình ổn; danh sách dài hơn menu thì
ImGui tự có thanh cuộn (con lăn chuột); việc hiện mọi người kể cả kẻ xâm nhập
(kèm vai trò) là đúng ý. Chưa có điều kiện thử phiên từ 3 người trở lên.

Chưa chạy thử phần lưu vị trí.

## Nút "Reset" bố cục menu (2026-09-25)

Nút nhỏ **Reset** ở góc trái đầu menu (tooltip "Reset the menu's position
and size"): ghi `-1` cho cả 4 key `[Menu]` qua `config::set_values`, và đặt
lại vị trí/kích thước mặc định ngay frame sau (`relayout` →
`Condition::Always`). Mốc so sánh đã lưu/đã thấy được xoá, để bố cục mặc định
vừa áp không bị ghi ngược lại vào ini thành số - ini giữ `-1`. Tiêu đề
"Souls Teleport" vẫn căn giữa, nút `X` vẫn ở góc phải.

Chưa chạy thử.

## Key `MenuScale` (2026-09-25)

`[Menu] MenuScale` (mặc định `1.0`, kẹp 0.5-3.0): hệ số người chơi tự chọn,
**nhân thêm** trên scale tự động theo độ phân giải. Đọc 1 lần lúc khởi động
(không có `ReloadKey` - đổi xong phải khởi động lại game, ini có ghi chú).
- Chữ, style (padding, bo góc, cỡ con trỏ...) và **kích thước** menu theo
  scale tổng = độ phân giải × `MenuScale`. **Vị trí** menu chỉ theo độ phân
  giải - tách riêng `pos_scale` - để đổi `MenuScale` không làm menu trôi khỏi
  chỗ người chơi đã đặt. `MenuWidth`/`MenuHeight` lưu theo scale tổng, nên
  cùng 1 giá trị lưu vẫn vừa khít chữ ở `MenuScale` mới.
- Font raster `40 × MenuScale` (kẹp 40-64 px) để chữ không bị mờ khi phóng
  to; không cho lên cao hơn vì atlas còn chứa cả chữ CJK.
- Chốt chặn: nếu `before_render` chạy trước `initialize` (lúc `menu_scale`
  còn 0) thì coi như 1.0.

Chưa chạy thử.

## Review trước phát hành: chống giả gói tin + dọn dẹp (2026-09-25)

Review toàn bộ crate trước khi đăng. Sửa:
- **Chống giả `HERE`/`WHERE`.** Trước đó người gửi chỉ lấy từ SteamID ghi
  trong nội dung gói, và request id là 1, 2, 3... → 1 người trong phiên dùng
  bản mod bị sửa có thể gửi `HERE` giả đúng lúc mình đang chờ và ép warp tới
  block/tọa độ rác (crash, rơi khỏi map, kẹt). Giờ:
  - `steam.rs` đọc thêm `m_identityPeer` của `SteamNetworkingMessage_t`
    (offset 16, sau `m_pData`/`m_cbSize`/`m_conn` - layout không đổi từ khi
    struct ra đời) = người gửi do **Steam** xác thực; `net.rs` bỏ mọi gói có
    SteamID trong nội dung khác người gửi thật (`sender_matches`).
  - Request id ngẫu nhiên (`RandomState` + thời gian, khác 0).
  - `HERE` có block `-1`, tọa độ NaN/vô cực hoặc > 100 000 bị bỏ
    (`spot_is_sane`); vẫn chờ tiếp, hết giờ thì báo như thường.
- **Giữ tối đa 1 `WHERE` hoãn lại cho mỗi người hỏi** (cái mới thay cái cũ) -
  trước đó ai gửi dồn dập lúc mình đang loading thì vào lại game sẽ trả hàng
  loạt `HERE` một lúc.
- Comment lỗi thời: đầu `net.rs` ("kẻ xâm nhập không được trả lời"),
  `warp.rs` ("turned out hostile"), `message_filter` trong `ui.rs`.
- Đoạn mở đầu README viết lại cho đúng mod độc lập.
- `DESCRIPTION.bbcode`: thêm mục Notes - chỉ bàn phím + chuột (chưa tay cầm),
  và khuyên đặt `cooppassword` riêng vì ai trong phiên cài mod cũng hỏi được
  vị trí của mình.
- 3 unit test mới trong `net.rs` (khớp người gửi, lọc vị trí rác, request id
  ngẫu nhiên khác 0).

Giới hạn đã biết, để sau 1.0.0: chưa chặn teleport khi người kia đang cưỡi
Torrent trên không / trong trận boss / đang chết; cấu trúc đọc qua
fromsoftware-rs (danh sách phiên, vị trí, `protocol_state`) phụ thuộc bố cục
bộ nhớ từng bản game; chưa test phiên từ 3 người.

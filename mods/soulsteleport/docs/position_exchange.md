# SoulsTeleport: lấy vị trí đồng đội qua Steam P2P (WHERE / HERE)

**Status 2026-09-26: ĐANG PHÁT HÀNH (1.1.0), test trong game với Seamless Co-op
(host và join, hai chiều).** Code: `src/net.rs`, `src/steam.rs`, `src/party.rs`. Phần dịch
chuyển nằm ở `warp_mechanism.md`; vai trò và chặn invader ở `roles_and_invaders.md`.

## Vì sao phải tự gửi vị trí qua mạng

Khảo sát Seamless Co-op (2026-09-24, hai instance trên một máy, tài khoản Steam thứ hai chạy
trong Sandboxie):

- Đồng đội **có** trong `WorldChrMan.player_chr_set` và Seamless báo cả hai là `chr_type Local`.
- `PlayerIns.current_block_id` và `block_position` **chỉ được cập nhật cho chính mình**; của
  đồng đội là `-1` / `0`.
- Tọa độ Havok (`CSChrPhysicsModule.position`) của đồng đội **sống và chính xác khi ở gần**
  (cách nhau ~0.9 m), nhưng khi người kia đi xa thì entry vẫn còn mà Havok về `(0, 0, 0)`:
  máy kia không còn biết vị trí. `ChrIns.block_id` thì luôn là block cũ, không đáng tin.

Kết luận: mod phải tự gửi vị trí. Mỗi instance đọc block và tọa độ của chính mình
(`PlayerIns.current_block_id` / `block_position`, luôn đúng cho main player) rồi gửi đi. Công
thức Havok chỉ còn là dự phòng khi ở gần.

## Truyền thông (`src/steam.rs`)

Binding tối thiểu tới `ISteamNetworkingMessages`, lấy bằng `GetProcAddress` từ `steam_api64.dll`
mà game đã nạp sẵn (flat C API, không cần Steamworks SDK hay crate). Chỉ đọc vài trường đầu của
`SteamNetworkingMessage_t`, giải phóng qua export `SteamAPI_SteamNetworkingMessage_t_Release`,
nên không phụ thuộc layout sâu hơn của struct. Kênh riêng `0x5354` ("ST"); SoulsChat dùng 42 nên
không đụng nhau. Gửi **reliable** (mỗi lần teleport chỉ một `WHERE` và một `HERE`, rớt gói là phải
chờ hết giờ). Danh sách người trong phiên lấy từ `CSSessionManager.players` (singleton tra theo
tên, không khoá version; mỗi entry có `steam_id`, `steam_name`, `is_local_player`); xác nhận dùng
được trong Seamless Co-op.

## Từ broadcast sang hỏi khi cần

Bản thử đầu **broadcast** vị trí mỗi 500 ms cho mọi người (gói 66 byte, magic `STP1`): đơn giản
và luôn có dữ liệu để khảo sát, nhưng chỉ là cách tạm. Test cho thấy vị trí nhận được chỉ cũ 0,0-0,4
giây, warp qua loading thành công và phiên co-op giữ nguyên. Bản thật đổi sang **hỏi khi cần**
vì: không phát vị trí liên tục khi chẳng ai teleport, vị trí luôn mới nhất, và không lộ vị trí
cho mọi người cài mod. (`CSSessionManager` còn liệt kê cả người lạ do chưa đổi `cooppassword`
của Seamless; bản broadcast đã gửi vị trí cho cả họ, dù gói bị bỏ qua vì họ không cài mod.)

- **Bên hỏi:** gửi `WHERE` (16 byte: magic `STW1`, SteamID người hỏi, request id) tới đúng một
  người, chờ tối đa **10 giây** (bản nháp đầu 2 giây: quá ngắn, vì đồng đội có thể đang loading
  hoặc hồi sinh và chỉ trả lời khi vào lại world).
- **Bên được hỏi:** trả `HERE` (70 byte: magic `STH1`, SteamID, request id, block, x/y/z/yaw
  trong block, tên nhân vật). Nếu `WHERE` tới khi mình đang loading hay hồi sinh thì giữ lại và trả
  ngay khi vào lại world (trong 10 giây); quá hạn thì bỏ và bên hỏi báo "did not respond". Một
  người hỏi chỉ được giữ tối đa **một** `WHERE` hoãn (cái mới thay cái cũ).
- `HERE` trễ hoặc của request cũ bị bỏ qua (so SteamID và request id).
- `AcceptSessionWithUser` được gọi cho mọi người trong phiên mỗi 2 giây; nếu không, `WHERE` đầu
  tiên từ một người mình chưa từng gửi gì sẽ bị Steam bỏ. Danh sách phiên chỉ được đọc khi cần.

## Chỉ trả lời khi đã "ở trong world" thật sự

Lỗi đã gặp: warp tới người đang ở màn hình loading thì **người warp bị văng ra màn hình chính**
(cả hai chiều). Nguyên nhân: lúc load, `WorldChrMan` vẫn có main player cũ nên máy được hỏi trả
`HERE` ngay với vị trí ở map đang rời đi. Sửa: chỉ trả lời khi đã "ở trong world" liên tục từ
2 giây trở lên (`SETTLE_TIME`), với "ở trong world" gồm ba điều kiện: có main player **và**
`GameMan.warp_requested` bằng false **và** `CSSessionManager.protocol_state` không ở pha tải lại
(`WaitInitData`, `WaitReloadWait`, `WaitReload`, `WaitReload2`, `WaitReentryToMap`). Chưa đủ điều
kiện thì giữ yêu cầu lại như trên. Chưa xác minh Seamless có cập nhật `protocol_state` không; nếu
không thì `warp_requested` cùng 2 giây vẫn là lớp chặn chính. Bên warp cũng không warp khi chính
mình đang có move-map chờ.

## Nhớ vai trò đồng đội khi `PlayerIns` tạm biến mất

Lần bấm đầu của người join tới host bị từ chối nhầm: host đang ở màn hình loading nên
`player_chr_set` của host tạm thời không có `PlayerIns` của người join, vai trò thành `None`, và
luật "chưa rõ vai trò thì không tin" loại ngay ở bước kiểm tra, **trước** bước "chưa ở trong world
thì giữ lại trả lời sau" (nên yêu cầu bị bỏ thay vì hoãn). Lúc đầu đọc log tưởng do host ở xa; thật
ra là do đang loading. Sửa (`party.rs`): `KNOWN_ROLES` nhớ `chr_type` cuối cùng thấy được của từng
SteamID, cập nhật mỗi lần đọc danh sách phiên (ít nhất 2 giây một lần), và người không còn
`PlayerIns` thì dùng vai trò đã nhớ; chỉ người **chưa từng thấy** mới bị coi là chưa rõ.

## Bấm một phím, hai instance cùng dịch chuyển

Lỗi đã gặp khi test hai instance trên một desktop: bấm `F10` ở một cửa sổ, **cả hai** bản game đều
warp. Nhận định ban đầu (hàm của fromsoftware-rs chỉ nhận phím của cửa sổ chính process) là **sai**:
`eldenring::util::input::is_key_pressed` dùng `GetKeyState`, mà mod gọi từ task `FrameBegin` của
game, luồng đó không có hàng đợi input riêng nên thấy trạng thái phím chung của cả desktop. Sửa:
dùng `common::input::is_key_pressed`, kiểm tra cửa sổ foreground thuộc chính process trước khi đọc
phím (instance không được focus không đọc phím gì), và gọi được từ mọi luồng (`GetAsyncKeyState`).

**Còn tồn đọng:** `common::reload` và `autoregen` vẫn gọi thẳng
`eldenring::util::input::is_key_pressed`, nên `ReloadKey` của các mod đó vẫn có khả năng lọt
sang instance khác trên cùng desktop. Chưa sửa.

## Chống giả gói tin (review 2026-09-25)

Trước đó người gửi chỉ lấy từ SteamID ghi trong nội dung gói và request id là 1, 2, 3...: một người
trong phiên dùng mod bị sửa có thể gửi `HERE` giả đúng lúc mình đang chờ và ép warp tới block hoặc
tọa độ rác (crash, rơi khỏi map, kẹt). Giờ:

- `steam.rs` đọc thêm `m_identityPeer` của `SteamNetworkingMessage_t` (offset 16, sau `m_pData`,
  `m_cbSize`, `m_conn`): người gửi do **Steam** xác thực. `net.rs` bỏ mọi gói có SteamID trong nội
  dung khác người gửi thật (`sender_matches`).
- Request id ngẫu nhiên (`RandomState` và thời gian, khác 0).
- `HERE` có block `-1`, tọa độ NaN/vô cực hoặc lớn hơn 100 000 bị bỏ (`spot_is_sane`); vẫn chờ
  tiếp, hết giờ thì báo như thường.
- Đã test lại: `WHERE` với request id ngẫu nhiên, `HERE` sau 28 ms, warp thành công, cả hai log
  không có dòng `Dropped a packet claiming to be from ...`, nên đọc `m_identityPeer` ở offset 16 là
  đúng layout.
- Test hai chiều lần 4: `WHERE` đến `HERE` sau **33 ms**.

## Giới hạn

- Ai trong phiên cài mod (và không thù địch) đều hỏi được vị trí của mình: nên đặt
  `cooppassword` riêng cho Seamless.
- Phụ thuộc layout bộ nhớ từng bản game cho danh sách phiên, vị trí và `protocol_state` (đọc qua
  fromsoftware-rs).
- Chưa test phiên từ 3 người trở lên.

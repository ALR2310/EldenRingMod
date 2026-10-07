# SoulsTeleport: vai trò người chơi và chặn teleport giữa người thù địch

**Status 2026-09-26: ĐANG PHÁT HÀNH (1.1.0), test trong game phía invader.** Code:
`src/party.rs`, `src/net.rs`, `src/ui.rs`. Cách hai máy trao đổi vị trí: `position_exchange.md`.

## Vai trò hiện trong menu

Mỗi dòng hiện vai trò bên phải (`Member::role`): `Host` (từ `SessionManagerPlayerEntry.is_host`), rồi
theo `chr_type`: `Co-op` (Seamless Co-op báo mọi người là `Local`), `Cooperator`, `Invader`, `Hunter`,
`Bloody Finger`, `Recusant`, `Arena`, `Other`; `Unknown` nếu chưa từng thấy nhân vật (người đó chưa
vào tới thế giới, chưa có `PlayerIns`, không phải lỗi). Màu: Host vàng, thù địch đỏ nhạt,
Unknown/Other xám. Thông tin ghép từ `CSSessionManager.players` (SteamID, tên Steam) với
`player_chr_set` (tên nhân vật, `chr_type`) qua `PlayerIns.session_manager_player_entry.steam_id`.
`KNOWN_ROLES` nhớ vai trò cuối cùng thấy được để hiện khi `PlayerIns` tạm biến mất.

## Các giai đoạn của quy tắc teleport

1. **Chỉ đồng đội** (2026-09-24): chỉ người không thuộc nhóm thù địch và có `PlayerIns` mới được tính.
2. **Bỏ lọc, hiện mọi người** (2026-09-25, theo yêu cầu người dùng): mọi người trong phiên đều là
   đích teleport và đều được trả lời `WHERE`, kèm vai trò để người chơi tự biết.
3. **Chặn giữa người thù địch** (2026-09-26): xem bên dưới. Đây là quy tắc hiện hành.

## Quy tắc hiện hành

Góp ý trên Nexus (Kolagon): "make sure this doesn't work for invaders, otherwise Reds are gonna tele
to hosts". Đúng: từ giai đoạn 2, một invader cùng phiên Seamless Co-op có thể dịch chuyển thẳng tới
host, phá PvP. Quy tắc: một cặp chỉ teleport được khi **cả hai bên đều không thù địch**.

- **Thù địch** (`party::is_hostile_type`): `Duelist` (Invader), `BloodyFinger` /
  `FesteringBloodyFinger` / `BloodyFingerNpc`, `Recusant` / `RecusantNpc`. **Hunter (`BluePhantom`) vẫn
  được phép** vì đến để giúp host.
- **Bên hỏi:** `party::own_is_hostile()` (đọc `WorldChrMan.main_player` `chr_type`): đang là invader
  thì menu chỉ hiện "Teleport is disabled while invading." và `request_partner_position` từ chối.
  Người thù địch khác không có trong danh sách (`Member::is_teleport_target` loại `Member::is_hostile()`).
- **Bên trả lời (quan trọng hơn):** `Net::answer_where` bỏ qua `WHERE` nếu người hỏi thù địch hoặc chính
  mình đang thù địch, nên host vẫn được bảo vệ kể cả khi invader dùng một bản mod đã bị sửa.
  `flush_deferred` kiểm tra lại vì người hỏi có thể đổi vai trò trong lúc chờ.
- Vai trò chưa từng thấy (`Unknown`) vẫn coi là không thù địch. Không có key ini để bật lại (theo yêu cầu
  người dùng).

## Kiểm chứng

Log thêm (chỉ khi đổi): `Own role: <ChrType>.` cho chính mình và `Role of '<tên>': <ChrType>.` cho từng
người. Test trong game: xâm nhập thế giới người khác, log `Own role: Duelist.` và menu hiện "Teleport
is disabled while invading."; về lại thì `Own role: Local.`. Máy invader cũng thấy chính mình là
`Duelist` qua `player_chr_set` (cùng đường mà máy host dùng để nhận ra invader). Chiều host (bị xâm
nhập) không test riêng, người dùng cho là đủ.

## Giới hạn

Chưa test phiên từ 3 người trở lên. Chưa chặn teleport khi người kia đang cưỡi Torrent trên không,
trong trận boss hay đang chết.

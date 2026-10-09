# SpiritMultiplier

> Nexus mod 11168 · **đã phát hành** · đã test trong game (vanilla, ELDEN RING Reforged, Seamless Co-op)

Mod DLL cho Elden Ring nâng cấp hệ thống triệu hồi Spirit Ash: **nhiều spirit
hơn mỗi Ash vượt trần 10 con** của game, gọi **nhiều Ash khác nhau cùng
lúc**, gọi lại không cần nghỉ, gọi ở bất cứ đâu, bỏ màu ma và cho spirit tự
hồi máu. Cấu hình trong `SpiritMultiplier.ini`, bấm `ReloadKey` để nạp lại
mà không cần khởi động lại game (trừ `MaxSpirits`).

## Cấu hình

Cấu hình trong `SpiritMultiplier.ini` cạnh DLL (tự tạo nếu thiếu; key mới được thêm vào file có sẵn mà không ghi đè giá trị đã chỉnh). Có thể chỉnh:

- số spirit mỗi Ash: nhân hệ số hoặc đặt số cố định, và trần `MaxSpirits`;
- bật/tắt `MultiSpirit`, `NoRestResummon`, `SummonAnywhere`, màu ma (`GhostColor`), cách hồi máu (`RegenMode`, `RegenValue`);
- phím nạp lại cấu hình và việc hiện banner sau khi nạp;
- ghi log và các key chẩn đoán (`SlotProbe`, `EnemyProbe`, `ActiveCharacterLimit` - không nên đụng).

Tên key, giá trị mặc định và ý nghĩa nằm ở chú thích trong [`SpiritMultiplier.ini`](SpiritMultiplier.ini); file mẫu này được nhúng vào DLL nên là nguồn sự thật duy nhất, README không lặp lại để khỏi lệch.

## Cách hoạt động

- **Vượt trần 10 spirit** (`src/band.rs`, `src/chain.rs`): spirit nằm trong
  mảng slot `summon_buddy_chr_set` của game (capacity 80, band người chơi bắt
  đầu ở slot 20, dài đúng 10). Mod patch vòng tìm slot và con trỏ vòng
  (ring) để band dài `MaxSpirits`, nới capacity khi `MaxSpirits > 60`, và kéo
  dài chain SpEffect → BuddyParam theo `Multiplier`/`Amount`. Với Seamless
  Co-op (capacity 1000) mỗi người chơi có band riêng không chồng nhau:
  [docs/slots_and_band.md](docs/slots_and_band.md).
- **Kẻ địch không biến mất** (`src/activate_limit.rs`): game chỉ activate tối
  đa 60 nhân vật, spirit chiếm suất của kẻ địch; mod nâng giới hạn đúng bằng
  số spirit đang có: [docs/activate_limit.md](docs/activate_limit.md).
- **Cải thiện AI spirit** (`src/think_override.rs`): 3 key bật/tắt nạp sẵn
  các field `NpcThinkParam` (giác quan, hung hăng, bám theo) theo kiểu chỉ
  nâng/chỉ hạ, cùng hai file tuỳ chọn để tự chỉnh từng field:
  [docs/think_override.md](docs/think_override.md).
- **Dịch chuyển spirit** (`src/warp.rs`): hạ các ngưỡng dịch chuyển của game
  (khoảng cách, thời gian khuất tầm nhìn, thời gian kẹt) và tuỳ chọn yêu cầu
  dịch chuyển cả khi spirit chỉ ở xa, nhưng chỉ cho spirit của người chơi này:
  [docs/warp_research.md](docs/warp_research.md).
- **MultiSpirit** (`src/multi_spirit.rs`): bỏ luật "1 nhóm spirit" của
  `SummonBuddyManager` (DoSummon/Update/CanUseItem/UI), cho về từng Ash khi
  bấm lại, Ash gọi thêm vẫn tốn FP/HP: [docs/multi_spirit.md](docs/multi_spirit.md).
- **NoRestResummon / SummonAnywhere** (`src/buddy_stone.rs`): sửa
  `BuddyStoneParam` và vài patch để gọi ngoài vùng bia đá:
  [docs/summon_anywhere.md](docs/summon_anywhere.md).
- **GhostColor** (`src/ghost_color.rs`): tắt ghi đè PhantomParam trên các
  `SpEffectVfx` chỉ spirit dùng (hoạt động cả với Reforged):
  [docs/ghost_color.md](docs/ghost_color.md).
- **Regen** (`src/regen.rs`): mỗi giây hồi máu cho spirit và Torrent: số HP cố định, % HP tối đa, hoặc % HP đã mất.
- **Hot reload không cộng dồn:** giá trị param gốc được chụp một lần, mọi lần
  áp/reload tính lại từ bản chụp.
- **Tương thích Seamless Co-op:** mọi patch code chỉ đổi đích `call` sẵn có
  (không sửa byte xung quanh) vì Seamless tự huỷ game khi chữ ký byte lệch:
  [docs/seamless_conflict.md](docs/seamless_conflict.md).
- **Log:** ghi phiên bản game, danh sách DLL đã nạp và từng patch đã cài
  (`common::diag`, không in đường dẫn đầy đủ).

## Giới hạn

- Vượt 60 spirit (`MaxSpirits > 60`) là tự chịu rủi ro: FPS tụt, có thể crash do hết heap nhân vật.
- Seamless Co-op: người chơi không cài mod vẫn dùng band vanilla (trùng band
  của host); đôi khi chỉ đồng bộ một phần spirit sau khi 1 bên load lại khu vực.
- Không dùng chung với mod khác sửa việc triệu hồi spirit (vd. phần spirit của Solid Uncapper): cùng patch các hàm `SummonBuddyManager`.
- ELDEN RING Reforged: spirit gọi ra mặc định ở trạng thái đỏ (Fury) và `MultiSpirit` chỉ cho 1 nhóm vào Fury; `NoRestResummon` không gỡ được luật của Reforged sau Spirit-Severing Blade.
- Seamless Co-op: spirit mất màu ma, trông như kẻ địch thường (chưa sửa).
- Dịch chuyển spirit về người chơi (`Warp*`, `WarpWhenFar`) là thử nghiệm (`WarpWhenFar` mặc định tắt); chỉ chạy trên bản exe mod nhận ra được (mã dịch chuyển của game phải đúng cấu trúc đã phân tích).
- Chỉ dùng ở chế độ offline hoặc Seamless Co-op, tắt EAC.

## Tài liệu liên quan

- [HISTORY.md](HISTORY.md): dòng thời gian phát triển.
- [CHANGELOG.md](CHANGELOG.md): ghi chú phát hành cho người dùng.
- [docs/slots_and_band.md](docs/slots_and_band.md): slot, band, `MaxSpirits`, Seamless.
- [docs/activate_limit.md](docs/activate_limit.md): giới hạn 60 nhân vật active.
- [docs/multi_spirit.md](docs/multi_spirit.md): gọi nhiều Ash cùng lúc.
- [docs/summon_anywhere.md](docs/summon_anywhere.md): NoRestResummon, SummonAnywhere.
- [docs/ghost_color.md](docs/ghost_color.md): màu ma, cờ hành vi spirit.
- [docs/seamless_conflict.md](docs/seamless_conflict.md): xung đột Seamless, quy tắc đổi đích `call`.
- [docs/warp_research.md](docs/warp_research.md): cơ chế dịch chuyển spirit về người chơi và cách mod đẩy nó.
- [docs/open_issues.md](docs/open_issues.md): vấn đề còn mở (Reforged, Seamless).
- [nexus_page.bbcode](nexus_page.bbcode): mô tả trang Nexus.

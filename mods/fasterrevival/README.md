# FasterRevival

> **v1.0.0** · Nexus mod 11160 · **đã phát hành**, test trong game (2026-09-28)

Mod DLL cho Elden Ring: **rút ngắn thời gian từ lúc nhân vật hết máu tới khi
hồi sinh**, không sửa file animation (`.anibnd` / `.tae`). Hồi sinh sau chết
thường từ 12.2-14.4s (vanilla) còn khoảng **4.2-4.7s**. Mod **luôn bật**, cài
vào là chạy, không có gì để chỉnh.

## Cấu hình (`FasterRevival.ini`)

File mẫu nhúng trong DLL (`include_str!`). Không có hot reload.

| Section | Key | Mặc định | Ý nghĩa |
|---|---|---|---|
| `[Debug]` | `DeathProbe` | `false` | Log vết animation chết (anim ID, play time, độ dài) mỗi 100ms trong lúc chết, để chẩn đoán |
| `[Logging]` | `LogFile` | `true` | Ghi `FasterRevival.log` cạnh DLL; `false` thì không tạo file |

Mỗi lần chết luôn có các dòng log `Died`, `Killed early`, `Death registered by
the game: +X ms`, `Respawned: X ms after HP hit 0`, nên so sánh thời gian
không cần bật `DeathProbe`.

## Cách hoạt động

Hai việc độc lập, cộng lại ra mức tiết kiệm trên.

- **Kéo mốc "chết" lên sớm (`src/death.rs`).** Game "giết" nhân vật bằng một
  event TAE gần cuối animation chết, nên màn "YOU DIED" chỉ hiện sau khi animation
  phát hết (~4-6s). Mod gọi trực tiếp hàm kill của game ngay khi HP về 0 và
  animation chết bắt đầu. Hàm kill idempotent nên event TAE về sau thành no-op,
  animation vẫn phát bình thường. Hàm được tìm bằng AOB (duy nhất trên cả
  2.7.1.0 và 2.6.2.0, không phụ thuộc phiên bản). Chỉ kill khi animation đang
  phát thuộc tập có event "Kill Character" (`a000_017xxx`, `070xxx`, `075003`,
  `117xxx`, `000150`). Chi tiết: [docs/death_flow.md](docs/death_flow.md).
- **Bỏ khoảng dừng sau "YOU DIED" (`src/fade.rs`).** Ghi 1 lần
  `MenuCommonParam[0].soloPlayDeath_ToFadeOutTime = 0` (vanilla 3.8s) khi nhân
  vật đã có trong thế giới; không cần patch code. Chi tiết:
  [docs/respawn_fade.md](docs/respawn_fade.md).
- **Khởi động:** `DllMain` tạo 1 thread, nạp/tạo ini, mở log, ghi phiên bản game
  và danh sách DLL (`common::diag`), tìm hàm kill, chờ `CSTaskImp`, rồi đăng ký
  task lặp trên `FrameBegin`. Không có `ReloadKey`: mod chạm vào game ít nhất có
  thể.

## Giới hạn

- Chỉ tác dụng với kiểu chết có animation chứa event "Kill Character" (chết
  thường khi bị đánh, đòn tóm). **Rơi vực không đổi** vì game kill ngay frame
  đầu.
- Hồi sinh có thể lâu hơn khi chết gần **Stake of Marika** (menu chọn điểm hồi
  sinh); không phải lỗi của mod.
- **Chưa test khi đang cưỡi Torrent.**
- Mod không kiểm tra một điều kiện nội bộ của game trước khi kill (nằm trong
  vùng Arxan, chưa hiểu), nên có thể kill sai lúc trong trường hợp hiếm mà điều
  kiện đó từng chặn.
- Không dời event "Send Ghost Info" của mod gốc: dùng mod DLL phải tắt EAC nên
  không có bloodstain/ghost để gửi.
- Nếu game cập nhật làm mất mẫu AOB của hàm kill, log ghi lỗi "game version not
  supported" và phần kill sớm bị tắt (phần bỏ khoảng dừng vẫn chạy).
- Chỉ dùng ở chế độ offline, tắt EAC.

## Nguồn ý tưởng

- **FasterDeathAnimation** của 0-F ([Nexus 3367](https://www.nexusmods.com/eldenring/mods/3367),
  [GitHub](https://github.com/0-F/FasterDeathAnimation), không có LICENSE): chỉ
  tham khảo ý tưởng, không copy code.
- **FasterRespawn** của ImAxel0 ([Nexus 501](https://www.nexusmods.com/eldenring/mods/501),
  [GitHub](https://github.com/ImAxel0/EldenRing-FasterRespawn-Mod), MIT).

## Tài liệu liên quan

- [CHANGELOG.md](CHANGELOG.md): ghi chú phát hành cho người dùng.
- [HISTORY.md](HISTORY.md): dòng thời gian phát triển.
- [docs/death_flow.md](docs/death_flow.md): luồng chết của game, điều kiện kích hoạt, kết quả test.
- [docs/respawn_fade.md](docs/respawn_fade.md): khoảng dừng sau "YOU DIED".
- [nexus_page.bbcode](nexus_page.bbcode): mô tả trang Nexus.

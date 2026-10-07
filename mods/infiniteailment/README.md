# InfiniteAilment

> **v1.0.1** · Nexus mod 11087 · **đã phát hành**, test trong game (2026-09-22), người dùng báo chạy tốt trên Convergence

Mod DLL cho Elden Ring: chỉnh **thời lượng** và **sát thương** của hiệu ứng
damage-over-time **Poison** và **Scarlet Rot** do người chơi gây ra (mặc định
thời lượng vô hạn). Cấu hình trong `InfiniteAilment.ini`, bấm `ReloadKey` để
nạp lại mà không cần khởi động lại game.

## Cấu hình (`InfiniteAilment.ini`)

File mẫu nhúng trong DLL (`include_str!`), là nguồn sự thật duy nhất cho giá
trị mặc định. Tên key có tiền tố (`ScarletRot.`, `Poison.`); `[Section]` chỉ để
trang trí.

| Key | Mặc định | Vanilla | Ý nghĩa |
|---|---|---|---|
| `ReloadKey` | `F5` | | Phím nạp lại ini (chỉ khi cửa sổ game đang focus) |
| `ReloadBanner` | `true` | | Hiện banner "Config reloaded" sau khi reload |
| `ScarletRot.Duration` | `-1` | 90 | Thời lượng DoT (giây), `-1` = vô hạn |
| `ScarletRot.PercentDamage` | `0.18` | `0.18` (`0.33` ở một số nguồn) | % máu tối đa mất mỗi tick |
| `ScarletRot.FixedDamage` | `15` | 15 | Sát thương cố định cộng thêm mỗi tick |
| `Poison.Duration` | `-1` | 90 | Như trên, cho Poison |
| `Poison.PercentDamage` | `0.07` | `0.07` | Như trên, cho Poison |
| `Poison.FixedDamage` | `7` | 7 | Như trên, cho Poison |
| `LogFile` | `true` | | Ghi `InfiniteAilment.log` cạnh DLL; `false` thì không tạo file |

## Cách hoạt động

- **Khởi động:** `DllMain` tạo 1 thread, nạp/tạo ini, mở log, chạy watcher
  `ReloadKey` (`common::reload`), rồi chờ `SoloParamRepository` được nạp
  (**không giới hạn thời gian**) trước khi áp dụng.
- **Chọn dòng cần sửa (`src/status_effect.rs`):** duyệt `SpEffectParam` đang
  chạy và lấy mọi dòng khớp chữ ký của từng ailment (`SpCategory`, `StateInfo`,
  `ChangeHpRate != 0`, `EffectEndurance == 90`): **175 dòng Scarlet Rot và 203
  dòng Poison**. Chữ ký này loại các dòng của quái, hồ độc/hồ rot và boss
  (Malenia). Danh sách ID được **lưu lại sau lần quét đầu**, vì chính điều kiện
  `EffectEndurance == 90` sẽ không còn khớp sau khi đã ghi đè.
- **Ghi 3 field** mỗi dòng: `EffectEndurance`, `ChangeHpRate`, `ChangeHpPoint`.
  Giá trị là tuyệt đối (không phải hệ số nhân), nên reload không cộng dồn.
- **Reload:** task lặp trên `FrameBegin` đọc lại ini và áp lại cả hai ailment
  mỗi khi `ReloadKey` được bấm.
- Cách tìm ra đúng tập dòng này (kèm hướng sai đã bỏ) nằm ở
  [docs/row_discovery.md](docs/row_discovery.md).
- **Log:** ghi phiên bản game và danh sách DLL đã nạp (`common::diag`, không in
  đường dẫn đầy đủ, ẩn DLL của game/Steam/bản crack).

## Giới hạn

- Chỉ sửa các dòng "người chơi tự gây ra"; Poison/Rot của quái, hồ độc, hồ rot
  và Malenia giữ nguyên.
- Chữ ký lọc dựa trên giá trị vanilla (`EffectEndurance == 90`): regulation
  overhaul đổi giá trị gốc này có thể khiến mod không khớp dòng nào.
- Dùng `repo.rows()` của fromsoftware-rs ở lần quét đầu (có thể panic với
  regulation lệch header, xem
  [dropmultiplier](../dropmultiplier/docs/convergence_panic.md)); chạy tốt trên
  Convergence theo người báo lỗi.
- Chỉ dùng ở chế độ offline, tắt EAC.

## Tài liệu liên quan

- [CHANGELOG.md](CHANGELOG.md): ghi chú phát hành cho người dùng (gửi lên tab Changelog của Nexus).
- [HISTORY.md](HISTORY.md): dòng thời gian phát triển.
- [docs/row_discovery.md](docs/row_discovery.md): tìm đúng các dòng `SpEffectParam`.
- [nexus_page.bbcode](nexus_page.bbcode): mô tả trang Nexus (không chứa changelog).

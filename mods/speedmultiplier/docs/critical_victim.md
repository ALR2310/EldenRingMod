# Nạn nhân của đòn chí mạng (backstab / riposte)

**Status 2026-10-07: đã làm (`speed.rs::sync_victims`), test trong game ổn. Chưa hỗ trợ Seamless Co-op.**

Người chơi được tăng tốc theo `Critical` nhưng kẻ địch bị túm thì không, nên người chơi xong
đòn mà kẻ địch vẫn đang "gập người" rồi mới bị hất ra (Nexus, Pietrasante). Mod giờ cho
nạn nhân chạy cùng tốc độ với người chơi trong lúc đòn diễn ra.

## Cách hoạt động

`ChrIns.modules.throw` (`CSChrThrowModule`, fromsoftware-rs `throw.rs`) → `throw_node.throw_state`.
Đọc bằng log probe (cùng số cho backstab `a0xx_031710` và riposte `a0xx_031700`):

| | Đòn không giết | Đòn giết |
|---|---|---|
| Người chơi | 0 → 1 → 3 → 0 | 0 → 1 → 3 → 5 → 0 |
| Kẻ địch | 0 → 2 → 4 → 0 | 0 → 2 → 4 → 6 → 0 |

Người chơi ở 1/3/5 thì kẻ địch ở 2/4/6 ghi `animation_speed` bằng tốc độ hiện tại của người
chơi; người chơi về 0 thì kẻ địch về 1.0 ngay. Kẻ địch về state 0 muộn hơn người chơi 1-2 giây
(đoạn đứng dậy), đoạn đó để tốc độ gốc.

## Đã thử / bỏ

- **Giữ tốc độ tới hết state của kẻ địch:** kẻ địch còn sống bị tăng tốc cả lúc đứng dậy. Bỏ.
- **Key thử `VictimStopState`** (dừng khi state người chơi vượt N): người chơi chỉ qua 1, 3, 5, cả đòn là
  1 state 3 liền nên không có số nào cắt giữa chừng. Đã bỏ.
- **Đọc anim time của kẻ địch:** `time_act` của nạn nhân chỉ là anim giữ chỗ `a004_043000` dài 0.033 s
  (anim cặp chạy ở chỗ khác), không dùng được. Anim của người chơi dài 5.37 s (`a042_031700`).
- Nếu sau này cần cắt giữa chừng: dùng `play_time / anim_length` của người chơi làm mốc.

## Giới hạn

Seamless Co-op: stub getter `animation_speed` chỉ cho qua con trỏ người chơi và Torrent
(`seamless.rs`), nên nạn nhân vẫn đọc 1.0 (không hại).

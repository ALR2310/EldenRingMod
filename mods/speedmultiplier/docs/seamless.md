# SpeedMultiplier: Seamless Co-op hook lại getter của `animation_speed`

**Status 2026-10-03: ĐÃ SỬA (phát hành 1.1.0), test trong game kể cả khi hai bản game kết
nối co-op (Sandboxie).** Code: `mods/speedmultiplier/src/seamless.rs`. Nhật ký so sánh
code Seamless: `dumps/seamless-diff/`.

## Triệu chứng

Nexus (DrKSolo): mod không chạy với Seamless Co-op. Người dùng tái hiện trên cả 1.0.0 và
bản hiện tại (hệ số 3 cho đi bộ): log ghi `speed=3.00` nhưng trong game không nhanh hơn.

## Nguyên nhân

Game chỉ đọc `CSChrBehaviorModule.animation_speed` ở **một chỗ** (xem
`animation_speed_research.md`): update behavior `sub_14041DCA0` gọi thunk `sub_140417EA0`
(`jmp <getter>`, getter trả `[rcx+0x18]`, rcx = behavior+0x17B0). Bản so code trong RAM với
exe khi có Seamless (2026-10-02, lúc dò màu ma của spiritmultiplier) cho thấy đúng byte
`0x140417EA1` là rel32 của `jmp` đó: **Seamless đổi đích sang code của nó**, nên giá trị mod
ghi không bao giờ được đọc.

## Cách sửa (`src/seamless.rs`)

- Tìm lệnh gọi thunk bằng AOB `48 8D 8F B0 17 00 00 E8 ?? ?? ?? ?? 0F 28 F0 F3 0F 59 B7 C0 15
  00 00` (call ở +7).
- Mỗi tick, nếu `jmp` của thunk không còn trỏ vào `eldenring.exe` (bị DLL khác hook), đổi
  rel32 sang stub của mod: owner của behavior (`[rcx-0x17A8]`) là người chơi hoặc Torrent
  (`OWN_CHRS`, `speed.rs` cập nhật mỗi frame) thì trả `animation_speed`; nhân vật khác thì
  nhảy tiếp vào hook cũ.
- Chỉ đổi rel32 (như spiritmultiplier 1.1.4) và làm **sau** khi Seamless đã hook, nên không
  đụng signature của Seamless; chỉ tra module khi đích thay đổi. Stub đã kiểm bằng Capstone.

## Kiểm tra tương thích (IDA, 2026-10-07)

Mẫu AOB duy nhất ở cả ba bản exe và nằm trong hàm update behavior:

| Bản exe | RVA khớp | Hàm |
|---|---|---|
| 2.6.2.0 | `0x41D7C0` | `sub_14041D760` |
| 2.7.0.0 | `0x41DD00` | `sub_14041DCA0` |
| 2.7.1.0 | `0x41DD00` | `sub_14041DCA0` |

## Giới hạn

Hình ảnh **không đồng bộ giữa hai máy**: bên join đánh nhanh (hệ số lớn), bên host thấy
đòn đó chậm hơn. Mỗi máy chỉ tăng tốc nhân vật của chính nó; nhân vật người khác trên máy
mình chạy theo game/Seamless.

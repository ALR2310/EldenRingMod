# Dịch chuyển spirit về người chơi

**Status 2026-10-09: hoạt động (đã test in-game: hạ ngưỡng và `WarpWhenFar`). Mã: `src/warp.rs`. Mặc định: `WarpDistance=25`, `WarpBlockedTime=1`, `WarpStuckTime=2`, `WarpStuckRange=3` (các giá trị đã test), `WarpWhenFar=false`.**

## Cơ chế của game (2.7.1.0, từ IDA)

`SummonBuddyManager.warp_manager` giữ một `std::map<FieldInsHandle, entry>` (sentinel ở `+8`), mỗi spirit một mục. Node: left +0, parent +8, right +16, isnil +25, key +32, giai đoạn (stage) +48, cờ +96, bộ đếm tia chặn +100, bộ đếm +104, thời gian kẹt +128/+132.

- `sub_1404C28D0` tính lại cờ mỗi frame: bit0 cờ của chr, bit1 chr không có group sống, bit2 `chr->vtable[464]()->+192->+55752->+40 == 82` (hay tắt), bit3 kẹt đường > `+32`, bit4 tia chặn tới người chơi (thời gian cộng dồn ở +100), bit5 thời gian chặn > `+24`, bit6 khoảng cách bình phương >= `+28`², bit7 một trạng thái chung (`sub_14037C970(.., 31) == 6`), bit8 xa quá ngưỡng 1,5 s.
- `sub_1404C2FD0` đặt stage 0 → 1 (RequestWarp) khi (cờ & 7) == 7 và: (bit3 và (bit4 hoặc bit6)), hoặc bit5, hoặc (bit7 và ...). Bit 8 một mình không ai đọc: spirit chỉ ở xa mà còn thấy người chơi thì chạy bộ.
- `sub_1404C3180` đẩy stage 1 → 2 (chọn điểm gần người chơi bằng `sub_1404C0EC0`, chuyển chr) → 3 (hiện ra) → 0.
- Bốn ngưỡng runtime nằm ở `warp_manager` (`+0x18` thời gian chặn, `+0x1C` khoảng cách, `+0x20` thời gian kẹt, `+0x24` quãng đường kẹt), sao chép từ `GameSystemCommonParam` `buddyWarp_*` (mặc định 5 s, 33 m, 3 s, 1 m) và đặt lại khi tải khu vực. Tooltip Smithbox: khoảng cách đường thẳng để dịch chuyển; thời gian dịch chuyển khi tia bị chặn; thời gian phán định kẹt; khoảng cách coi là tắc.

## Mod làm gì

- `WarpDistance`, `WarpBlockedTime`, `WarpStuckTime`, `WarpStuckRange` (0 = của game): ghi mỗi giây vào bốn ngưỡng trên, nhớ giá trị của game để trả lại.
- `WarpWhenFar`: 10 lần mỗi giây, spirit của người chơi này ở stage 0 mà có cờ bit8, bit5, hoặc (bit3 và (bit4 hoặc bit6)) thì đặt stage = 1; engine lo phần còn lại. Không đụng mục của người chơi khác (so key với `groups` không remote).
- `WarpProbe`: ghi log trạng thái manager khi đổi.
- An toàn: `layout_known` tìm 2 đoạn mã của chính engine đọc các offset trên (AOB), không thấy (game cập nhật) thì tắt tính năng; mọi node được kiểm tra đọc được trước khi đụng.

## Kết quả test (Convergence, 2026-10-09)

- Hạ ngưỡng: spirit dịch chuyển sớm hơn khi khuất tầm nhìn (cờ có bit5, `0x177`).
- Chạy nhanh và xa mà không bị chặn: vanilla chạy bộ; với `WarpWhenFar` spirit được dịch chuyển sau ~1,5 s (cờ `0x147`, stage 1 → 2 → 3 → 0).
- Bị chặn 4,8 s mà cờ là `0x033` (thiếu bit2): engine không dịch chuyển; `WarpWhenFar` xử lý trường hợp này.

## Điều chưa làm

Cả nhóm thường dịch chuyển cùng lúc (cùng bị xa); chưa tách từng spirit. Lần thử cũ (chỉ hạ ngưỡng khi chưa có công cụ xem trạng thái) kết luận sai là không có tác dụng.

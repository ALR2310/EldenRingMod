# Dịch chuyển spirit về người chơi (nghiên cứu, hoãn)

**Status 2026-10-08: không có tính năng; thử chỉnh ngưỡng `buddyWarp_*` không có tác dụng. `src/warp.rs` và 2 key `WarpDistance`/`WarpBlockedTime` đã gỡ trước 1.0.0.**

## Dữ liệu

- `GameSystemCommonParam`: `buddyWarp_TriggerDistToPlayer = 33`, `buddyWarp_TriggerTimeRayBlocked = 5`, `buddyWarp_ThresholdTimePathStacked = 3`, `buddyWarp_ThresholdRangePathStacked = 1`. Engine giữ bản sao ở `SummonBuddyManager.warp_manager` (`SummonBuddyWarpManager`, giai đoạn `RequestWarp → Warping → FadeIn`; field `trigger_dist_to_player`, `trigger_time_ray_block`).
- Quan sát: chỉ xa thôi không đủ, còn phải khuất tầm nhìn và/hoặc bị kẹt; cách kết hợp AND/OR chưa đọc trong code.
- `NpcThinkParam` Latenna và Lone Wolf giống nhau ở mọi field "quay về" (`maxBackhomeDist 9999`, `backhomeDist 9979`, `isBuddyAI 1`, `backToHomeStuckAct 0`) → đi theo/dịch chuyển không do các field này. Latenna đứng yên do SpEffect `297103` NO_FOLLOW; mọi spirit có `297000` "Follow & Warp to Player" (ô 28), xem [ghost_color.md](ghost_color.md).

## Thử và kết quả

Mod ghi 2 giá trị vào `warp_manager` mỗi giây (hot reload): log xác nhận giá trị được ghi (tới `0 m` / `0 s`; game đặt lại 33/5 khi load khu vực và mod ghi đè lại) nhưng spirit **vẫn không dịch chuyển**. Nghi: dịch chuyển chỉ xét spirit đã có trong `SummonBuddyWarpManager::entries` (cần cơ chế khác yêu cầu trước), hoặc engine đọc param ở chỗ khác.

## Bước tiếp (nếu quay lại)

Đọc hàm update của `SummonBuddyWarpManager` trong IDA. Hướng dịch chuyển người chơi (không phải spirit) đã có trong [soulsteleport/docs/warp_mechanism.md](../../soulsteleport/docs/warp_mechanism.md).

# Regen.PerTick: Trigger Idle (3) và Gesture (4)

**Status 2026-10-08: hoạt động, đã test trong game (Kolagon, Nexus). Mã: `src/regen.rs` (`confirm_pending_gesture`, `is_idle`, `is_gesture_active`, `common::player::main_player_action_snapshot`).**

## Idle (`Trigger=3`)

Đọc thẳng `CSChrActionRequestModule`: không di chuyển (`movement_request_flags.raw_input`) và không giữ action nào (`r1/r2/l1/l2/sp_move/jump/use_item/action/guard/rideon/rideoff/ladderup/ladderdown`). Phải đứng yên liên tục `IDLE_GRACE_MS = 5000` (tính từ lần `busy` cuối, `IDLE_SINCE_MS`) mới tính idle. Không cần `AttackHook`.

## Gesture (`Trigger=4`, key `Regen.PerTick.GestureId`)

Mặc định 10 gesture ngồi: `80,90,91,92,93,94,95,97,100,101`. `requested_gesture` đúng bằng **1 nửa** `GESTURE_ID` trong CE table của The Grand Archives (Dejection 160 → 80, Rest 184 → 92, Sitting Sideways 186 → 93); hệ số /2 chỉ suy ra từ log thật, không có tài liệu. Danh sách đọc từ ini nên người dùng tự thêm/bớt gesture. Tính năng ban đầu tên "Sitting", đổi thành "Gesture" 2026-09-17 vì "ngồi" chỉ là giá trị mặc định.

`requested_gesture` chỉ đúng **1 frame** khi `new_action_presses.gesture()` bắn, nên phải chốt (latch) `IS_GESTURE_ACTIVE`. Không có cách phân biệt "bắt đầu" với "huỷ" từ tín hiệu đó: nếu đang active thì lần bấm gesture kế (bất kể ID) là huỷ → đứng dậy xử lý ngay; huỷ chốt khi có input "busy" khác (di chuyển/tấn công/dùng đồ/đỡ/cưỡi ngựa).

## Cơ chế xác nhận (lần thử thứ 4, thành công)

Vấn đề: bấm gesture giữa lúc đang cast phép thì game chặn, nhưng `requested_gesture` vẫn được ghi → không được tính là ngồi (báo lỗi Kolagon 2026-09-13). Giải pháp: **xác nhận sau một khoảng trễ** bằng `CSChrTimeActModule::anim_queue[read_idx].anim_id` (TAE anim đang chạy). Không cần bảng map TAE↔gesture, chỉ cần "anim có đổi so với lúc bấm không".

- Bấm khớp danh sách → chỉ ghi `PENDING_GESTURE_SINCE_MS` + `PENDING_GESTURE_ANIM_ID` (anim lúc bấm).
- Chốt `IS_GESTURE_ACTIVE=true` khi `anim_id` **khác lúc bấm VÀ khác 0 VÀ giữ ổn định liên tục `ANIM_STABLE_MS` (1000ms)**; hết `GESTURE_CONFIRM_TIMEOUT_MS` (6000ms) thì bỏ pending. Đủ cả 2 điều kiện đầu vì: chỉ "khác 0" sai khi anim cast chưa kịp đổi; chỉ "khác lúc bấm" sai khi anim cast tự kết thúc về 0 trong lúc chờ.
- Bấm lặp lại khi còn pending bị **bỏ qua** (không đụng baseline/timer), để lần đầu chạy hết chu trình; nếu sau đó thành `true` thì lần bấm kế rơi vào nhánh "đang ngồi → huỷ". (Trước đó ghi đè baseline làm không bao giờ confirm, rồi confirm nhầm lúc đứng dậy.)
- Timeout 3000 → 6000ms (2026-09-22): quay gấp ngay trước khi bấm làm anim xoay/dừng mất >3s để ổn định → xác nhận hết hạn, regen không hồi.
- Test: 5/5 cast + bấm gesture bị chặn đúng, 2/2 ngồi thật, 2/2 đứng dậy ngay; nhiều lần bấm dồn dập đều đúng. Chi phí ≈ 0 khi không pending (1 biến atomic).

## Hướng đã thử và sai (đừng lặp lại)

- `queued_action_inputs.gesture()` (2.6.0): chỉ set cho action có trong `possible_action_inputs`, mà gesture wheel không đi qua đó → gần như không bao giờ bật, Sitting mất hẳn tác dụng (lỗi im lặng, không log).
- `TaeCancelFlags::cancel_disable`: `false` ngay cả khi đang niệm phép.
- `possible_action_inputs.gesture()`: `false` ngay cả khi ngồi thành công.
- `possible_action_cancels` (OR toàn bitfield): tưởng đúng qua 11 lần test, hoá ra **dao động đều mỗi 3 giây** giữa 0 và `31073500911` khi đứng yên (artifact chu kỳ anim idle). **Điều kiện bắt buộc trước khi thử field mới:** có log nhiều phút không tương tác chứng minh field đứng yên khi không cast.
- RE tĩnh (Ghidra, IDA 9.3): hàm `PlayGesture` `0x14078a9c0`, gate `GetManipulatorKind() == 1` (Pad) - không giải thích được bug (solo luôn là Pad kể cả khi cast).
- Bản đầu confirm chỉ check "khác 0" bị sai dương; anim chuyển tiếp qua nhiều ID trung gian (đổi ~0.5-1s) nên cần ổn định liên tục chứ không đọc 1 lần.

## Log

`Gesture: requested_gesture=<n> -> ...` mỗi lần bấm, `Regen.PerTick: trigger=<n> -> condition_met=<bool> (...)` chỉ ghi khi `(trigger, condition_met)` đổi (dedupe đã sửa 2026-09-17 vì `idle` nhảy khi đi rồi dừng cũng gây spam).

# Xung đột với Seamless Co-op: đổi đích `call` thay vì sửa byte

**Status 2026-10-08: sửa ở 1.1.4, đã test với Seamless Co-op 2.0.1 và offline. Quy tắc cho mọi patch code của mod này. Mã: `common::codepatch::{rel32_target, redirect_rel32}`.**

## Sự cố

Từ 1.1.0 tới 1.1.3, game dừng ngay với Seamless Coop 2.0.1 "Fatal Error: No such pattern `E8 ? ? ? ? 83 F8 02 41 0F 44 FC` ... signatures.cpp 1426". Đó là chỗ tính HP cost `0x1403C0C2F` (`call GetBuddyState; cmp eax,2; cmovz edi,r12d`) mà patch FP/HP của MultiSpirit thay bằng `jmp` inline (`E8` → `E9` + NOP). Seamless tìm chỗ patch của nó bằng chữ ký byte và **tự huỷ game** khi không khớp. Từ 1.0.0 tới lúc đó toàn test offline nên không lộ.

## Điều tra

- `ersc.dll` (13 MB, không có version resource): database IDA ở `dumps/ersc/fcd11a18/` (gitignore). Chuỗi gần như đều mã hoá; `ersc\buddy\buddy.cpp` chỉ là cấp slot spirit (65 slot / người chơi, `sub_18008A7D0`).
- Dump crash: chữ ký của Seamless chỉ được giải mã lúc dùng → không liệt kê được hết → phải giả định bất kỳ byte nào quanh chỗ patch đều có thể nằm trong 1 chữ ký của họ.

## Quy tắc

Chỉ ghi lại 4 byte displacement của 1 lệnh `call`/`jmp rel32` sẵn có sang stub gần đó; opcode và mọi byte xung quanh giữ nguyên (chữ ký Seamless wildcard đúng phần `E8 ? ? ? ?`). Nếu mod khác đã đổi đích cùng lệnh, `rel32_target` trả về stub của họ → 2 mod nối tiếp nhau. Stub là "hàm được gọi": vào với `rsp % 16 == 8`, tự `sub rsp,28h/38h` trước mỗi lời gọi bên trong, kết thúc bằng `ret` hoặc tail `jmp`.

Đã chuyển: `multi_spirit.rs` (UI x3, CanUseItem, FP/HP x2, recall, DoSummon, DisappearAll), `buddy_stone.rs` (stateInfo call/tail, in-range). Hai patch inline cũ (giữ bia đá 8 byte, xoá `+0x38` 7 byte) bỏ, logic chuyển vào stub (`decide_state`, stub stateInfo). `band.rs` giữ nguyên (đã chạy với Seamless từ 1.0.0).

## Test

Seamless 2.0.1 (`ersc.dll` nạp): game không còn bị huỷ, mọi dòng cài đặt `was` trỏ về hàm gốc của game; MultiSpirit gọi nhiều Ash và cho về từng Ash (cả Ash nâng cấp `258001`, Ash 15 spirit `240000`); SummonAnywhere dùng bia dự phòng `10000100`. Offline: 12 patch cài đủ. Reforged (`reforged.dll` nạp): 12 patch, không lỗi.

## Seamless còn gây

Spirit mất màu ma khi chơi Seamless (hook `sub_1403F0C20`): [open_issues.md](open_issues.md).

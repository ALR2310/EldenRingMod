# MultiSpirit: gọi nhiều Ash khác nhau cùng lúc

**Status 2026-10-08: hoạt động (vanilla, Reforged, Seamless), đã test. Mã: `src/multi_spirit.rs`. Reforged + Fury xem [open_issues.md](open_issues.md).**

## Vanilla (2.7.1.0, offset theo `SummonBuddyManager` của fromsoftware-rs)

- `GetBuddyState(mgr, goodsId)` = `sub_1404B72F0`: 2 = đang có spirit và `goodsId` trùng Ash đang ra (`+0xE4`) hoặc `goodsId = -1`; 1 = đã sang khu bia đá khác; -1 = không có gì.
- `DoSummon(mgr, speffect)` = `sub_1404B85B0`: `GetBuddyState(mgr, -1)` → hễ có spirit là bật `disappear_requested` (`+0x28`) và thoát (luật "1 nhóm spirit"); không thì spawn, `+0x20 = speffect`. SpEffect được làm tròn `100 * (a2 / 100)` trước khi tra.
- `Update` = `sub_1404B86D0`: có request thì tìm nhóm trong `groups` (`+0x70`), nhóm có spirit thì `sub_1404BB9A0` thu hồi cả nhóm cũ, rồi spawn nhóm mới.
- `CanUseItem` = `sub_14068EE60`: với Ash (loại 7/8) `usable = GetBuddyState(mgr, goodsId) != -1` (`0x140690383`) → đây là chỗ làm xám Ash khác.
- `sub_1403C0930` dựng chi phí dùng item: nếu `GetBuddyState(mgr,-1) == 2` thì FP (`+0xB8`) và HP (`+0xB4`) = 0 (vanilla coi là "cho về" nên miễn phí).
- Nối goods → SpEffect: mọi Ash có `refCategory = 2` và `refId_default` = SpEffect kích hoạt (khoá của `trigger_speffect_to_buddy_map`); mỗi cấp nâng là 1 goods riêng (+0 = `207000`, +4 = `207004`), làm tròn `100 * (id / 100)` cho 86 khoá, không 2 Ash chung khoá.

## Các patch (từ 1.1.4 đều đổi đích `call`, xem [seamless_conflict.md](seamless_conflict.md))

Viết lại (không copy) từ phân tích Solid Uncapper 2.3.3 (MojoW):

1. **DoSummon** (AOB `E8 ?? ?? ?? ?? 83 F8 01 76 1B 83 F8 02 75 04 C6 43 28 01`, `0x1404B85C9`): đổi đích `call GetBuddyState`; `decide_state` trả 2 (cho về) hoặc 0 (gọi thêm). Nhận "cùng Ash" bằng `decide_dismiss`: lấy BuddyParam id của SpEffect (đã làm tròn) trong `trigger_speffect_to_buddy_map`, xem `groups` còn entry sống (`!disappear_requested`, `!is_remote`) có `buddy_param_id` thuộc danh sách không. Có → nhớ `DISMISS_TARGET`, state 2; không → state 0. Cũng gán `+0x38 = +0x3C` khi `+0x38 = 0`.
2. **Update** (AOB `8B 10 49 8B CF E8 ?? ?? ?? ?? 41 83 BF 88 00 00 00 00`, `0x1404B89AB`): bỏ qua `call sub_1404BB9A0` (thu hồi nhóm cũ) khi bật.
3. **UI x3** (`0x1407C3C9E`, `0x140847A1B`, `0x140847CDF`): `GetBuddyState` thật rồi `eax = 0` nếu bật và kết quả >= 1 (bỏ hộp thoại "gửi spirit về?").
4. **CanUseItem** (AOB `48 8B C8 41 8B D5 E8 ?? ?? ?? ?? 83 F8 FF 41 0F 95 C5`, `0x14069037D`): kết quả -1 thì hỏi lại với goodsId -1, ra 2 → trả 0; ngoài khu bia đá vẫn xám như vanilla.
5. **Giữ bia đá**: `+0x38` về 0 sau lần gọi đầu nên lần 2 bị coi "rời khu" và cả 2 nhóm biến mất sau ~10 s; `decide_state` giữ `+0x3C` cũ (mục "bia đá" của [summon_anywhere.md](summon_anywhere.md)).
6. **Cho về từng Ash**: vanilla chỉ có `DisappearAll` nên bấm lại 1 Ash cho về cả các nhóm. Đổi đích `call DisappearAll` của nhánh `+0x28` trong `sub_1404B92B0` (`0x1404B9587`, AOB `41 80 7F 28 00 74 08 49 8B CF E8 ...`) sang `disappear_all_hook`: khi bật, `+0x28` đang set và có `DISMISS_TARGET` thì đánh dấu tạm `disappear_requested` mọi entry sống của Ash khác, gọi hàm gốc, gỡ dấu; còn nhóm khác thì khôi phục `+0x3C` mà hàm gốc vừa xoá; Ash đích hết spirit sống thì không gọi hàm gốc. Mọi lý do khác (rời khu, chết, SpEffect 202) giữ vanilla (cho về hết). Log: `sent back Ash (SpEffect N): X spirit(s), Y other(s) kept`.
7. **FP/HP cho Ash gọi thêm** (`0x1403C0B79` FP / `0x1403C0C2F` HP): `cost_state` trả 2 (miễn phí) chỉ khi chính Ash được bấm còn spirit sống (= cho về), ngược lại tính giá bình thường. Solid Uncapper 2.3.3 không có phần này.

**Không phải Spirit Ash** (vd. Spirit-Severing Blade của Reforged đi qua cùng `DoSummon` với SpEffect không thuộc Ash nào): `ash_alive` trả `None` khi SpEffect không có chain → `decide_state`/`cost_state` giữ hành vi vanilla (cho về tất cả / miễn phí).

## Bài học / lỗi đã gặp

- Ash đã nâng cấp (+1..+10) bấm lại gọi thêm thay vì cho về (Nexus MonkeyDLuffy2426, 1.1.2): `buddy_ids` tra thẳng `207004` không thấy. Sửa làm tròn `100 * (speffect / 100)` ở 1 chỗ, dùng chung `decide_dismiss`, `disappear_all_hook`, `cost_state` (1.1.3). Mimic Tear: 3 con `133201000` tự biến mất sau ~8 s là hiệu ứng xuất hiện của Mimic, không qua send-back.
- So `[+0x24]` (active) thay vì chỉ `[+0x20]` (pending) như Solid Uncapper, vì `Update` đặt `+0x20 = -1` sau khi spawn nên dùng lại cùng Ash sẽ bị coi là "Ash khác".
- Test lần 3 lẫn `Solid_Uncapper.dll` trong thư mục mod; lần 4 mới sạch (xem memory test isolation).
- Hàm UI `sub_140847830`/`sub_140847B20` chỉ hiển thị chi phí, không phải chỗ làm xám; không có hàm nào vừa đọc `EquipParamGoods+0x79` (`useLimitSummonBuddy`) vừa gọi thẳng API summon.

## Xung đột

Solid Uncapper (Multi Spirit / Max Spirits Out) và bất kỳ mod nào patch các hàm `SummonBuddyManager` cùng chỗ - không dùng chung; mô tả Nexus có ghi Notes.

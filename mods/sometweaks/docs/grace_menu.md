# GraceMenu: thêm mục vào menu Site of Grace bằng ESD sống

**Status 2026-10-08: hoạt động, đã test trong game (2026-08-28, 3 mục Nâng cấp / Mua / Bán + UnlockShop). Mod đã bỏ (xem [../README.md](../README.md)); đây là phần có giá trị tái sử dụng nhất. Mã: `src/grace_menu/{mod,msg_hook,unlock_shop}.rs`.**

## Công thức chèn 1 mục menu (chạy hoàn toàn runtime từ DLL, không sửa file ESD tĩnh)

Mỗi lần vào Site of Grace game dựng 1 **bản copy ESD graph mới** trong bộ nhớ; mod bắt khi graph vào state và chèn mục vào đúng bản đó.

1. Dựng 1 `EzState` **hoàn toàn mới** (`Box::leak`), `entry_events` = chuỗi lệnh ESD mong muốn (vd. `OpenRegularShop(0, 9999999)`, `OpenEnhanceShop` kèm 4× `CombineMenuFlagAndEventFlag` + `c1_141(9)`, `OpenSellShop(-1,-1)`).
2. Đặt `transitions[0].evaluator` = `MenuCloseExpr(<loại menu của lệnh đó>)` (biểu thức ESD `CheckSpecificPersonMenuIsOpen(type, 0) == 0 || CheckSpecificPersonGenericDialogIsOpen(0)`): loại 9 cho `OpenEnhanceShop`, 5 cho `OpenRegularShop`, 6 cho `OpenSellShop`, 1 cho talk menu. Tự mã hoá bằng bảng opcode ESD (`AST.cs` của SoulsIds), chỉ khác 1 byte giữa các loại.
3. `target_state` trỏ về 1 state vanilla **không bao giờ bị sửa** (`find_menu_rebuild_state`: state có đúng 1 `entry_events` là `clear_talk_list_data`, tìm theo nội dung chứ không theo địa chỉ).
4. Chèn thêm `add_talk_list_data` (text) cạnh mục "Sort Chest" (anchor tìm theo nội dung: `is_sort_chest_event`, `targets_open_repository`).

**Nguyên nhân bug "nhân vật đứng dậy" khi mở menu (đã giải):** không phải lệnh `OpenEnhanceShop` ép đứng dậy mà do điều kiện chờ của state không khớp loại menu. Điều kiện gốc của "Tailoring Shop" dành cho menu loại khác, đánh giá "đã đóng" trong khi UI Enhance Shop còn đang khởi tạo → ESD lệch pha với hệ thống menu → engine chạy logic "hội thoại xong" (đứng dậy). `EldenConvenienceMod` ghi đè đúng điều kiện này (`MenuCloseExpr(9)` trong `SoulsIds.ESDEdits`). Kết luận "airtight" ban đầu (OpenEnhanceShop tự ép đứng dậy) là SAI.

**Không hijack state vanilla:** ban đầu chiếm "Tailoring Shop" / "Dupe Shop" để thử; sau chuyển hẳn sang state mới vì không có lý do kỹ thuật phải chiếm, và chiếm thì mất tính năng gốc nếu Grace đặc biệt nào dùng. State mới cũng không bị bug đứng dậy.

**Cờ toàn cục `PATCHED` sai:** mục chỉ hiện ở Grace đầu tiên. Thay bằng `already_has_custom_items` (quét `entry_events` tìm `add_talk_list_data` với message ID của mod) - mỗi graph mới tự được patch đúng 1 lần. Cái giá: mỗi lần vào Grace `Box::leak` ~1KB; Grace biến thể thiếu anchor sẽ leak ở mọi lần ghé (chưa gặp).

## Text tuỳ biến (`msg_hook.rs`)

Hook hàm `get_message(msg_repository, unknown, bnd_id, msg_id) -> const wchar_t*` (leaf function, prologue 15 byte `cmp;jae;cmp;jae;mov`; 2 AOB lấy từ `erdGameTools`). Detour trả chuỗi UTF-16 tự soạn khi `bnd_id == 33` và `msg_id` ở dải `90000001+` (không đụng ID thật lẫn dải `69010000-69015000` của erdGameTools); còn lại replay 5 byte đầu rồi nhảy tiếp. 2 lệnh `jae` trong prologue phải tính lại đích tuyệt đối khi relocate (đảo thành `jb` + nhảy tuyệt đối). Có thể gọi ngược chính `get_message` để lấy text vanilla (`22130001` Nâng cấp, `26000010` Mua, `20000011` Bán) rồi nối hậu tố → đúng ngôn ngữ game; cuối cùng cả 3 mục chỉ dùng text vanilla.

**Fallback (game 1.17):** prologue của `get_message` đổi nên hook "fail closed" và mục hiện chữ trắng; thêm `effective_msg_ids()`: hook không cài được thì dùng thẳng 3 ID vanilla làm tham số `add_talk_list_data`. Chưa test lại sau thay đổi này.

## Mua tất cả thương nhân: giật 1-2 giây (chấp nhận, không sửa)

`OpenRegularShop(0, 9999999)` ("All Shops" của Elden-Ring-CT-TGA) gộp mọi thương nhân; hàm gốc build đồng bộ ~1277 dòng `ShopLineupParam` (178 vũ khí + 453 giáp + 12 bùa + 498 vật phẩm + 135 + 1) nên giật. Thu hẹp range (`shop_lot_range`, min/max ID thật) **vô tác dụng**: các loại item trải gần hết `0..9999999`. Bản đầu chỉ mở Twin Maiden Husks (`101800-101899`, ~100 dòng) không giật. `ermerchant.dll` (Glorious Merchant) không dùng `OpenRegularShop` mà dựng cây hội thoại `AddTalkListData` + lệnh "give item" trực tiếp (không xác định được lệnh vì ghi đè state trong ESD sống, cần debugger runtime). Hướng hết giật: tự dựng menu + give item, hoặc giảm phạm vi thương nhân.

## UnlockShop

`GraceMenu.UnlockShop`: `ShopLineupParam.event_flag_for_release = u32::MAX` cho mọi dòng (script "Access all shop inventory.cea"), snapshot giá trị gốc, hot-reload 2 chiều bằng `ReloadKey`; tắt từ đầu thì không chờ `SoloParamRepository`.

## Hướng đã bỏ

- Thêm menu bằng `ezstate_event`/native từ thread riêng với transition "luôn đúng" (các lần thất bại đầu tiên: không nhúng lệnh ESD thật vào `entry_events`).
- `Misc.GraceOnTorrent` (ngồi Grace khi đang cưỡi Torrent): chỉ xoá `isGrayoutForRide`/`isInvalidForRide` của `ActionButtonParam` row `6100` là không đủ, EMEVD event của action `6100` có xử lý "đang cưỡi" riêng (site chưa mở: tự xuống ngựa rồi kẹt). Cần viết EMEVD event mới như `EldenConvenienceMod` (họ clone row 6100 sang ID mới + event mới, đồng thời tăng `radius`); workspace không có công cụ EMEVD.

# SpiritMultiplier: vấn đề còn mở (Reforged, Seamless, warp)

**Status 2026-10-08: chuyển từ TODO.md sang đây. Reforged Spirit Fury / Fortune of the Spiritcaller đã sửa ở 1.1.4; các mục dưới còn mở.**

## Reforged

Cơ chế Reforged (CHANGELOG.txt của ERR): bấm lại Ash khi spirit đang ở ngoài = **kích nộ (Spirit Fury)**, không phải cho về; cho về bằng item Spirit-Severing Blade. "Aggro" = Spirit Fury, "un-aggro'ed" = trạng thái bình tĩnh mặc định.

- **Spirit gọi ra mặc định ở trạng thái đỏ (Fury)** thay vì trắng (puffintoast, 2026-10-01). Muốn: gọi ra trắng như spirit thường, hoặc tuỳ chọn bảng màu trong ini. Chưa rõ màu đỏ do mod (`GhostColor=false` tắt vfx 70000/70010/70020 của Reforged, chỉ còn màu Fury) hay do thiết kế Reforged - cần test `GhostColor=true` vs `false`.
- **MultiSpirit + Fury**: hiện Fury 1 nhóm thì mọi Ash đều xám. Cần tìm cách Reforged đánh dấu "đang Fury" và cái gì làm Ash xám (SpEffect? `reforged.dll`?); lưu ý cân bằng.
- **NoRestResummon không có tác dụng sau Spirit-Severing Blade**: là luật của Reforged gốc (sau Blade ô Ash xám tới khi ngồi grace), không phải do mod; NoRest chỉ gỡ khoá vanilla (`summonedEventFlagId` của bia đá). Không phải SpEffect triệu hồi chuẩn: `9540/9541/9545/9547/9560` (GameSystemCommonParam `onBuddySummon_*`) trong Reforged gần như rỗng → nghi event flag của script Reforged (EMEVD/ESD) hoặc `reforged.dll`.
- **Hướng lâu dài**: nhận diện Reforged (vd. `reforged.dll` trong module đã nạp) rồi nhường thao tác bấm lại Ash cho Spirit Fury. MultiSpirit chiếm đúng thao tác đó (patch `DoSummon`, `DisappearAll`, `GetBuddyState`/`CanUseItem`/UI), có thể đè hook của `reforged.dll`. Cần tự tái hiện (fortune Spiritcaller, rơi từ boss Road's End Catacombs).

## Seamless Co-op: spirit mất màu ma (DVM20, 2026-09-30; tạm dừng 2026-10-02)

- Đã loại trừ (probe `GhostProbe`, code ở `git stash` "seamless ghost probe"): PhantomParam 200/201 giống hệt offline; spirit vẫn có SpEffect 295000; module vfx (`ChrIns.modules +0xB8`, field +0x44) vẫn chọn 200; team (47 offline / 1 Seamless) không phải nguyên nhân.
- Nguyên nhân: Seamless hook đầu `sub_1403F0C20` (2.7.1.0) = hàm "PhantomParam của ChrIns" (override +0x540 → vfx +0x44 → mặc định theo chr type). Tìm bằng diff code game trong RAM vs `eldenring.exe` (363 chỗ chỉ có ở Seamless). Seamless còn hook `sub_1403F1C90` (get team).
- Đặt `phantom_param_override = 200` trên spirit: không có tác dụng (hook bỏ qua cả override).
- Hướng còn lại: đổi rel32 của lệnh `E9` Seamless đặt ở đầu `sub_1403F0C20` sang stub của mình - spirit (vfx +0x44 >= 0, trong summon_buddy_chr_set) trả id đó, còn lại nhảy tiếp vào hook Seamless; chỉ cài sau khi Seamless đã hook xong (byte đầu = E9).

## Spirit dịch chuyển tới người chơi khi bị bỏ lại

Hoãn từ trước 1.0.0; chỉ chỉnh `buddyWarp_*` không có tác dụng.

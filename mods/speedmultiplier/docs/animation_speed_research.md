# SpeedMultiplier: nghiên cứu "núm" tốc độ và SpeedProbe

**Status 2026-10-07: HOÀN THÀNH, đã dùng trong bản phát hành.** Kết luận: ghi
`CSChrBehaviorModule.animation_speed` mỗi frame, theo nhóm của animation đang
chạy; game không tự reset field này. Code: `mods/speedmultiplier/src/speed.rs`;
công cụ dò `src/probe.rs` (chỉ còn là log debug, `SpeedProbe=false` mặc định).

## Các "núm" tốc độ đã tìm được

| Núm | Tác dụng | Nguồn |
|---|---|---|
| `CSChrBehaviorModule.animation_speed` (+0x17C8, chuỗi `ChrIns+0x190 → +0x28 → +0x17C8`) | Tăng tốc **toàn bộ** animation của một nhân vật: đi, chạy, đánh, cast, lăn, uống bình | Bảng CE TGA (`Hero/Animation/Animation Speed`), `Elden_Ring_game_tools` (`SyncPlayerAnimationSpeed`, ghi lại mỗi 5 ms nếu giá trị khác) |
| HKS `act(SetMovementScaleMult, x)` (2001) | Chỉ tăng quãng đường di chuyển (root motion), animation giữ tốc độ cũ. HKS vanilla gọi mỗi frame trong `Move_onUpdate` | soulsmods/EldenRingHKS, các mod Nexus 8408, 9506, 9603 và "Fast Torrent" 3022 (sửa `c8000.hks`) |
| `CSChrPhysicsModule.motion_multiplier` / `CSChrBehaviorDataModule.hks_root_motion_mult` | Nghi là đích của act 2001 | suy đoán, đã xác định ở dưới |
| `CSChrBehaviorDataModule.hks_animation_speed_multiplier` (+0x310) | Chưa rõ | fromsoftware-rs |
| `MagicParam.analogDexterityMin/Max` | Game nội suy tốc độ cast theo DEX giữa hai mốc này | Paramdex |

Các hướng đã loại:
- **SpEffect:** `SpEffectParam` không có field tốc độ animation; tốc độ theo
  SpEffect trong các mod khác đều làm trong HKS.
- **RideParam:** không có field tốc độ cho Torrent.
- **Sửa TAE (`AnimSpeedGradient`):** cách các mod Nexus dùng cho tốc độ đánh và
  cast, nhưng quá cứng cho một mod DLL có file cấu hình.

IDA (db 2.7.1.0): không tìm thấy lệnh float nào truy cập thẳng `[reg+17C8h]`;
vtable của behavior module chỉ có 2 slot và hàm update không đi qua vtable. Không
lần ra ai ghi `animation_speed`, nên chuyển sang kiểm chứng runtime bằng probe.

## SpeedProbe (`src/probe.rs`)

Bản nghiên cứu đo ở 3 thời điểm trong frame (`ChrIns_PreBehavior` = PB,
`ChrIns_PrePhysics` = PP, `ChrIns_PostPhysics` = PO), mỗi lần đọc 4 field
(`a` = `animation_speed`, `m` = `motion_multiplier`, `r` = `hks_root_motion_mult`,
`h` = `hks_animation_speed_multiplier`), cho cả người chơi và Torrent; log khi anim id
đổi hoặc một giá trị đổi (tối đa một lần mỗi 250 ms). Phím F6 (`ProbeForce*`) xoay
vòng ép từng field bằng một giá trị ở một thời điểm đã chọn.

### Kết quả test

- `a` (`animation_speed`): ép ở PB vẫn giữ nguyên tới PP/PO, **game không ghi đè
  trong frame**; Torrent cũng giữ được. Giá trị đọc lại luôn bằng giá trị đã ép, nên chỉ cần ghi
  khi giá trị cần thay đổi. Đây là hướng chính.
- `m` (`motion_multiplier`): trên người chơi bị engine reset về 1.0 ngay giữa PB và
  PP mỗi frame, ghi ở PB vô dụng; trên Torrent thì không bị reset.
- `r` (`hks_root_motion_mult`): **đích của HKS `act(SetMovementScaleMult)`**. Log thấy
  đúng 0.98 khi chạy tới (`020110`/`020210`), khớp `Move_onUpdate` vanilla, và 0.00
  trong nhóm anim `005xxx`. Ghi trong bước behavior (giữa PB và PP), nên muốn đè thì
  phải ghi ở PrePhysics.
- `h` (`hks_animation_speed_multiplier`): HKS tự đặt 0.60 trong `a885_040005/040006`
  (AoW), còn lại 1.0.
- **AoW "không dùng được" khi ép tốc độ** là do hết FP, không phải lỗi của mod
  (người dùng xác nhận): `040005/040006` là anim "hết FP"; đầy FP thì AoW chạy đủ
  `040000 → 040001 → 040020` và nhanh gấp đôi khi `a=2.0`. Lúc đầu từng nghi do mod vì
  biến thể này cũng xuất hiện khi `a=1.0`; probe được thêm FP vào log để kiểm chứng.
- Đính chính một giả định: `0680xx` là ngồi nghỉ ở Site of Grace (FP hồi 11 đến 100), không phải uống
  bình; `050190` là anim gọi Torrent (xuất hiện ngay trước khi Torrent hiện ra), không
  phải dùng item.

### Bản đồ đuôi anim ban đầu (đã tinh chỉnh ở `action_groups.md`)

Prefix `aXXX` của anim id không chỉ đổi theo vũ khí mà còn theo loại hành động
(đứng/di chuyển `a003`/`a010`, đánh `a060`, AoW `a885`/`a435`), nên không thể lọc theo
prefix cho mọi thứ. Phần **đuôi** (`anim_id % 1_000_000`) mới có quy luật rõ: `000000`
đứng yên; `020xxx`/`022xxx` đi/chạy/dừng; `027xxx` lăn/lùi; `030000`-`039999` đòn
đánh; `040xxx`/`045xxx` AoW và phép; `050xxx` dùng item; `10xxxx`/`12xxxx`/`20xxxx` lên /
cưỡi / xuống Torrent. Torrent: `000000` đứng, `0020xx`/`0022xx` khi đang được cưỡi,
`020000`-`020004` di chuyển.

## Torrent

Torrent là entry có `npc_param_id == 80020000` trong
`WorldChrMan.summon_buddy_chr_set` (slot #0 của người chơi local). Chỉ các entry
`Active`/`ReadyForActivation` mới được dereference (cùng quy tắc với `regen.rs` của
spiritmultiplier).

## Game chỉ có một tốc độ animation cho cả nhân vật

Hardware breakpoint (DR0 đặt từ mod, VEH ghi RIP; Arxan không phản ứng) cho thấy game
chỉ đọc `animation_speed` ở **một chỗ**: getter `sub_14036834A` (`movss
xmm0,[rcx+18h]`, rcx = behavior+0x17B0, nằm trong vùng Arxan), gọi từ `sub_14041DCA0`
(update behavior mỗi frame: `dt × animation_speed × behavior+0x15C0` rồi đưa vào
hkbCharacter; hệ số `+0x15C0` được đặt lại 1.0 mỗi frame từ `+0x15C4`). Hệ quả: không
thể tăng riêng lớp animation nửa thân trên (xem `action_groups.md`, mục uống bình khi
chạy). Chỗ đọc duy nhất này cũng là điều Seamless Co-op khai thác khi hook lại (xem
`seamless.md`).

## Kế hoạch triển khai đã chọn

Mỗi frame (`ChrIns_PreBehavior`), đặt `animation_speed` của người chơi theo nhóm của
anim đang chạy và của Torrent theo nhóm riêng; chỉ ghi khi giá trị khác. Phần ép field
của probe (`ProbeForceKey/Value/Stage`, đo 3 thời điểm, ghi 4 field ứng viên) đã gỡ
sau khi mod chạy tốt (2026-09-29); probe chỉ còn log khi anim đổi, ở
`ChrIns_PostPhysics` (sau khi `speed.rs` đã ghi): anim id, nhóm, `animation_speed` thật
của người chơi (kèm FP) và của Torrent. Dòng log ở frame đầu của một anim mới đôi khi
còn ghi hệ số của nhóm cũ vì task của probe và của `speed.rs` cùng nằm ở
`ChrIns_PreBehavior` và thứ tự chạy giữa hai task không cố định; chỉ ảnh hưởng log.

# SpeedMultiplier

Mod DLL tăng tốc độ của người chơi: **di chuyển** (của người chơi và
**Torrent**), **tốc độ tấn công** và **tốc độ thi triển phép**.

**Trạng thái hiện tại (2026-09-29, `1.0.0`, Nexus mod 11173):**
`src/speed.rs` đặt `animation_speed` theo nhóm hành động (`[Speed]`:
`PlayerAll` ghi đè `PlayerMovement`/`Roll`/`Attack`/`Skill`/`Cast`/`Item`/
`Other`, cộng `Torrent`). Đã test trong game: chạy đúng.
`src/probe.rs` (`SpeedProbe=false`) chỉ còn là log debug.

## Nghiên cứu ban đầu (2026-09-29)

Các "núm" tốc độ đã tìm được (tên field theo fromsoftware-rs):

| Núm | Tác dụng | Nguồn |
|---|---|---|
| `CSChrBehaviorModule.animation_speed` (+0x17C8, chuỗi `ChrIns+0x190 → +0x28 → +0x17C8`) | Tăng tốc **toàn bộ** animation của 1 nhân vật: đi, chạy, đánh, cast, lăn, uống bình | Bảng CE TGA (`Hero/Animation/Animation Speed`), `.docs/Elden_Ring_game_tools` (`SyncPlayerAnimationSpeed`: ghi lại mỗi 5 ms nếu giá trị khác) |
| HKS `act(SetMovementScaleMult, x)` (2001) | Chỉ tăng quãng đường di chuyển (root motion), animation giữ tốc độ cũ. HKS vanilla gọi mỗi frame trong `Move_onUpdate` (0.96/0.98) | soulsmods/EldenRingHKS, các mod Nexus 8408, 9506, 9603, "Fast Torrent" 3022 (sửa `c8000.hks`) |
| `CSChrPhysicsModule.motion_multiplier` / `CSChrBehaviorDataModule.hks_root_motion_mult` | Nghi là đích mà act 2001 ghi vào, chưa rõ là field nào | Suy đoán |
| `CSChrBehaviorDataModule.hks_animation_speed_multiplier` (+0x310) | Chưa rõ | fromsoftware-rs |
| `MagicParam.analogDexterityMin/Max` | Game nội suy tốc độ cast theo DEX giữa 2 mốc này | Paramdex |

Các hướng đã loại:
- **SpEffect:** `SpEffectParam` không có field tốc độ animation. Tốc độ
  theo SpEffect trong các mod đều được làm trong HKS.
- **RideParam:** không có field tốc độ cho Torrent.
- **Sửa TAE (`AnimSpeedGradient`):** là cách các mod Nexus dùng cho tốc độ
  đánh và cast, nhưng quá cứng cho 1 mod DLL có file ini.

IDA (db 2.7.1.0): không tìm thấy lệnh float nào truy cập thẳng
`[reg+17C8h]`. Vtable của behavior module chỉ có 2 slot, hàm update không
đi qua vtable. Không lần ra được ai ghi `animation_speed`, nên chuyển sang
kiểm chứng runtime bằng probe.

Hướng dự kiến: ghi `animation_speed` mỗi frame trong 1 task của engine,
với hệ số tuỳ theo loại animation. Loại animation phân theo phần đuôi
`anim_id % 1_000_000` (`aXXX_YYYYYY`: `XXX` = nhóm tư thế cầm vũ khí,
`YYYYYY` = loại hành động dùng chung cho mọi vũ khí), không lọc theo từng
vũ khí. Torrent là entry có `npc_param_id == 80020000` trong
`WorldChrMan.summon_buddy_chr_set` (slot #0 của người chơi local, xem
README của spiritmultiplier).

## Tạo crate, viết SpeedProbe (2026-09-29)

`src/probe.rs`, bật bằng `SpeedProbe=true` (mặc định trong bản nghiên cứu):

- **Đọc ở 3 thời điểm trong frame:** `ChrIns_PreBehavior` (PB),
  `ChrIns_PrePhysics` (PP), `ChrIns_PostPhysics` (PO). Mỗi lần đọc 4 field:
  `a` = `animation_speed`, `m` = `motion_multiplier`,
  `r` = `hks_root_motion_mult`, `h` = `hks_animation_speed_multiplier`.
  Đọc cho cả người chơi (`P`) lẫn Torrent (`T`).
- **Log:** mỗi khi anim id đổi, hoặc khi 1 giá trị đổi (tối đa 1 lần mỗi
  250 ms). Dạng:
  `P a031_030000 (31030000) PB[...] PP[...] PO[...] | force=...@PB mounted=...`.
- **`ProbeForceKey` (F6):** xoay vòng Off → AnimSpeed → MotionMult →
  RootMotionMult → HksAnimSpeed → Off. Field đang chọn được ép bằng
  `ProbeForceValue` (1.5) ở thời điểm `ProbeForceStage` (PreBehavior /
  PrePhysics / PostPhysics), trên cả người chơi lẫn Torrent. Khi chuyển
  sang field khác, field cũ được ghi lại 1.0 một lần. Có banner báo trong
  game.

Mục tiêu test:
1. Dải đuôi anim id của đi / chạy / chạy nhanh / lăn, của đánh (1 tay,
   2 tay, skill), và của cast (sorcery, incantation).
2. Field nào bị game ghi đè, ghi đè ở bước nào (giá trị ép ở PB mà đến PP
   hoặc PO đã mất).
3. Field nào thật sự làm nhanh lên trong game, và Torrent có ăn giống
   người chơi không.

## Kết quả test SpeedProbe lần 1 (2026-09-29)

Đã test: đi/chạy/lăn, đánh bằng 2 vũ khí, AoW, cưỡi Torrent. Ép
`AnimSpeed` rồi `MotionMult` (1.5, stage PB). Chưa thử `RootMotionMult`,
`HksAnimSpeed`, và chưa thấy anim cast phép nào rõ ràng.

**Field tốc độ:**
- `a` (`animation_speed`): ép ở PB vẫn giữ nguyên tới PP/PO, không bị game
  ghi đè trong frame. Torrent cũng giữ được. Là hướng chính.
- `m` (`motion_multiplier`): trên người chơi bị engine reset về 1.0 ngay
  giữa PB và PP mỗi frame, nên ghi ở PB vô dụng. Trên Torrent thì không bị
  reset.
- `r` (`hks_root_motion_mult`): **đây mới là đích của HKS
  `act(SetMovementScaleMult)`**. Log thấy đúng 0.98 khi chạy tới
  (`020110`/`020210`), khớp `Move_onUpdate` vanilla, và 0.00 trong nhóm
  anim `005xxx`. Được ghi trong bước behavior (giữa PB và PP), nên muốn đè
  thì phải ghi ở PrePhysics.
- `h` (`hks_animation_speed_multiplier`): HKS tự đặt 0.60 trong
  `a885_040005/040006` (AoW), còn lại 1.0.

**Anim id:** prefix `XXX` không chỉ đổi theo vũ khí mà còn theo loại hành
động (đứng/di chuyển `a003`/`a010`, đánh `a060`, AoW `a885`/`a435`), nên
càng không thể lọc theo prefix. Phần đuôi thì có quy luật rõ:

| Đuôi | Hành động |
|---|---|
| `000000` | Đứng yên |
| `020xxx`, `022xxx` | Đi / chạy / dừng |
| `027xxx` | Lăn / lùi |
| `029xxx` | Chưa rõ (đổi vũ khí / cầm 2 tay?) |
| `030000`–`030040`, `031900`, `032000`–`032010` | Đòn đánh (chuỗi R1, R2…) |
| `040xxx`, `045xxx` | AoW |
| `050xxx` | Dùng item |
| `005xxx` | Chưa rõ (HKS đặt `r=0`) |
| `10xxxx`, `12xxxx`, `20xxxx` | Lên / cưỡi / xuống Torrent |

Torrent: `000000` đứng, `0020xx`/`0022xx` khi đang được cưỡi,
`020000`–`020004` di chuyển.

**AoW "không dùng được" khi đang ép tốc độ:** lần dùng không ép, anim là
`040000 → 040001 → 040020` (khoảng 3 giây). Lúc ép thì là
`040005 → 040006` (khoảng 1 giây, `h=0.60`). Nhưng biến thể
`040005/040006` xuất hiện **cả khi `a=1.0`** (pha `MotionMult`, mà `m` còn
bị reset mỗi frame). Nên chưa chắc do mod. Nghi là anim "hết FP" của
skill. Probe đã thêm FP vào log để kiểm chứng.

Sửa probe: stage đang ép giờ được đọc **trước khi ghi**, để thấy giá trị ép
ở frame trước có sống qua cả frame không. Log thêm `fp=cur/max`.

## Kết quả test SpeedProbe lần 2 (2026-09-29)

- **AoW "không dùng được" là do hết FP, không phải lỗi mod** (người dùng
  xác nhận). `040005/040006` là anim "hết FP". Đầy FP thì AoW chạy đủ
  `040000 → 040001 → 040020` và nhanh gấp đôi khi `a=2.0`.
- **`animation_speed` giữ được qua các frame:** stage PB giờ đọc trước khi
  ghi, và giá trị đọc được luôn là 2.00. Game không reset field này, nên
  chỉ cần ghi khi giá trị cần thay đổi.
- Đuôi anim mới thấy: `0680xx` = ngồi nghỉ ở Site of Grace (FP 11 → 100
  đúng lúc `068050`/`068011`), `026xxx` = chưa rõ (lùi?), `004xxx`,
  `049995`, `0052xx`/`0053xx` (`r=0`) = chưa rõ.
- **Đính chính:** ban đầu tôi đoán `0680xx` là uống bình, nhưng người dùng
  xác nhận chưa dùng item hay bình lần nào, nên FP hồi là do nghỉ ở grace.
  `050190` (lần test 1) là anim gọi Torrent: nó xuất hiện ngay trước khi
  Torrent hiện ra, không phải dùng item. Vì vậy chưa có dữ liệu cho item
  hay bình.
- Vẫn chưa có anim cast phép, và chưa test `RootMotionMult`/Torrent.

## Bộ lọc theo nhóm hành động - `src/speed.rs` (2026-09-29)

Mỗi frame (`ChrIns_PreBehavior`), đặt `animation_speed` của player theo
nhóm của anim đang chạy, và của Torrent theo `Torrent`. Chỉ ghi khi giá trị
khác, vì game không tự reset field này. Tạm dừng khi probe đang ép 1 field
bằng F6. Mỗi dòng log của probe giờ có thêm tên nhóm.

| Key ini | Đuôi anim | Mặc định (bản test) |
|---|---|---|
| `Movement` | `000000`, `020000`–`026999` | 1.5 |
| `Roll` | `027xxx` | 1 |
| `Attack` | `03xxxx` | 1.5 |
| `Skill` | `04xxxx` | 1.5 |
| `Other` | mọi đuôi còn lại | 1 |
| `Torrent` | mọi anim của Torrent | 1.5 |

Có hot reload. Giá trị bị kẹp trong khoảng 0.1–10.

Nhóm Item/bình **chưa có**, vì chưa có log item hay bình nào (xem đính
chính ở mục trên). Cast phép cũng chưa có dải. Cả 2 tạm nằm trong
`Other`. Các đuôi chưa rõ nghĩa (`004xxx`, `005xxx`, `029xxx`) cũng để ở
`Other`, không gộp vào `Movement`.

## Key tổng `Player` (2026-09-29)

Theo thiết kế của người dùng: thêm key tổng `Player` (mặc định 1). Giá trị
khác 1 áp cho **mọi** hành động của player và ghi đè toàn bộ key riêng
(`Movement`, `Roll`, `Attack`, `Skill`, `Other`). `Player=1` thì dùng key
riêng. `Torrent` không bị key tổng ảnh hưởng. Mọi key trong `[Speed]` giờ
đều mặc định 1 (vanilla), thay cho giá trị test 1.5 ở mục trên.

## Nhóm `Cast`, phân loại theo prefix TAE (2026-09-29)

Người dùng chép tay 2 danh sách TAE từ Nexus (articles 511 và 208: Nexus
chặn fetch tự động bằng Cloudflare) vào `.docs/Tae List.txt` và
`.docs/Elden Ring tae list updated for SOTE.txt`. Danh sách này cho biết ý
nghĩa của **prefix** `aXXX`:

| Prefix | Ý nghĩa |
|---|---|
| `a000`–`a016` | Tư thế đứng 1H/2H. `a000` còn gồm lăn, emote, chết, tương tác môi trường |
| `a020`–`a062` | Bộ chiêu theo loại vũ khí |
| `a100`–`a268`, `a831`/`a832`/`a839`/`a852`/`a935`/`a953` | Bộ chiêu riêng của từng vũ khí đặc biệt |
| `a400`–`a561` | Sorcery / incantation |
| `a600`–`a971` (trừ các số vũ khí ở dòng trên) | Sword Arts (AoW) |

Đối chiếu với log: `a435_045100/045110/045111` là **incantation Golden
Vow** (không phải AoW như tôi ghi ở mục test 1), còn `a885_040xxx` là AoW
**Carian Sovereignty**. Vậy cast phép và AoW **dùng chung đuôi `04xxxx`**,
không tách được bằng đuôi.

`speed::group_of` giờ xét prefix trước: `400..600` → `Cast`, `600..1000`
(trừ `WEAPON_TAES_IN_SKILL_RANGE`) → `Skill`. Phần còn lại vẫn phân theo
đuôi. Ini thêm key `Cast` (mặc định 1).

## Đổi tên key `[Speed]` (2026-09-29)

Người dùng thấy ini khó đọc, nên các key được đổi sang tiền tố rõ đối
tượng (PascalCase như các mod khác): `Player` → `PlayerAll`; `Movement`,
`Roll`, `Attack`, `Skill`, `Cast`, `Other` → `PlayerMovement`,
`PlayerRoll`, `PlayerAttack`, `PlayerSkill`, `PlayerCast`, `PlayerOther`;
`Torrent` giữ nguyên. Không cần bảng rename: mod chưa phát hành và ini
trong game chưa từng có các key cũ. Tên key trong các mục phía trên README
là tên cũ tại thời điểm viết.

## Test bộ lọc `[Speed]` trong game: chạy đúng (2026-09-29)

Người dùng test từng key một, qua hot reload. Theo xác nhận của người dùng,
mọi nhóm đều **hoạt động tốt**. Log (`SpeedProbe`, không bấm F6) khớp với
phân loại:

- `Movement`: `020xxx`/`022xxx`, đứng yên `000000`. `Roll`: `027010`,
  `027110`. `Attack`: `a060_030xxx`. `Skill`: `a885_040xxx`. `Cast`:
  `a435_045xxx` (Golden Vow).
- `Torrent` chạy tới 5.0 (`000000`, `0021xx`/`0022xx`, `005103`,
  `007xxx`, `020000`).
- Người dùng thử `PlayerRoll=0` thì bị kẹp về 0.1, đúng như thiết kế.
- Đuôi mới rơi vào `Other`: `050110`/`050111`/`050112`/`050210` (nghi là
  uống bình / dùng item, cùng họ `05xxxx` với `050190` gọi Torrent), và
  `100000`/`101xxx`/`104000`/`12xxxx` (lên / cưỡi / xuống Torrent).
- Dòng log ở frame đầu của 1 anim mới đôi khi còn ghi hệ số của nhóm cũ.
  Nguyên nhân: task của probe và của `speed.rs` cùng nằm ở
  `ChrIns_PreBehavior`, và thứ tự chạy giữa 2 task không cố định. Chỉ ảnh
  hưởng log, không ảnh hưởng hành vi.

Còn mở: nhóm "luôn 1.0" cho anim chết / bị túm (chờ người dùng đồng ý,
dùng chung danh sách anim chết với `fasterrevival` qua `common`), nhóm
Item, cache hệ số và slot Torrent trước khi phát hành.

## Nhóm `PlayerItem` (2026-09-29)

Test uống bình / dùng item / ném item (log 17:32–17:35): mọi animation
dùng item đều thuộc `a000_050xxx`. Bình: `050110 → 050111 → 050112`
(đưa lên / uống / hạ). `050110 → 050113` nghi là bình hết lượt: FP đứng yên
ở 70 suốt 3 lần uống. Item tiêu hao `050000`, ném `050010`, item khác
`050180`, gọi Torrent `050190` (Spectral Steed Whistle cũng là 1 item).
Ngồi nghỉ grace `068xxx` không thuộc họ này và vẫn ở `PlayerOther`. Danh
sách TAE trong `.docs` không có mục item (item nằm trong `a000`), nên chỉ
tách được bằng đuôi.

`group_of`: đuôi `50_000..=50_999` → `Item`, key `PlayerItem` (mặc định 1).

## Nới nhóm `PlayerItem` ra `05xxxx` (2026-09-29)

Người dùng báo throwing dagger và crystal dart không được tăng tốc. Log cho
thấy 2 item này dùng đuôi `055000`, nằm ngoài dải `050xxx` của mục trên,
nên bị xếp vào `Other`. Item khác (`050070`) thì đã nhận đúng hệ số. Trong
mọi log đến nay, không anim nào không phải item mang đuôi `05xxxx` (nghỉ
grace là `068xxx`, anim lúc load `063000`), nên dải được nới thành
`50_000..=59_999`.

## Tách anim gọi Torrent khỏi `PlayerItem` (2026-09-29)

Người dùng báo animation lên ngựa bị tăng tốc theo `PlayerItem`. Log:
`050190 → 101004 → 100000` (gọi Torrent → nhảy lên → đang cưỡi). Mục
"Nhóm `PlayerItem`" ở trên coi `050190` là item vì Spectral Steed Whistle
là item, nhưng trong game nó là bước đầu của động tác lên ngựa. Vì vậy
`group_of` giờ xếp riêng `50_190` vào `Other`, cùng các anim cưỡi ngựa
(`1xxxxx`/`2xxxxx`). Ghi chú của `PlayerItem` trong ini bỏ chữ "Torrent's
whistle".

## Gỡ phần ép field của SpeedProbe (2026-09-29)

Người dùng xác nhận mod chạy tốt. Phần nghiên cứu đã xong nhiệm vụ nên bị
gỡ: các key `ProbeForceKey`, `ProbeForceValue`, `ProbeForceStage`, việc đo
ở 3 thời điểm trong frame, và việc ghi 4 field ứng viên. `SpeedProbe` giữ
lại để debug, mặc định `false`. Probe giờ chỉ ghi log khi anim đổi, ở
`ChrIns_PostPhysics` (sau khi `speed.rs` đã ghi): anim id, nhóm,
`animation_speed` thật của player (kèm FP) và của Torrent.

`current_anim_id()` và `torrent()` chuyển từ `probe.rs` sang `speed.rs`
vì bộ lọc mới là nơi cần chúng. `speed.rs` bỏ `probe::is_forcing()`. Các
mục trên đây còn nhắc tới F6/`ProbeForce*` là lịch sử, không còn trong
code.

## Chuẩn bị phát hành 1.0.0: description, thumbnail (2026-09-29)

- `DESCRIPTION.bbcode` viết theo `template/`: intro kiểu "A DLL mod that
  lets you...", Features (chỉ những gì người chơi được), Notes (khoảng
  0.1–10, và cảnh báo rằng key tổng cũng tăng tốc cả lúc bị đánh, cưỡi ngựa,
  nghỉ grace). Credits ghi 2 bài TAE list trên Nexus (208, 511), bảng CE
  TGA (địa chỉ `animation_speed`) và fromsoftware-rs. Changelog `1.0.0
  Initial release`: mod chưa có trang Nexus nên không có changelog thật để
  đối chiếu. Chưa có mục "Why this mod?" (tuỳ chọn).
- `THUMBNAIL_PROMPT.txt`: prompt ảnh bìa dựng từ 3 screenshot cùng 1 nhát
  greatsword. Tư thế giữa là nhân vật chính, 2 tư thế còn lại làm dư ảnh
  mờ dần. Người dùng đã tạo ảnh xong, rồi xoá file prompt khỏi crate.
- `Cargo.toml` `version` 0.1.0 → 1.0.0, khớp mục changelog.

## Giá trị mặc định cho 1.0.0, sửa intro/Features (2026-09-29)

Người dùng chốt mặc định: `PlayerMovement=1.2`, `PlayerAttack=1.2`,
`PlayerRoll=1.1`, `PlayerSkill=1.2`, `PlayerCast=1.2`, `Torrent=1.3`.
`PlayerAll`, `PlayerItem`, `PlayerOther` giữ 1. Các mục trên README ghi
"mọi key mặc định 1" là trước thay đổi này. Intro của `DESCRIPTION.bbcode`
đổi theo câu của người dùng. Features giờ liệt kê đúng 6 mục kèm % mặc
định, bỏ các bullet về key tổng, SotE và hotkey reload.

## Deploy 1.0.0 lên Nexus (2026-09-29)

Đã deploy: **mod 11173** (Speed Multiplier, category Gameplay, internal id
`18610093304741`). File `SpeedMultiplier` 1.0.0 (mod file `8049852`) là
file đầu tiên của trang, nên được tạo bằng `POST /mod-files` (trang mới
chưa có update group nào, giống lần đầu của spiritmultiplier). Build từ
commit `e51b1a0`. Changelog Nexus: `1.0.0: Initial release.` Đã ghi vào
`manifest.json`.

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
chặn fetch tự động bằng Cloudflare) vào 2 file (từ 2026-10-07 đã gộp thành `docs/tae-ids.md`). Danh sách này cho biết ý
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

## Chuyển cấu hình sang TOML: `SpeedMultiplier.toml` (2026-10-02)

Yêu cầu từ Nexus (Lwingr, Linear Convergence): hệ số tốc độ khác nhau theo
SpEffect đang có trên người chơi. Cấu hình kiểu đó là **danh sách quy tắc**,
mà ini của workspace không chứa nổi: `common::config` gộp mọi `[Section]`
vào 1 namespace (phải đánh số `PlayerMovement01`, `02`...), không có mảng,
và `migrate` dựa theo tên key. Người dùng chọn chuyển hẳn mod này sang
TOML trước khi làm tính năng, để còn mở rộng.

**`common` (dùng chung, các mod ini không đổi gì):**
- `shared/src/toml_config.rs` mới: `TomlConfig<T>` - mỗi mod khai báo 1
  struct serde (`#[serde(default, deny_unknown_fields)]`, `Default` = giá
  trị template). Key thiếu lấy mặc định; key sai tên / sai kiểu là lỗi kèm
  số dòng (thông báo của crate `toml`); file lỗi không bao giờ thay cấu hình
  đang chạy (khởi động: chạy bằng `T::default()`, reload: giữ bản cũ).
  Khởi động: chưa có file thì ghi template; có rồi thì thêm key/bảng còn
  thiếu từ template bằng `toml_edit` (giữ comment, giá trị, bố cục của
  người dùng) - tương đương `config::migrate` của ini. Cấu hình là snapshot
  `Arc<T>`: tick lấy 1 lần rồi đọc field, không tra chuỗi mỗi giá trị.
- `logger::set_enabled(bool)`: mod TOML tự bật/tắt log (mặc định vẫn đọc
  `LogFile` trong ini).
- `reload::run_with(key_fn, reload_fn)`: watcher F5 cho cấu hình không phải
  ini; reload lỗi thì log lỗi + banner "Config error - see log" thay cho
  "Config reloaded". `reload::run(ini)` giờ gọi lại `run_with`.
- Dependency workspace mới: `serde`, `toml` 0.8, `toml_edit` 0.22.
- `scripts/build-mod.ps1`: tìm `<Mod>.toml` trước, rồi `<Mod>.ini`, và
  đóng gói đúng file đó.

**SpeedMultiplier:**
- `SpeedMultiplier.ini` → `SpeedMultiplier.toml` (cùng 3 bảng `[General]`,
  `[Speed]`, `[Logging]`, key giữ PascalCase như ini theo lựa chọn của người
  dùng). Không tự chuyển giá trị từ ini cũ (người dùng: ini còn ít key) -
  người cập nhật từ 1.0.0 được file mặc định mới, ini cũ không còn được đọc.
- `src/config.rs` mới: struct `Config`, `init`/`get`/`reload`; test kiểm
  template == `Config::default()`, key thiếu, key gõ sai (báo dòng), và số
  nguyên (`Torrent = 2`) vẫn nhận làm f32.
- `speed.rs`: `Group::key()` (tên key ini) → `Group::speed(&Speed)`;
  `multiplier(key)` → `clamped(value)` (giữ khoảng 0.1–10, NaN/inf → 1).
- `DESCRIPTION.bbcode`: "ini" → "config file", file cài đặt
  `SpeedMultiplier.toml`.

Tính năng ghi đè theo SpEffect (`[[Override]]`) chưa làm - xem `TODO.md`.

**Test trong game (người dùng): đúng.** Lần đầu ghi `ReloadKey = 0x75` báo
lỗi "invalid type: integer `117`, expected a string" - trong TOML `0x75`
không có nháy là số nguyên, ini thì nhận cả hai. Thêm
`common::toml_config::key_name` (nhận chuỗi hoặc số, số → chuỗi thập phân
cho `parse_virtual_key`). Sau đó: F6 (`0x75`) reload được, tốc độ đổi theo;
gõ sai `PlayerRol` → log lỗi dòng 16 kèm danh sách key hợp lệ, cấu hình cũ
giữ nguyên.

## `[[Override]]`: tốc độ khác khi người chơi có SpEffect (2026-10-02)

Yêu cầu Nexus (Lwingr, Linear Convergence). Thiết kế chốt với người dùng
sau vài vòng:
- Tên `[[Override]]` (không phải `Rule`/`Condition`/`Profile`): nói đúng
  việc nó làm - ghi đè giá trị `[Speed]` khi khớp. `[[...]]` = danh sách
  bảng TOML, viết bao nhiêu khối cũng được.
- **Chỉ 1 điều kiện `SpEffect`**, nằm thẳng trong khối (bỏ bảng con `When`
  đã đề xuất): mod chỉ đổi tốc độ anim, SpEffect đã gồm buff/debuff/
  talisman; HP/vũ khí/giờ trong game là thừa.
- `SpEffect = 1234` hoặc `[1234, 1235]` = có **bất kỳ** id nào (người dùng
  từng cân nhắc "mảng = phải có đủ" rồi bỏ). Id số hoặc chuỗi; mảng rỗng,
  id không phải số, thiếu `SpEffect`, key lạ = lỗi kèm dòng.
- **Xếp chồng**: mọi override khớp đều áp từ trên xuống, trùng key thì khối
  viết sau thắng (2 buff độc lập - 1 tăng lăn, 1 tăng đánh - có tác dụng
  cùng lúc, không phải viết thêm khối gộp như kiểu "khối đầu tiên thắng").
  `PlayerAll` cũng đi qua quy trình này. Torrent theo SpEffect của người chơi.

Code:
- `config.rs`: struct `Override` (key tốc độ là `Option<f32>`, không
  `#[serde(default)]` ở struct để thiếu `SpEffect` là lỗi), visitor
  `sp_effect_ids`, `Config::effective_speed(has_sp_effect)` trả tốc độ cuối
  + chỉ số các override đang bật. 6 test mới (khớp bất kỳ id, xếp chồng,
  không override nào, id dạng chuỗi, các lỗi).
- `speed.rs::apply`: mỗi frame lấy `special_effect.entries()` của người
  chơi (bỏ qua nếu không có override nào) → `effective_speed`; log 1 dòng
  `Speed: active overrides: #1, #2` / `none` khi tập override đổi.
- Probe: `EffectProbe` mới (mặc định false) log SpEffect người chơi khi đổi
  (`P SpEffect +[thêm] -[mất] now [...]`); lúc đầu nằm trong `SpeedProbe`,
  người dùng tách ra cùng ngày. Không quảng cáo trong comment - người cần
  tính năng này tự biết tra id (người dùng).
- Template: mô tả + ví dụ `[[Override]]` dạng comment giữa `[Speed]` và
  `[Logging]`; chú giải `PlayerOther` đổi thành "any action not covered by
  the keys above". File toml đã có của người dùng không nhận comment mới
  (`toml_config` chỉ thêm key thiếu).

**Test trong game (người dùng): hoạt động hoàn hảo.** Probe cho Golden Vow
(phép) = SpEffect `1660000`/`1660001`/`1660002`, khoảng 80 giây (Smithbox
xác nhận; bản kỹ năng vũ khí là `1730`, bản item `20503170`). Log: override
bật/tắt theo buff (`#1` → `none`), 2 override cùng lúc (`#1, #2`), reload
giữa chừng vẫn đúng.

## Đòn chí mạng: nhóm riêng `PlayerCritical` (2026-10-02)

Người dùng điều tra đâm lén / riposte (Nexus, invadersnes64 muốn tăng tốc
riposte; người dùng: đòn chí mạng phải khớp anim kẻ địch). `SpeedProbe`:
- Riposte (sau parry `a692_044840`): `a023_031700`.
- Backstab: `a023_031719` → `a023_031710` (các anim `32xxxx` ở lần đầu là
  lúc cúi người lén tới).
- Đuôi `0317xx` nằm trong dải Attack (`030000`-`039999`) → bị tăng theo
  `PlayerAttack`. Ở 2.0 người chơi rút kiếm xong trong khi kẻ địch vẫn đang
  ngã (người dùng xác nhận) - anim nạn nhân không được mod tăng tốc.

Sửa: nhóm `Critical` = đuôi `031700`-`031799`, đặt trước Attack. Bản đầu
ép cố định 1.0 (kể cả dưới `PlayerAll`/`[[Override]]`); cùng ngày người
dùng muốn thành key → `PlayerCritical` (mặc định 1.0, comment cảnh báo lệch
nhịp nếu > 1), hành xử như mọi key `Player*` (`PlayerAll` khác 1 đè lên,
`[[Override]]` đặt được). Test `group_of` (031700/031710/031719 = Critical,
030000/031699/031800 = Attack). **Test trong game: đúng.** Chưa thử vũ khí
khác `a023` (vd. dao găm có đòn chí mạng riêng).

## Uống bình khi đang chạy: giới hạn, không sửa (2026-10-02)

Nexus (13586927500): uống bình khi di chuyển không được tăng tốc. Điều tra:
- `SpeedProbe` + `play_time` của `anim_queue` mỗi frame: uống khi đứng yên
  = `050110`→`050111`→`050112`, chạy đúng 2.0× theo `PlayerItem`. Uống khi
  chạy: anim uống **không vào `anim_queue`** (giữ anim chạy `0201xx`) → mod
  áp `PlayerMovement`. Người dùng xác nhận bằng bản DLL cũ: tăng
  `PlayerMovement` thì uống khi chạy nhanh lên.
- Anim nửa thân trên nằm ở `CSChrTimeActModule +0xD0` (`unkd0`), nhưng giá
  trị này **không bị xoá khi anim xong** - giữ tới khi anim chính đổi (dừng
  chạy). Bản thử dùng nó (Item khi `+0xD0` là anim item) làm tốc độ cao kéo
  dài tới lúc ngừng chạy. `+0xC8`/`+0xCC` (blend 0.2/0.233)/`+0xD4` không
  báo được anim còn chạy hay không.
- Hardware breakpoint (DR0, đặt từ mod, VEH ghi RIP; Arxan không phản ứng):
  game chỉ đọc `animation_speed` ở 1 chỗ - getter `sub_14036834A`
  (`movss xmm0,[rcx+18h]`, rcx = behavior+0x17B0, code Arxan) gọi từ
  `sub_14041DCA0` (update behavior mỗi frame: dt × `animation_speed` ×
  behavior+0x15C0 [hệ số đặt lại 1.0 mỗi frame từ +0x15C4] → hkbCharacter).
  Game có đúng 1 tốc độ anim cho cả nhân vật nên không thể tăng riêng lớp
  nửa thân trên.

Người dùng chốt: giới hạn hiện tại - uống khi chạy thuộc nhóm Movement. Gỡ
toàn bộ code thử (probe dò `+0xD0`/timer, `watch.rs`, `group_now`).

## Tách `PlayerMovement` → `PlayerWalk` / `PlayerRun` / `PlayerSneak` (2026-10-02)

Yêu cầu Nexus (cfzlbj): chỉnh riêng đi bộ / chạy. Trên bàn phím của người
dùng chỉ có 2 kiểu: đi bộ và chạy (giữ Shift) - không có sprint riêng.
`SpeedProbe` (đứng yên → đi → chạy → dừng → ngồi → ngồi đi → ngồi chạy):

| | Đứng | Lén (sneak) |
|---|---|---|
| Đi bộ | `0201xx`, dừng `0221xx` | `3201xx`, dừng `3221xx` |
| Chạy | `0202xx`, dừng `0222xx` | `3202xx`, dừng `3222xx` |
| Yên | `000000` | `300000`, vào tư thế `390000` |

Chữ số thứ 3 của đuôi = kiểu di chuyển. `0200xx` (`020010`) chỉ xuất hiện
khi đang dùng item lúc di chuyển (chân trong 2 đoạn đầu của uống bình) -
không phải "chạy thường" như đoán trước đó.

Sửa: bỏ `PlayerMovement` (người dùng: không cần nữa, phần còn lại rơi vào
`Other`), thêm `PlayerWalk` (`0201xx`/`0221xx`), `PlayerRun`
(`0202xx`/`0222xx`), `PlayerSneak` (đuôi `300000`-`399999`; trước đây rơi
vào `Other`, giờ mặc định 1.2). Đứng yên, nhảy/rơi, `0200xx`... → `Other`.
Tên ban đầu `PlayerCrouch`, người dùng đổi thành `PlayerSneak` cho hợp hơn.
`[[Override]]` nhận 3 key mới. File toml cũ có `PlayerMovement` sẽ báo
"unknown field" (bản TOML chưa phát hành nên chấp nhận). Features trong
DESCRIPTION: "Faster walking, running and sneaking". Torrent vẫn 1 key.

**Test trong game (người dùng): 3 key hoạt động đúng.**

## `[Player]` / `[Torrent]`, tốc độ Torrent và nhảy, migration theo phiên bản, `ReloadBanner` (2026-10-02)

**Bố cục `[Player]` / `[Torrent]`** (người dùng): `[Speed]` với key
`Player*` / `Torrent` → `[Player]` (`All`, `Walk`, `Run`, `Sneak`, `Jump`,
`Roll`, `Attack`, `Critical`, `Skill`, `Cast`, `Item`, `Other`) và
`[Torrent]` (`All`, `Walk`, `Run`, `Jump`, `Other`). `[[Override]]` dùng
dotted key `Player.Roll = 1.5` / `Torrent.Run = 2` (bảng con
`[Override.Player]` cũng được). Comment cùng dòng (người dùng viết lại
template). `All` khác 1 đè mọi key của bảng đó.

**Torrent** (`SpeedProbe` khi cưỡi; người cưỡi chạy `12xxxx`): đứng
`000000`, đi `0021xx` (002100, 002110), chạy - giữ Shift, cả lần bấm thêm
và giảm tốc - `0022xx` (002220, 002200, 002221, 002210), quẹo/xoay
`0051xx`, nhảy `0061xx` (006110 đứng, 006130 đi hoặc chạy - cùng id) và tiếp
đất `0074xx` (007400, 007451) → `Walk` / `Run` / `Jump` / `Other`. Mặc định
`All 1.0, Walk 1.0, Run 1.3, Jump 1.0, Other 1.0` (trước đây mọi anim 1.3).
`SpeedProbe` ghi nhóm cho dòng `T`.

**Nhảy của người chơi**: cất nhảy `2020xx` (202000 đứng, 202020 đi,
202030/202040 chạy), tiếp đất `2021xx` (202100, 202115, 202126) →
`Player.Jump` (mặc định 1.0; trước rơi vào `Other`). Chỉ lấy dải đã thấy,
không cả `20xxxx`.

**Key lạ / đổi tên (`common::toml_config`)**: ini chuyển key lạ vào
`[Legacy]`, TOML trước đây báo lỗi. Giờ lúc khởi động key không có trong
template được gỡ và ghi lại thành comment ở cuối file (log WARN); F5 vẫn
báo lỗi kèm dòng (người dùng đang sửa file). `[[Override]]` được giữ
(`Migration::keep`). Bản đầu dùng danh sách đổi tên phẳng (`Speed.PlayerRun`
→ `Player.Run`...) - đã chuyển file test thật của người dùng đúng - nhưng
người dùng hỏi trường hợp nhảy phiên bản (1.0.0 → 1.2.0): danh sách phẳng
phụ thuộc thứ tự, không tách/quy đổi được key, xoá 1 cặp là mất giá trị.
Thay bằng **migration theo phiên bản**: `General.ConfigVersion` (phiên bản
hiện tại = giá trị trong template), `STEPS[i]` đưa file từ `i+1` lên `i+2`,
mỗi bước chạy đúng 1 lần mỗi file; helper `rename_key` / `get_value` /
`remove_value` / `set_value`; file không có version = 1; file mới hơn mod
(hạ cấp) không bị đụng. **Bước đã phát hành không bao giờ được sửa** - sửa
sai bằng bước mới. SpeedMultiplier: `ConfigVersion = 1` = định dạng TOML
phát hành đầu (1.1.0), `STEPS` rỗng, bỏ danh sách đổi tên (bản TOML chưa
phát hành, không ai có file `[Speed]`); test `steps_cover_every_version`.

**`ReloadBanner`** (`[General]`, mặc định true): tắt banner "Config
reloaded"; banner lỗi luôn hiện (tắt luôn thì reload hỏng sẽ im lặng).
`common::reload::run_with` nhận thêm `banner: Fn() -> bool` (đọc sau reload);
mod ini (`reload::run`) luôn hiện như cũ.

**Test trong game (người dùng): mọi thứ hoạt động.**

## Giá trị mặc định chỉ nằm trong template (2026-10-02, nhánh `feat/modmenu`)

Trước: mỗi giá trị mặc định viết 2 lần - template `SpeedMultiplier.toml` và
`impl Default` trong `config.rs` - giữ khớp bằng test (người dùng đổi
`Walk`/`Sneak` 1.1 phải sửa cả code + test). Bước đầu của việc làm menu
(menu sẽ lấy metadata từ struct, mặc định từ template).

Giờ (cấu hình xếp lớp - template là lớp dưới, file người dùng là lớp trên):
- `common::toml_config::parse` đọc 2 lượt: (1) chỉ file người dùng → lỗi
  kèm số dòng như cũ (key thiếu nhận giá trị rỗng của `#[derive(Default)]`);
  (2) giá trị người dùng đặt lên template (bảng trộn từng key, giá trị /
  mảng / `[[...]]` thay nguyên) → cấu hình thật. File lỗi → chạy bằng
  `defaults(template)`, không còn `T::default()`.
- SpeedMultiplier: bỏ ~60 dòng `impl Default`; struct
  `#[derive(Default, Serialize)]` (Default = rỗng, không bao giờ là cấu hình
  đang chạy); `config::template()` = mặc định. Test
  `template_sets_every_field` (`common::toml_config::template_missing_keys`)
  báo field nào template chưa khai báo; các test so với `template()` thay vì
  ghi cứng số.
- Kiểm chứng: đổi `Torrent.Run` trong template 1.3 → 1.7 thì mặc định trong
  code đổi theo, 17 test vẫn qua; bỏ `EffectProbe` khỏi template → test báo
  `template lacks ["Logging.EffectProbe"]`.
- Hiệu năng: đọc 2 lượt chỉ chạy lúc khởi động / reload (cỡ trăm µs), mỗi
  frame vẫn chỉ lấy snapshot `Arc`. DLL 864 KB → ~1.04 MB (merge + `Serialize`)
  - người dùng chốt kích thước vài MB không đáng lo, chỉ quan tâm hiệu năng.

**Test trong game (người dùng): hoạt động.**

## Seamless Co-op: tốc độ không có tác dụng - Seamless hook getter của `animation_speed` (2026-10-03)

Nexus (DrKSolo): mod không chạy với Seamless Co-op. Người dùng tái hiện trên
cả 1.0.0 và bản hiện tại (`Walk = 3`): log ghi `speed=3.00` nhưng trong game
không nhanh hơn.

Nguyên nhân: game chỉ đọc `CSChrBehaviorModule.animation_speed` ở 1 chỗ
(hardware breakpoint 2026-10-02): update behavior `sub_14041DCA0` gọi thunk
`sub_140417EA0: jmp <getter>` (getter trả `[rcx+0x18]`, rcx = behavior
+0x17B0). Bản so code trong RAM vs exe khi có Seamless (2026-10-02, lúc dò
màu ma của SpiritMultiplier; lưu ở `.docs/seamless-diff/`) có đúng
`0x140417EA1` = rel32 của `jmp` đó → Seamless đổi đích sang code của nó,
giá trị mod ghi không bao giờ được đọc.

Sửa (`src/seamless.rs`): tìm lệnh gọi thunk bằng AOB (`48 8D 8F B0 17 00 00
E8 ?? ?? ?? ?? 0F 28 F0 F3 0F 59 B7 C0 15 00 00`, call ở +7); mỗi tick, nếu
`jmp` của thunk không còn trỏ vào `eldenring.exe` (bị DLL khác hook), đổi
rel32 sang stub: owner của behavior (`[rcx-0x17A8]`) là người chơi / Torrent
(`OWN_CHRS`, `speed.rs` cập nhật mỗi frame) → trả `animation_speed`; nhân vật
khác → nhảy tiếp vào hook cũ. Chỉ đổi rel32 (như SpiritMultiplier 1.1.4) và
làm sau khi Seamless đã hook → không đụng signature của Seamless. Chỉ tra
module khi đích thay đổi. Stub đã kiểm bằng Capstone.

**Test trong game (người dùng): hoạt động**, cả khi 2 bản game kết nối co-op
(Sandboxie). Giới hạn: hình ảnh không đồng bộ giữa 2 máy - bên join đánh
nhanh (`Attack = 5`), bên host thấy đòn đó chậm hơn: mỗi máy chỉ tăng tốc
nhân vật của chính nó, nhân vật người khác trên máy mình chạy theo game /
Seamless.

## Version thật trong thuộc tính file DLL (2026-10-05)

Trước đây DLL không có version resource nên Properties → Details trống và
dòng "Loaded modules" trong log (`common::diag`) hiện `v0.0.0.0`; `version`
trong `Cargo.toml` không vào DLL (`cdylib` của Rust không tự nhúng). Thêm
`build.rs` (crate `winresource`, cần `rc.exe` của Windows SDK) nhúng
File/Product version lấy từ `CARGO_PKG_VERSION`, cộng `ProductName`,
`FileDescription`, `InternalName`, `OriginalFilename` khai báo trong
`[package.metadata.winresource]` của `Cargo.toml`. Hệ quả: version trong
`Cargo.toml` (phải khớp Nexus, xem bump version cùng changelog) giờ chính
là version hiện trong Properties và trong log.

## Thang: nhóm riêng `PlayerLadder` (2026-10-06)

Người dùng lấy log `SpeedProbe` (anim `a000_028xxx`, trước đó rơi vào `Other`):
- Vào thang: `028999` (chung cho mọi lần); đứng yên trên thang: `028030`.
- Leo lên: `028100` (vào từ đầu dưới) → `028011` ↔ `028012` (vòng lặp) → `028013`.
- Leo xuống từng bậc: `028020` → `028021` ↔ `028022` → `028023`.
- Trượt xuống: `028020` → `028030` → `028000` → `028001` → `028002`.
- Đòn đánh / đá khi đang bám thang: `028040`, `028045` (chưa phân biệt được
  cái nào là đòn nào, người dùng chốt không cần); `028036` = chuyển tiếp.

Thêm nhóm `Ladder` = đuôi `028000`-`028999` (`speed.rs::group_of`, đặt sau
Roll `027xxx`, không đụng dải nào khác) và key `Ladder` trong `[Player]`
(`SpeedMultiplier.toml`, `config.rs::Player` / `PlayerOverride`). Mặc định
1.0 (đúng hành vi cũ: trước đây cả dải này là `Other` = 1.0), nên người dùng
cũ không thấy khác gì; `All` khác 1 vẫn đè và `[[Override]]` đặt được như
mọi key `Player*`. File cũ tự được thêm key mới khi khởi động (không cần
migration). Lý do tách riêng: tăng tốc leo/trượt có thể lệch nếu game tính
quãng đường theo root motion hay timer riêng - cho người dùng tự chọn.
Test `ladder_anims_are_their_own_group` (13 đuôi đã thấy trong log + 2 biên
`027999` = Roll, `029000` = Other). **Test trong game (người dùng, 2026-10-06): hoạt động** với `Ladder` ≠ 1.

## Placidusax's Ruin: phần phóng tia không theo `Cast` (2026-10-06)

Nexus (bloodaxis, 2026-10-04): tia laser của Placidusax's Ruin lệch nhịp khi
chỉnh tốc độ niệm phép. `SpeedProbe` (`Cast = 1.2`, niệm 2 lần):
`a451_045100` → `a451_045110` (~6 giây) → `a000_004050` (`Other`, FP trừ ở
đây) → `a000_000000`. Prefix `a451` chỉ của thần chú này (TAE list). Nghi:
tia là hiệu ứng game tạo ra, chạy theo thời gian riêng, nên anim bị tăng
tốc thì lệch - cùng cơ chế với đòn chí mạng (`PlayerCritical`).

Sửa (`speed.rs::group_of`): prefix `451` + đuôi `045110`-`045119` →
`Other` (mặc định 1.0), đặt trước nhánh Cast; phần niệm đầu `045100` vẫn
`Cast`. Hệ quả: tốc độ phần tia theo key `Other` (và `All` nếu khác 1),
không có key riêng. Test `placidusax_ruin_beam_is_not_cast`. **Test trong
game (người dùng, 2026-10-06): hết lệch** với `Cast` = 0.7 / 1.0 / 1.2 / 1.5 / 5.0.
Log: `045110` luôn ~8-9 giây bất kể `Cast` (trước đó ở `Cast = 1.2` chỉ ~6
giây), nghĩa là phần tia chạy 1.0 như ý định. Lưu ý khi đọc log: dòng
`a451_045110 ... Other speed=X` ghi `X` = giá trị `Cast` của anim trước (log
in trước khi áp), không phải tốc độ thật của anim đó.

## Tốc độ theo equip load: điều kiện `EquipLoad` trong `[[Override]]` (2026-10-06)

Nexus (Sannh, 2026-10-05: "Possible to tie this to equip load?"). Làm tầng
rẻ nhất: thêm điều kiện cạnh `SpEffect`, không thêm bảng mới. Tên key: lúc đầu là
`Load`, đổi ngay thành `EquipLoad` (cùng ngày, chưa phát hành) vì `Load` đơn
lẻ dễ nhầm với "load game"/"load config"; "Equip Load" là đúng thuật ngữ
trong game.

Điều tra: `SpeedProbe` ghi thêm dòng `P weight_type=N max_equip_load=X` khi
1 trong 2 đổi (`probe.rs`). Người dùng đổi trang bị từ nhẹ tới quá tải:
`ChrCtrl.weight_type` = 1, 2, 3, 4 theo đúng thứ tự (0 = chưa nạp dữ liệu,
lúc mới vào game); anim lăn đổi theo (`027110` ở 2, `027120` ở 3,
`027130` ở 4). `max_equip_load` đổi độc lập (59.8 → 56.1 khi đổi talisman).
Chốt: 1 `Light`, 2 `Medium`, 3 `Heavy`, 4 `Overweight`.

Cách dùng: `EquipLoad = "Heavy"` hoặc `EquipLoad = ["Light", "Medium"]` (không phân
biệt hoa thường) trong `[[Override]]`. Override có `SpEffect`, `EquipLoad`, hoặc
cả hai (cả hai phải khớp); không có điều kiện nào = lỗi (báo dòng). Mọi
override khớp vẫn xếp chồng như cũ. Lớp tải chưa rõ (`weight_type` = 0 hay
ngoài 1-4) thì override có `EquipLoad` không khớp. Code: `config.rs`
(`LoadClass`, `Override` đọc qua `RawOverride` + `TryFrom` để kiểm tra có
điều kiện), `speed.rs::apply` đọc `main_player.chr_ins.chr_ctrl.weight_type`
mỗi frame. Test `override_load_*`, `weight_type_maps_to_a_class`, lỗi
`needs a condition` / `not a class`.

Lưu ý: `weightmultiplier` đổi tổng tải nên lớp tải cũng đổi theo (hợp lý).
Anim đi bộ khi **quá tải** là `020020` (không nằm trong `Walk`
`020100`-`020199`) nên `Walk` không áp lúc đó - xem `Other`. Tải liên tục
theo % chưa làm. **Test trong game (người dùng, 2026-10-06): hoạt động.**

## Changelog 1.2.0 + bump version (2026-10-06)

Đối chiếu Nexus (mod 11173, API): có 1.0.0 và 1.1.0, khớp bản local. Thêm
mục `1.2.0` vào `[Changelog]` của `DESCRIPTION.bbcode` (nhóm thang, điều
kiện equip load, sửa tia Placidusax's Ruin) và 2 dòng ở Features (thang,
equip load); `version` trong `Cargo.toml` = `1.2.0`, build lại cập nhật
`Cargo.lock`. Chưa deploy lên Nexus.

Cập nhật cùng ngày: mặc định `Ladder` trong file mẫu là **1.2** (như `Attack`/`Skill`/`Cast`) theo người dùng; Features trong `DESCRIPTION.bbcode` ghi "Faster ladder climbing and sliding (20% by default)". Chỉ áp cho file cấu hình mới - file có sẵn giữ giá trị đang ghi (key đã tồn tại không bị đè).

## Đổi tên thư mục `crates/` thành `mods/` (2026-10-07)

Tái tổ chức workspace: thư mục chứa các mod đổi từ `crates/` sang `mods/` (tên chung chung, không gắn với Rust - sau này 1 mod có thể chỉ là dự án Smithbox, không có `Cargo.toml`). Đường dẫn của mod này giờ là `mods/<tên>`; `path = ../../shared` trong `Cargo.toml` giữ nguyên vì độ sâu thư mục không đổi. Hành vi runtime không đổi.

Các đường dẫn `crates/<mod>/...` trỏ tới file của chính repo này ở phần trên đã được cập nhật thành `mods/...`; riêng các đường dẫn `crates/eldenring/...` là của repo `fromsoftware-rs`, và dòng đổi tên `crates/teleporttest` là lịch sử nên giữ nguyên.

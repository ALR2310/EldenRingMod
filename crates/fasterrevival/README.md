# FasterRevival

Mod DLL cho Elden Ring: rút ngắn thời gian từ lúc nhân vật hết máu tới
màn hình "YOU DIED" và hồi sinh, **không sửa file animation** (`.anibnd`
/ `.tae`). **Chưa có tính năng chính** - hiện chỉ có công cụ dò
`Debug.DeathProbe` phục vụ nghiên cứu.

## Tạo crate (2026-09-28)

Ý tưởng lấy từ mod **FasterDeathAnimation** của 0-F (tên GitHub; trên Nexus là
[0R4X](https://www.nexusmods.com/profile/0R4X))
([Nexus 3367](https://www.nexusmods.com/eldenring/mods/3367),
[GitHub](https://github.com/0-F/FasterDeathAnimation), repo không có
LICENSE - chỉ tham khảo ý tưởng, không copy code). Mod gốc là 1 tool
console .NET 7 chạy offline: đọc `c0000.anibnd.dcx`, mở `a00.tae`, với mỗi
animation có cả 2 event TAE type 0 (`ChrActionFlag`):

- action **12 = Kill Character** (event đầu tiên có `StartTime != 0`)
- action **20 = Send Ghost Info** (gửi thông tin vết máu/bóng ma)

thì dời cả 2 lên đầu animation (event 12 về `t=0`, event 20 giữ nguyên
khoảng cách so với event 12), giữ nguyên thời lượng; `EndTime >= 100` coi
là "tới hết animation" và không đụng tới. Animation ngã vẫn phát bình
thường, chỉ mốc logic "chết" được kéo lên sớm. Tên action lấy từ
`TAE.Template.ER.xml` của DSAnimStudio và comment trong
`fromsoftware-rs` (`ChrCtrlModifierActionFlags`: action 20
`SEND_GHOST_INFO`, action 40 `TEMPORARY_DEATH_STATE`).

Vì sao làm bản DLL thay vì patch file:

- Mod gốc thay nguyên `c0000.anibnd.dcx`, file mà rất nhiều mod animation
  khác cũng thay - xung đột, người dùng phải tự chạy tool lại trên bản đã
  mod.
- Bản build mod gốc trên Nexus lúc tải về (2023-04-15) còn cũ hơn commit
  sửa lỗi đường dẫn output (`8b49722`, 2023-04-22).

Hướng đang cân nhắc (cần IDA, db ở `dumps/ida/2.7.1.0/`):

1. Patch TAE trong RAM sau khi game nạp `c0000` (port 1:1 thuật toán gốc).
   Rủi ro: event trỏ vào bảng thời gian dùng chung, sửa thẳng float có
   thể làm lệch event khác - phải trỏ event sang float riêng.
2. Hook chỗ game duyệt event TAE / xử lý `ChrActionFlag` 12/20.
3. Không cần reverse: tick mỗi frame, lúc chết thì tua `play_time` hoặc
   tăng tốc độ phát animation (khác mod gốc - animation cũng nhanh theo).

`Debug.DeathProbe=true` (mặc định bật trong giai đoạn nghiên cứu): từ frame
HP về 0 tới lúc hồi sinh, cứ 100ms (hoặc khi anim ID / `death_flag` đổi,
đánh dấu `*`) ghi 1 dòng log `anim=<ID> t=<play_time>/<anim_length>
death_flag=<..>` đọc từ `CSChrTimeActModule.anim_queue[read_idx]` và
`ChrIns.chr_flags1c5.death_flag`. Mục đích: lấy anim ID chết thật và thời
điểm game chuyển sang "đã chết" từ log thay vì đoán.

Cấu trúc: `src/lib.rs` (DllMain, ini, logger, reload), `src/death.rs`
(task mỗi frame: probe, và từ mục dưới cả `FastDeath`).

## IDA: tìm ra hàm kill, bản thử `FastDeath` (2026-09-28)

Phân tích `eldenring.exe` 2.7.1.0 (db lưu cố định ở
`dumps/ida/2.7.1.0/eldenring.exe.i64`). Luồng chết của game:

1. HP về 0 → bật `ChrIns+0x1C5` bit 7 (`chr_flags1c5.death_flag`).
   `sub_1403F84C0` mỗi frame: hoặc yêu cầu phát animation chết, hoặc (chết
   không có animation) gọi kill ngay.
2. Event TAE type 0 được `CSChrTaeAnimEvent` (vtable `0x142A372B0`) chia
   cho 2 handler: `sub_140427B30` (switch 143 case - action 20 bật
   `ChrCtrlModifier+0x18` bit 1 = `send_ghost_info`, khớp fromsoftware-rs)
   và `sub_140428DE0` - **chỗ xử lý action 12**: nếu cờ byte `event+0x18`
   bật và `sub_14041BA80(behavior module, 0)` (thunk vào vùng Arxan, chưa
   rõ) trả false thì gọi **`sub_1403EDA70(ChrIns*)` = hàm kill**. Action
   47 ("Kill Character (No Item Drop)") gọi bản gần giống `sub_1403EDB60`.
3. Hàm kill **idempotent**: thoát ngay nếu `ChrCtrl(+0x58)->modifier(+0xC8)
   +0x24` bit 0 đã bật, và chính nó bật bit đó đầu tiên
   (`sub_1403FCD90`) - nên gọi sớm thì lần gọi sau của event TAE thành
   no-op. Bỏ qua `ChrType` 3 (Ghost) / 10 (BloodstainGhost).

Vì vậy không cần sửa TAE trong RAM hay hook: `death.rs` tự gọi hàm kill
(AOB `KILL_CHR_PATTERN`, duy nhất trên cả 2.7.1.0 `0x1403EDA70` lẫn 2.6.2.0
`0x1403ED840` - không cần RVA theo version) khi `death_flag` bật, cờ
killed chưa bật, và anim ID đã đổi so với lúc thấy `death_flag` (= animation
chết đã bắt đầu), hoặc sau 300ms nếu anim ID không đổi (điều kiện này đã
thay - xem mục "Test bị quái đánh" bên dưới). Ini: `[Death]
FastDeath=true`. Animation chết vẫn phát, chỉ mốc "chết" kéo lên sớm - giống
mod gốc.

Khác mod gốc / chưa rõ:

- Mod không kiểm tra điều kiện `sub_14041BA80` của game trước khi kill
  (chưa gọi vì là thunk vào vùng Arxan) - nếu điều kiện đó từng chặn kill
  trong trường hợp nào đó, mod sẽ kill sai lúc đó.
- Chưa dời action 20 "Send Ghost Info" (handler còn 1 điều kiện
  `sub_1404C8730` chưa hiểu) - nếu animation bị cắt trước khi event 20 chạy,
  có thể mất bloodstain/ghost cho người chơi khác (online).
- Gọi hàm kill từ task `FrameBegin`, không phải từ trong lượt cập nhật
  nhân vật như game - cần test.

`DeathProbe` giờ log thêm `killed=` (cờ bit 0 trên) thay cho
`death_flag` (luôn true trong lúc log). **Chưa test trong game.**

## Nhận biết chết bằng HP thay vì `death_flag`, thêm `ToggleKey` + log thời gian (2026-09-28)

Test đầu tiên trong game (2.7.1.0): chết nhưng log **không có dòng nào**
sau "Death: task registered" - task `FrameBegin` không bao giờ thấy
`death_flag` bật. Giả thuyết: game bật rồi xử lý/xoá cờ này ngay trong lượt
cập nhật nhân vật của cùng frame (`sub_1403F84C0` xoá bit 7 khi đã kill),
nên task chạy đầu frame không kịp đọc. Đổi sang **`hp <= 0`**
(`CSChrDataModule.hp`) làm điều kiện "đang chết"; `death_flag` chỉ còn được
log ra để kiểm chứng giả thuyết.

Để so sánh bật/tắt:

- `[Death] ToggleKey=F6`: đảo `FastDeath` ngay trong game, hiện banner
  "FasterRevival: ON/OFF". Chỉ là ghi đè tạm - nhấn `ReloadKey` thì quay về
  giá trị trong ini.
- Mỗi lần chết luôn log (không cần `DeathProbe`): `Died (FastDeath ON/OFF)`,
  `Death registered by the game: +X ms` (lúc cờ killed bật - bất kể do mod
  hay do event TAE), và `Respawned: X ms after HP hit 0.` - so 2 con số này
  giữa ON và OFF là thấy chênh lệch.

Test lại bằng **nhảy vực**: log đã hoạt động (`hp <= 0` bắt được), nhưng
ON (8063 ms) và OFF (8128 ms) như nhau - đúng như mong đợi cho kiểu chết
này: cờ killed đã bật ngay frame đầu HP về 0 (game kill ngay, không chờ
event), anim `4100` (2s) rồi `4101`, `death_flag` luôn đọc được `false`
(khớp giả thuyết ở trên). Chạy tool gốc trên `c0000.anibnd.dcx` trong repo
của 0-F để xem animation nào thật sự có event action 12: 91 animation,
thuộc các nhóm `017xxx` (chết thường khi bị đánh), `070xxx` (chết trong
đòn tóm/throw), `117xxx`, `075003`, `000150` - không có `4100/4101`. Vậy
mod (cả bản gốc lẫn bản này) chỉ có tác dụng với các kiểu chết đó; chết do
rơi vực không đổi. Còn phải test: chết do bị quái đánh.

## Test bị quái đánh: chạy đúng, sửa điều kiện kích hoạt (2026-09-28)

Chết do bị quái đánh (animation chết `17002` = `a000_017002`, dài 6.667s):

| | Game ghi nhận chết | Hồi sinh |
|---|---|---|
| ON | +53 ms (`Killed early` ở +36 ms, t=0.000) | 8379 ms |
| OFF | +5997 ms (t=5.964) | 14392 ms |

→ nhanh hơn ~6s mỗi lần chết, không crash/kẹt. Gọi hàm kill từ task
`FrameBegin` chạy ổn.

Log lần OFF lộ lỗi của điều kiện cũ ("anim đổi so với lúc HP về 0, hoặc sau
300ms"): sau HP 0, anim còn đổi `10020113` → `10020110` (đòn đánh đang dở)
1 frame **trước khi** sang `17002` - nếu đang ON thì đã kill trước khi
animation chết bắt đầu. Đổi thành: chỉ kill khi anim đang phát thuộc đúng
danh sách animation có event action 12 (`has_kill_event`: `a000_017xxx`,
`070xxx`, `075003`, `117xxx`, `000150` - lấy từ output của tool gốc), bỏ
mốc 300ms. Anim ID trong `CSChrTimeActModule` có dạng
`category * 1_000_000 + id`.

Test lại sau khi sửa (chết giữa lúc đang tấn công, anim `20032010`/`20032000`
→ 33ms sau mới sang `17022`, dài 4.000s): mod chờ đúng tới `17022` mới
kill.

| | Game ghi nhận chết | Hồi sinh |
|---|---|---|
| ON (2 lần) | +49/+50 ms | 8411/8421 ms |
| OFF | +3827 ms (t=3.795) | 12227 ms |

Mức tiết kiệm = gần trọn độ dài animation chết (~6s với `17002`, ~3.8s với
`17022`); khi ON, hồi sinh luôn ~8.4s bất kể animation - phần còn lại là
màn "YOU DIED" + fade + load, ngoài tầm của mod. Sau animation chết game
luôn chuyển sang anim `18xxx` tương ứng (`17022` → `18022`).

Chết trong đòn tóm (test ngay sau đó): HP về 0 giữa anim `70440`
(t=1.933/4.167) → kill ngay frame đó; đòn tóm vẫn diễn đủ `70440` →
`70441` → `70442` (tư thế nằm cuối, giữ tới lúc hồi sinh), hồi sinh 8395
ms, không kẹt. Lưu ý: tool gốc chỉ có event kill ở `70441`, không ở
`70440` - dải `70_000..=70_999` của `has_kill_event` kill sớm hơn mod gốc
~0.7s ở trường hợp này. Giữ dải (không đổi sang danh sách ID chính xác) vì
chạy ổn và bắt được cả animation tóm do mod animation khác thêm vào. Cùng
đợt: `17032` (4.8s) và `17012` (4.667s) kill ở +33 ms như mong đợi.

Còn chưa test: khi đang cưỡi Torrent.

Không dời action 20 "Send Ghost Info" nữa (bỏ khỏi danh sách việc cần
làm): chơi có mod DLL thì phải tắt EAC, không vào được online chính thức,
nên không có bloodstain/ghost nào để gửi cho người khác.

## Bỏ khoảng dừng ở màn "YOU DIED" (2026-09-28)

Nguồn: **FasterRespawn** của ImAxel0 ([Nexus 501](https://www.nexusmods.com/eldenring/mods/501),
[GitHub](https://github.com/ImAxel0/EldenRing-FasterRespawn-Mod), MIT). DLL
C++ 15 dòng: AOB `74 ?? F3 0F 10 19`, đổi `74 06` (`jz`) → `EB 06` (`jmp`)
- decompile bản DLL và source công khai khớp hoàn toàn. Tác giả không ghi
cú nhảy đó điều khiển gì; tra trong IDA (2.7.1.0, mẫu duy nhất ở
`0x1405A7F23`; 2.6.2.0 ở `0x1405A70D3`):

- Nằm trong `sub_1405A7E00`, 1 bước của `CSDeathRestartEvent::Start`
  (`sub_14059F670`, có chuỗi tên hàm đó). Bước này hẹn giờ callback
  `sub_1405A7FA0` qua `sub_1405942D0(a2, 4006, cb, delay, 0, 4, 1)`.
- `delay` (xmm3) = `*sub_140D2FE90()` - con trỏ tới param slot 143
  (`MenuCommonParam`), row ID 0, trường đầu tiên =
  **`soloPlayDeath_ToFadeOutTime`** (tên paramdef/Smithbox, hiển thị
  "[YOU DIED] Fade Out Duration", vanilla 3.8; accessor fromsoftware-rs
  `solo_play_death_to_fade_out_time`). Patch ép luôn nhánh `xorps xmm3, xmm3`
  → delay = 0.
- Bước kế (stage 5, trong `sub_1405A7FA0`) còn 1 delay khác đọc từ
  `GameSystemCommonParam[0]+0x37C` - FasterRespawn không đụng, chưa xác
  định là trường nào.

Vì delay chính là 1 giá trị param nên không cần patch code: `src/fade.rs`
ghi thẳng `MenuCommonParam[0].soloPlayDeath_ToFadeOutTime` = 0 khi
`FastDeath` bật, trả về giá trị gốc (snapshot trước lần ghi đầu, tránh cộng
dồn) khi tắt - theo đúng công tắc/F6 của `death.rs`, không thêm key ini
riêng (theo ý người dùng). Chỉ ghi khi trạng thái đổi, và chỉ khi
`main_player` đã có (gate `SoloParamRepository`). Không đụng
`partyGhostDeath_ToFadeOutTime` (co-op, vanilla 3.3), giống bản gốc. **Chưa test trong
game.**

## Hồi sinh chậm ~8-20s ở 1 số lần: không phải lỗi (2026-09-28)

Sau khi thêm phần bỏ khoảng dừng "YOU DIED", các lần chết thường hồi sinh
~4.4-4.7s (trước đó ~8.4s - khớp với 3.8s vanilla của
`soloPlayDeath_ToFadeOutTime`), nhưng 1 số lần 7.8s / 14s / 20s. Đo từ log:
animation đứng yên (fade) ở ~1.7s và khoảng load (không có frame) ~2.3-2.5s
là **như nhau** ở mọi lần; phần chênh là thời gian game vẫn chạy frame sau
fade. Đã thử giả thuyết stage 5 của `CSDeathRestartEvent` (chỉ chờ khi
`is_death_penalty_skip`, delay từ `GameSystemCommonParam[0]+0x37C`) bằng
log tạm: `penalty_skip=false`, `+0x37C = 0` → sai. Nguyên nhân thật (người
dùng tự nhận ra): chết gần **Stake of Marika** → game hiện menu chọn hồi
sinh ở Stake hay ở grace cuối, thời gian đứng ở menu được tính vào. Đã gỡ
đoạn log tạm đó.

## Chuẩn bị phát hành 1.0.0 (2026-09-28)

Người dùng test ổn toàn bộ (chết thường, chết giữa đòn tấn công, đòn tóm,
rơi vực, Stake of Marika). Tổng kết hồi sinh sau chết thường: vanilla
12.2-14.4s → ~4.2-4.7s khi bật.

- `Debug.DeathProbe` mặc định đổi sang `false` (log mỗi 100ms trong lúc
  chết quá dày cho người dùng thường; vẫn bật được để chẩn đoán). Các dòng
  `Died` / `Killed early` / `Death registered` / `Respawned` vẫn luôn log.
- Thêm `DESCRIPTION.bbcode` cho Nexus (chưa có trang mod, nên changelog bắt
  đầu từ 1.0.0 - không có changelog thật nào để đối chiếu qua API). Credit
  0-F (FasterDeathAnimation) và ImAxel0 (FasterRespawn, MIT).

## Bỏ `ReloadKey`, `FastDeath`, `ToggleKey` - mod luôn bật (2026-09-28)

Theo ý người dùng: mod cài vào là chạy, không có gì để chỉnh, nên 3 key đó
thừa (`FastDeath`/`ToggleKey` chỉ phục vụ việc so sánh bật/tắt lúc test,
đã xong). Gỡ luôn phần code tương ứng:

- `death.rs`: bỏ `fast_death_enabled` (override F6 + reset theo
  `RELOAD_GENERATION`), dòng log `Died (FastDeath ON/OFF)` thành `Died:`.
- `fade.rs`: không còn trạng thái OFF nên không cần snapshot giá trị gốc
  để khôi phục - chỉ ghi `soloPlayDeath_ToFadeOutTime = 0` đúng 1 lần (log
  kèm giá trị cũ).
- `lib.rs`: không chạy `common::reload::run` nữa.

Ini chỉ còn `[Debug] DeathProbe` và `[Logging] LogFile`. Ini cũ của người
dùng còn các key đã bỏ thì vô hại (không ai đọc). Description Nexus bỏ 2
dòng tính năng hotkey.

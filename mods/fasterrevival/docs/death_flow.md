# FasterRevival: luồng chết của game và cách kéo mốc "chết" lên sớm

**Status 2026-09-28: HOÀN THÀNH, test ổn trong game** (chết thường, chết giữa
lúc đang tấn công, đòn tóm, rơi vực, chết gần Stake of Marika). Hồi sinh sau
chết thường từ 12.2-14.4s (vanilla) còn ~4.2-4.7s. Code:
`mods/fasterrevival/src/death.rs`. Phần bỏ khoảng dừng sau "YOU DIED" nằm ở
`respawn_fade.md`.

## Nguồn ý tưởng

**FasterDeathAnimation** của 0-F (trên Nexus là 0R4X,
[Nexus 3367](https://www.nexusmods.com/eldenring/mods/3367),
[GitHub](https://github.com/0-F/FasterDeathAnimation); repo không có LICENSE
nên chỉ tham khảo ý tưởng, không copy code). Mod gốc là tool console .NET 7
chạy offline: đọc `c0000.anibnd.dcx`, mở `a00.tae`, với mỗi animation có cả 2
event TAE type 0 (`ChrActionFlag`):

- action **12 = Kill Character** (event đầu tiên có `StartTime != 0`);
- action **20 = Send Ghost Info**.

thì dời cả 2 lên đầu animation (event 12 về `t=0`), giữ nguyên thời lượng;
`EndTime >= 100` coi là "tới hết animation", không đụng. Animation ngã vẫn
phát bình thường, chỉ mốc logic "chết" được kéo lên sớm.

Vì sao làm bản DLL thay vì patch file: mod gốc thay nguyên `c0000.anibnd.dcx`,
file mà rất nhiều mod animation khác cũng thay, nên xung đột và người dùng
phải tự chạy lại tool trên bản đã mod. (Bản build mod gốc trên Nexus lúc tải
về, 2023-04-15, còn cũ hơn commit sửa lỗi đường dẫn output `8b49722`.)

## Luồng chết của game (IDA, exe 2.7.1.0)

1. HP về 0: bật `ChrIns+0x1C5` bit 7 (`chr_flags1c5.death_flag`).
   `sub_1403F84C0` mỗi frame hoặc yêu cầu phát animation chết, hoặc (chết
   không có animation) gọi kill ngay.
2. Event TAE type 0 được `CSChrTaeAnimEvent` (vtable `0x142A372B0`) chia cho 2
   handler: `sub_140427B30` (switch 143 case; action 20 bật
   `ChrCtrlModifier+0x18` bit 1 = `send_ghost_info`, khớp fromsoftware-rs) và
   `sub_140428DE0`, **chỗ xử lý action 12**: nếu cờ byte `event+0x18` bật và
   `sub_14041BA80(behavior module, 0)` (thunk vào vùng Arxan, chưa rõ) trả
   false thì gọi **`sub_1403EDA70(ChrIns*)` = hàm kill**. Action 47 ("Kill
   Character (No Item Drop)") gọi bản gần giống `sub_1403EDB60`.
3. Hàm kill **idempotent**: thoát ngay nếu `ChrCtrl(+0x58)->modifier(+0xC8)
   +0x24` bit 0 đã bật, và chính nó bật bit đó đầu tiên (`sub_1403FCD90`),
   nên gọi sớm thì lần gọi sau của event TAE thành no-op. Bỏ qua `ChrType` 3
   (Ghost) và 10 (BloodstainGhost).

Vì vậy không cần sửa TAE trong RAM hay hook: mod tự gọi hàm kill.

## Cách mod kích hoạt

- Tìm hàm kill bằng AOB `KILL_CHR_PATTERN`, duy nhất trên cả 2.7.1.0
  (`0x1403EDA70`) lẫn 2.6.2.0 (`0x1403ED840`), nên không cần RVA theo phiên
  bản. Không tìm thấy thì log lỗi "game version not supported" và tắt phần kill
  sớm.
- Task lặp trên `FrameBegin`. Điều kiện "đang chết" là **`hp <= 0`**
  (`CSChrDataModule.hp`), vì `death_flag` không đọc được: game bật rồi xử lý và
  xoá cờ trong cùng lượt cập nhật nhân vật của frame đó, nên task đầu frame
  không bao giờ thấy nó (test đầu tiên không ra dòng log nào).
- Chỉ kill khi animation đang phát nằm trong tập có event action 12
  (`has_kill_event`: `a000_017xxx`, `070xxx`, `075003`, `117xxx`, `000150`, lấy
  từ output của tool gốc trên `c0000.anibnd.dcx`: 91 animation). Anim ID trong
  `CSChrTimeActModule` có dạng `category * 1_000_000 + id`.
- Mỗi lần chết luôn log: `Died`, `Killed early`, `Death registered by the game:
  +X ms`, `Respawned: X ms after HP hit 0`. `Debug.DeathProbe=true` thêm vết
  animation (anim ID, play time, độ dài) cứ 100ms.

## Hướng đã cân nhắc và bỏ

1. Patch TAE trong RAM sau khi game nạp `c0000` (port 1:1 thuật toán gốc): rủi
   ro event trỏ vào bảng thời gian dùng chung, sửa thẳng float có thể làm lệch
   event khác.
2. Hook chỗ game duyệt event TAE / xử lý action 12/20: không cần vì hàm kill
   idempotent.
3. Tua `play_time` hoặc tăng tốc độ phát animation: không cần reverse nhưng
   khác mod gốc (animation cũng nhanh theo).

Điều kiện kích hoạt cũ cũng đã bỏ: "anim đổi so với lúc HP về 0, hoặc sau
300ms" sai, vì sau HP 0 anim còn đổi (`10020113` → `10020110`, đòn đánh đang
dở) 1 frame **trước khi** sang `17002`, nên ở trạng thái ON đã kill trước khi
animation chết bắt đầu.

## Kết quả test (2.7.1.0)

Thời gian từ HP 0, trước khi bỏ khoảng dừng "YOU DIED" (xem `respawn_fade.md`):

| Kiểu chết | Anim | Game ghi nhận chết (ON / OFF) | Hồi sinh (ON / OFF) |
|---|---|---|---|
| Rơi vực | `4100` → `4101` | ngay frame đầu / ngay frame đầu | 8063 ms / 8128 ms (không đổi) |
| Bị quái đánh | `17002` (6.667s) | +53 ms / +5997 ms | 8379 ms / 14392 ms |
| Bị quái đánh giữa đòn tấn công | `17022` (4.0s) | +49-50 ms / +3827 ms | 8411-8421 ms / 12227 ms |
| Đòn tóm | `70440` → `70441` → `70442` | kill ngay frame đó | 8395 ms |

- Mức tiết kiệm bằng gần trọn độ dài animation chết (~6s với `17002`, ~3.8s với
  `17022`); khi ON hồi sinh luôn ~8.4s bất kể animation, phần còn lại là màn
  "YOU DIED", fade và load (xem `respawn_fade.md`).
- Rơi vực không đổi: game kill ngay frame đầu, và `4100/4101` không có trong
  91 animation có event kill. Vậy mod gốc cũng chỉ có tác dụng với các kiểu chết
  trong `has_kill_event`.
- Đòn tóm: đòn vẫn diễn đủ `70440` → `70441` → `70442` (tư thế nằm cuối giữ tới
  lúc hồi sinh), không kẹt. Tool gốc chỉ có event kill ở `70441`, không ở
  `70440`, nên dải `70_000..=70_999` kill sớm hơn mod gốc ~0.7s ở trường hợp
  này. Giữ cả dải (không đổi sang danh sách ID chính xác) vì chạy ổn và bắt
  được cả animation tóm do mod animation khác thêm.
- `17032` (4.8s) và `17012` (4.667s) kill ở +33 ms như mong đợi.
- Sau animation chết game luôn chuyển sang anim `18xxx` tương ứng (`17022` →
  `18022`).

## Khác mod gốc / giới hạn

- Mod **không kiểm tra** điều kiện `sub_14041BA80` của game trước khi kill (chưa
  gọi vì là thunk vào vùng Arxan). Nếu điều kiện đó từng chặn kill trong một
  trường hợp nào đó, mod sẽ kill sai lúc đó.
- **Không dời action 20 "Send Ghost Info"** (handler còn 1 điều kiện
  `sub_1404C8730` chưa hiểu). Bỏ khỏi danh sách việc cần làm: chơi có mod DLL
  thì phải tắt EAC, không vào được online chính thức, nên không có
  bloodstain/ghost nào để gửi cho người khác.
- Gọi hàm kill từ task `FrameBegin`, không phải từ trong lượt cập nhật nhân vật
  như game; chạy ổn trong các lần test.
- **Chưa test khi đang cưỡi Torrent.**

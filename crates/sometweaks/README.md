# SomeTweaks

Đổi tên từ **LifeBetween** (2026-08-18) khi gộp vào workspace
[`EldenRingMod`](../../README.md) - lý do đổi tên: tránh nhầm với tên file
`AutoRegen.ini`/`SomeTweaks.ini` chung namespace crate (`autoregen`,
`sometweaks`), tên cũ giữ nguyên trong lịch sử bên dưới vì vẫn phản ánh
đúng ý tưởng gốc của mod. Sống ở `crates/sometweaks` trong workspace, không
còn là repo Git riêng.

Dự định gộp các mod Quality-of-Life hiện có cho Elden Ring (AutoRegen,
RuneMultiplier, PassiveRunes, RiseArcher, ReductionWeight, DropRateTuner,
InventoryDeclutter, SpiritSummonMultiplier, QuestPath...) thành **1 DLL
duy nhất**, thay vì mỗi mod 1 file riêng.

Tên "LifeBetween" chơi chữ nhẹ với "Lands Between" (tên vùng đất trong
Elden Ring) + "Better Life" — đúng tinh thần: mod này không thêm cơ chế
gameplay mới, chỉ tinh chỉnh trải nghiệm (QoL).

## Trạng thái

Crate `cdylib` viết bằng Rust, build ra 1 DLL duy nhất (`SomeTweaks.dll`),
mỗi tính năng cũ là 1 module bật/tắt độc lập qua `SomeTweaks.ini`. Từ
2026-08-22, `[Regen Per Tick]`/`[Regen Per Hit]` mỗi mục có cờ `Enabled`
riêng (không còn chỉ dựa vào giá trị `0` để tắt).

**Đã triển khai:**

- `Regen` (hồi HP/FP/Stamina theo thời gian + theo đòn đánh trúng) — xem
  `src/regen.rs` + `src/regen/attack_hook.rs`.
- `Rune Reward` (cộng rune theo thời gian + bonus mốc thời gian, port từ
  [`PassiveRunes`](../passiverunes)) — xem `src/rune_reward.rs`.
- `RuneMultiplier` (nhân hệ số rune nhận được từ mọi nguồn, port từ
  [`RuneMultiplier`](../runemultiplier)) và `WeightMultiplier` (nhân hệ số
  Trọng Tải, ảnh hưởng cả số hiển thị lẫn roll-type thực tế, port từ
  [`WeightMultiplier`](../weightmultiplier)) — 2 hook nhỏ, độc lập nhau,
  gộp chung vào `src/multipliers.rs` (module con `rune_multiplier`/
  `weight_multiplier`, xem mục 2026-08-24 bên dưới).

**Đã chốt kiến trúc:**

- Dùng thư viện [`fromsoftware-rs`](https://github.com/vswarte/fromsoftware-rs)
  (crate `eldenring`) để đọc/ghi `WorldChrMan`/`ChrIns`/`CSChrDataModule`
  (HP/FP/Stamina) sống trong process game — không tự AOB-scan/offset tay
  cho phần này nữa. Lý do chọn Rust thay vì `libER` (C++): `libER` không có
  struct `ChrIns`/`PlayerIns` (không có field HP/FP/Stamina), còn
  `fromsoftware-rs` có đầy đủ.
- Phần **không** được thư viện cover (hook vào call site "hit trúng đòn"
  của game, gốc từ 1 CE table cộng đồng) vẫn tự viết tay bằng AOB scan +
  trampoline ASM (`global_asm!`) — không có lib nào phản chiếu (reflect)
  được call site này.
- Cấu hình đóng gói kiểu `RuneMultiplier`: `SomeTweaks.ini` được nhúng vào
  binary lúc build (`include_str!`), tự ghi lại ra đĩa nếu bị xoá. Phần
  đọc/ghi ini/log giờ dùng chung crate [`common`](../../shared) thay vì code
  riêng.

**Chưa làm:** RiseArcher (đọc/ghi `regulation.bin` sống qua `param_table`
của `fromsoftware-rs`/`libER`), các module còn lại (DropRateMultiplier/
TorrentAnywhere/Spirit/Misc trong ini hiện chỉ là placeholder).

## Đã chốt (nhưng chưa triển khai)

- **RiseArcher** sẽ chuyển từ sửa tĩnh `regulation.bin` (`.MASSEDIT`) sang
  đọc/ghi param sống bằng `libER`, hệ số chỉnh qua ini thay vì hardcode —
  lý do: cho người dùng tự chỉnh số theo ý thích, và tránh xung đột file
  `regulation.bin` khi cài chung với mod khác. Xem chi tiết ở
  `d:\MyProjects\EldenRing\RiseArcher\README.md` (mục "Quyết định: sẽ
  chuyển sang DLL + libER") - RiseArcher chưa được đưa vào workspace này.

## Đổi regen/mod.rs thành regen.rs (2026-08-24)

`src/regen/mod.rs` → `src/regen.rs`, giữ nguyên `src/regen/attack_hook.rs`
tại chỗ - dùng convention module edition 2018+ của Rust (file `<tên>.rs`
đặt ngang hàng thư mục `<tên>/` chứa submodule, thay cho `<tên>/mod.rs`
kiểu edition 2015). Lý do: tên file `mod.rs` không tự mô tả được nội dung
- mở nhiều file `mod.rs` từ nhiều module khác nhau (kể cả khác crate) cùng
lúc trong editor khiến tab khó phân biệt hơn hẳn `regen.rs`. Không gộp
`attack_hook.rs` vào cùng file hay đổi tên nó, và không bỏ thư mục con
`regen/` (giữ thư mục để tránh trùng tên `attack_hook.rs` với
`crates/autoregen/src/attack_hook.rs` nếu làm phẳng hoàn toàn) - chỉ đổi
đúng 1 việc: tên file `mod.rs`.

## Gộp RuneMultiplier + WeightMultiplier vào 1 file, thống nhất tên module (2026-08-24)

`src/rune_multiplier.rs` và `src/weight.rs` ban đầu là 2 file riêng - gộp
lại thành `src/multipliers.rs` (2 module con `pub mod rune_multiplier`/
`pub mod weight_multiplier`) vì cả 2 đều là hook 1-lần rất nhỏ, không chia
sẻ code với nhau, và việc tách file riêng chỉ thêm ceremony không cần
thiết cho quy mô này. Đồng thời đổi tên cho nhất quán theo 1 quy ước duy
nhất `<tên>_multiplier` thay vì lẫn lộn `rune_multiplier`/`weight` (module
`weight` ban đầu, trước khi gộp, chưa có hậu tố `_multiplier`) - và đổi
luôn `src/rune.rs` (Rune Reward, module `rune`) thành `src/rune_reward.rs`
(module `rune_reward`) để không còn dễ nhầm với `rune_multiplier`. `lib.rs`
giờ gọi qua `rune_reward::run`/`multipliers::rune_multiplier::run`/
`multipliers::weight_multiplier::run`.

## Thêm module WeightMultiplier, port từ WeightMultiplier crate (2026-08-24)

Kích hoạt `WeightMultiplier` trong `[General]` (trước đó chỉ là dòng
comment placeholder), triển khai `src/multipliers.rs`'s `weight_multiplier` module
port 1:1 kỹ thuật hook của
[`WeightMultiplier`](../weightmultiplier)'s `src/hook.rs`: patch 7 byte lệnh
`movaps xmm0,xmm6` (đúng thời điểm game vừa cộng xong tổng Trọng Tải, chạy
1 lần duy nhất, không dồn qua vòng lặp 5 slot đồ) để nhân hệ số vào `xmm6`
trước khi copy ra kết quả - xem README của crate `WeightMultiplier` cho
toàn bộ lịch sử tìm offset (3 lần thử, xác nhận chéo qua 1 DLL "NoWeight"
cộng đồng).

Khác biệt so với bản crate độc lập:

- Patch không đủ chỗ cho kỹ thuật absolute-jump 12-byte của
  `attack_hook.rs`/module `rune_multiplier` (chỉ 7 byte) - dùng
  `common::codepatch::install_jmp_hook` (JMP tương đối 5 byte + tìm vùng
  nhớ gần bằng `VirtualAlloc`) thay vì viết tay riêng.
- Giữ nguyên tên key `WeightMultiplier` (hệ số nhân trực tiếp, `1` = không
  đổi) - `SomeTweaks.ini` đã có sẵn key này từ trước, không cần đổi thành
  `WeightReductionPercent` (0-100%) như bản crate độc lập.
- Không có `InitialDelaySeconds` riêng trong ini (giữ nguyên delay 5s cố
  định trong code) - `SomeTweaks` chưa có module nào khác cần delay khởi
  động nên chưa đáng thêm 1 key ini chỉ cho module này. Delay này quan
  trọng: hook chỉ patch 1 lần, không có vòng lặp retry như
  `wait_for_cs_task` (xem mục fix `InvalidRva` bên dưới) - nếu quét AOB
  trước khi game giải nén/relocate code xong thì tắt hẳn cho session đó.
- Cố tình không có hotkey reload, giống nguyên bản - đổi `WeightMultiplier`
  cần khởi động lại game.

## Thêm module RuneMultiplier, port từ RuneMultiplier crate (2026-08-24)

Kích hoạt `RuneMultiplier` trong `[General]` (trước đó chỉ là dòng comment
placeholder), triển khai `src/multipliers.rs`'s `rune_multiplier` module
port 1:1 kỹ thuật hook của [`RuneMultiplier`](../runemultiplier)'s
`src/hook.rs`: patch 12 byte đầu
`AddSoul_Call` (hàm cộng-rune cấp thấp nhất của game, dùng chung mọi
nguồn) bằng redirect `mov rax,<stub>; jmp rax` sang 1 stub tự sinh nhân
`EDX` (amount) theo fixed-point Q20 integer, chỉ nhân khi `amount > 0` để
không ảnh hưởng rune **chi tiêu** (lên cấp, mua đồ) - xem README của
`RuneMultiplier` crate cho toàn bộ lịch sử dịch ngược/quyết định kỹ thuật.

Khác biệt so với bản crate độc lập:

- Giữ nguyên tên key `RuneMultiplier` (dưới `[General]`, không tách section
  `[Settings]` riêng) thay vì đổi thành `Multiplier` như bản
  `RuneMultiplier` đã làm ngày 2026-08-19 - `SomeTweaks.ini` đã có sẵn key
  này từ trước, không cần đổi tên lần nữa.
- Không tự poll `General.ReloadKey` - đọc lại `RuneMultiplier` mỗi tick
  (`apply_multiplier()`, chỉ log khi giá trị thực sự đổi) thay vì so khớp
  hotkey riêng, vì `regen::run` đã lo việc reload `SomeTweaks.ini` dùng
  chung config map cho cả module này (cùng cách `rune::run` đã làm).
- Hook install không phụ thuộc `WorldChrMan`/in-world (không dereference
  con trỏ player nào), nhưng vòng lặp áp dụng multiplier mỗi tick vẫn cần
  `CSTaskImp` nên áp dụng luôn fix `wait_for_cs_task` (retry `InvalidRva`)
  ở mục ngay bên dưới.

## Fix cả 2 module bị vô hiệu hoá vĩnh viễn bởi InvalidRva (2026-08-24)

Cùng lỗi mà [`PassiveRunes`](../passiverunes) gặp (xem README của nó, mục
2026-08-24): `CSTaskImp::wait_for_instance` coi `SystemInitError::InvalidRva`
là lỗi chết ngay, không tự retry dù truyền `Duration::MAX` (chỉ tự retry
case `Null` bên trong nó) - `InvalidRva` xảy ra khi RVA lookup chạy trước
lúc game giải nén/relocate xong (Arxan), tức là 1 cuộc đua timing với lúc
thread của DLL này khởi động, không liên quan gì đến việc DLL đặt ở đâu.
Log thực tế: cả `regen::run` lẫn `rune_reward::run` đều dính lỗi này cùng
lúc và tắt hẳn tính năng cho session đó. Đã thêm `wait_for_cs_task()`
(retry sau 1s thay vì bỏ cuộc ngay) vào cả `src/regen.rs` (khi đó còn tên
`src/regen/mod.rs`) và `src/rune_reward.rs` (khi đó còn tên `src/rune.rs`,
xem mục đổi tên module bên trên).

## Fix Rune Reward cộng rune trước khi vào world (2026-08-24)

Cùng lỗi mà [`PassiveRunes`](../passiverunes) gặp (xem README của nó, mục
2026-08-24): `GameDataMan` resolve xong ngay khi save slot được chọn, còn
đang ở màn hình title/loading, **trước khi** player thật sự vào world -
`add_runes` trong `src/rune_reward.rs` (khi đó còn tên `src/rune.rs`) trước
đây chỉ check `GameDataMan`, nên cả
tick theo interval lẫn milestone đều bắn sớm hơn dự kiến (milestone dùng
`session_elapsed_ms` tính từ lúc DLL load, không phải lúc vào game). Đã
thêm check `regen::main_player_chr_ins_ptr()` (cùng cổng `WorldChrMan.
main_player` mà `Regen` module đã dùng) vào `add_runes` - milestone loop
cũng đổi từ "bỏ qua vĩnh viễn nếu add_runes fail" sang "giữ nguyên
`next_milestone`, thử lại tick sau" để không mất bonus khi milestone rơi
đúng lúc còn đang loading.

## Gom nhóm lại ini, thêm Enabled/Condition/Trigger cho Regen (2026-08-22)

Cấu trúc key cũ (`Regen.Hp`, `Regen.HpOnHit`, `Regen.HpPctOnHit`...) chỉ có
1 quy ước "0 = tắt", không tách được "tắt hẳn tính năng" khỏi "để giá trị
0 vì đang thử". Đã đổi ini sang namespace rõ ràng hơn theo từng nhóm tính
năng (đúng ý ban đầu khi thêm `[Section]` - xem lịch sử trao đổi lúc cân
nhắc đổi sang TOML rồi quyết định giữ ini vì section + tiền tố đã đủ gom
nhóm):

- `[Regen Per Tick]`: thêm `Regen.PerTick.Enabled` (cờ bật/tắt rõ ràng) và
  `Regen.PerTick.Trigger` (0/1/2 = luôn/ngoài combat/trong combat, đặt tên
  giống `Regen.PerHit.Trigger` cho nhất quán, dù ý nghĩa là "điều kiện combat"
  chứ không phải "loại tính hồi phục" như bên PerHit). `Regen.PerTick.Unit`
  (0=điểm, 1=%) thay cho việc tách riêng `HpPct`/`Hp` như AutoRegen - mỗi
  stat giờ chỉ có 1 giá trị `Regen.PerTick.HP/FP/Stamina`, ý nghĩa đổi theo
  `Unit` chứ không cộng dồn flat+percent như bản AutoRegen.
- `[Regen Per Hit]`: `Regen.PerHit.Enabled` + `Regen.PerHit.Trigger` (0=điểm
  cố định, 1=% chỉ số tối đa, 2=% sát thương vừa gây ra - gộp 3 chế độ độc
  lập thay vì AutoRegen's `Trigger` 2 chế độ có flat+pct cộng dồn ở chế độ
  0). Cùng lý do: mỗi stat 1 giá trị duy nhất `Regen.PerHit.HP/FP/Stamina`.
- Thêm combat-tracking (`mark_combat_activity`/`is_in_combat`, cửa sổ 15s
  kể từ lần đánh/bị đánh gần nhất) vào `src/regen.rs` (khi đó còn tên
  `src/regen/mod.rs`), port từ chính
  cơ chế `Condition` mà AutoRegen từng làm dựa trên bản gốc của SomeTweaks -
  giờ port ngược lại đây vì `Regen.PerTick.Trigger=1/2` cần nó.
  `attack_hook.rs` đọc thêm `[ctx+8]` (target) để phát hiện cả trường hợp
  player **bị** đánh trúng, không chỉ **đánh** trúng - cả 2 đều tính là hoạt
  động combat dù chỉ chiều "đánh trúng" mới có heal-on-hit.
- Debug section thêm `RegenLog` (log riêng cho module Regen, độc lập với
  `DebugLog`) - theo đó `lib.rs` đổi logger sang luôn init (ghi đè mỗi lần
  chạy, không cộng dồn), thay vì trước đây chỉ init khi `DebugLog=true`
  (khiến `RegenLog=true` mặc định trong ini vô nghĩa vì chưa có file log
  nào được tạo).

Phát hiện thêm 1 lỗi copy-paste trong lúc đổi ini: `[Regen Per Hit]` bị gõ
nhầm key `Regen.PerTick.HP/FP/Stamina` (trùng với section trên), đã sửa lại
đúng `Regen.PerHit.HP/FP/Stamina`.

## Thêm module Rune Reward, port từ PassiveRunes (2026-08-23)

Kích hoạt `[Rune Reward]` trong `SomeTweaks.ini` (trước đó chỉ là block
comment placeholder), triển khai `src/rune_reward.rs` (khi đó còn tên
`src/rune.rs`, xem mục đổi tên module ngày 2026-08-24) dựa trên
[`PassiveRunes`](../passiverunes)'s `src/rune.rs` - cùng cơ chế đọc/ghi
`GameDataMan::main_player_game_data.rune_count` qua `fromsoftware-rs` và
threshold-crossing cho milestone (không dùng so khớp tuyệt đối
`elapsed==milestone`, tránh bỏ lỡ mốc nếu tick lệch nhịp), nhưng đổi tên
key và đơn vị cho khớp quy ước của `SomeTweaks.ini`:

- `Rune.Passive.Enabled` (cờ bật/tắt cả nhóm, kể cả milestone) thay vì
  `EnableMilestones` riêng của PassiveRunes.
- `Rune.Passive.Interval` tính bằng **mili giây** (khớp
  `Regen.PerTick.Interval`) thay vì `IntervalSeconds` của PassiveRunes.
- `Rune.Passive.Amount` thay `RunesPerInterval`.
- `Rune.Milestone` (không có tiền tố `Passive.` vì áp dụng độc lập với
  interval) vẫn giữ định dạng chuỗi giây:rune như PassiveRunes's
  `Milestones` - `Rune.Milestone=0` tự nhiên parse ra danh sách rỗng (không
  có dấu `:`) nên không cần case riêng cho "0 = tắt".

Chạy trên 1 thread/task riêng (`std::thread::spawn(rune::run)` trong
`lib.rs`), độc lập với tick của `Regen` - không tự lắng nghe
`General.ReloadKey` vì đọc chung 1 config map với Regen, nên hotkey reload
ở đâu cũng làm mới giá trị cho cả 2 module. Milestone list chỉ parse 1 lần
lúc khởi động (không hot-reload), giống hệt giới hạn PassiveRunes gốc đã
chấp nhận.

Tiện thể sửa 1 lỗi tài liệu trong ini: comment mô tả `Rune.Milestone` ghi
nhầm định dạng `(ms:rune)` trong khi giá trị mặc định dùng đơn vị giây
(`1800:5000` = 30 phút) - đã sửa thành `(giây:rune)`.

## Ý tưởng đã thử, chưa chốt: menu cấu hình trong game

Đã thử nghiệm 1 lần (rồi revert lại) — hook DX12 `Present` bằng
[`hudhook`](https://github.com/veeenu/hudhook) + vẽ UI bằng `imgui`, cho
slider chỉnh trực tiếp các giá trị `Regen.*` (không ghi ra ini, chỉ ảnh
hưởng bộ nhớ live, giống hệt cơ chế `ReloadKey` đang có). Về mặt kỹ thuật
là khả thi — build sạch, hook lên được, slider tương tác được.

**Vấn đề gặp phải khi test thật trong game (chưa giải quyết xong):**

- Con trỏ chuột không hiện — Elden Ring tự ẩn cursor hệ điều hành mỗi
  frame, phải bắt `imgui` tự vẽ cursor riêng (`io.mouse_draw_cursor = true`).
- Toạ độ chuột bị lệch khi Windows Display Scale khác 100% — `hudhook` lấy
  toạ độ thô từ `WM_MOUSEMOVE` (đơn vị bị Windows virtualize theo % scale)
  nhưng lại map vào kích thước buffer thật của swapchain (pixel vật lý).
  Test xác nhận: về 100% thì chuẩn, khác 100% thì lệch.
- Thử bù bằng `GetDpiForWindow` (nhân toạ độ theo `dpi/96`) nhưng **chưa ăn
  đúng** — có thể do giả định "process không DPI-aware" bị sai (Elden Ring
  nhiều khả năng đã tự per-monitor-DPI-aware, nên WM_MOUSEMOVE có khi không
  bị virtualize như giả định, cần đo lại thực tế thay vì suy luận).
- UI (khung/chữ/nút) tự nó cũng không scale theo độ phân giải/DPI — imgui
  vẽ theo pixel cố định, cần tự nhân `font_global_scale`/style scale theo
  màn hình nếu làm tiếp.

**Quyết định hiện tại:** revert toàn bộ phần này, giữ nguyên cấu hình qua
ini + `ReloadKey` hot-reload. Đây mới là thử nghiệm — sẽ quay lại giải
quyết các vấn đề trên nếu thật sự chốt chuyển sang cấu hình bằng menu
trong game (không chỉ để vui).

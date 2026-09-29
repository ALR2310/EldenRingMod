# SpiritMultiplier

Mod DLL nhân số lượng linh hồn (spirit ash) triệu hồi lên **vượt mức trần
10 con** mà engine đang áp - khác `sometweaks`'s `Spirit.Summon.Amount`/
`Multiplier` (`crates/sometweaks/src/spirit/summon_count.rs`), vốn bị kẹp
cứng ở `MAX_SUMMONS = 10`.

**Trạng thái hiện tại (2026-09-27, `0.2.0`, chưa đăng Nexus):**

- `src/band.rs`: band 60 slot (`MaxSpirits`) cho người chơi index 0; người
  chơi co-op index 1-5 có band riêng chia theo capacity (Seamless 1000 →
  60 slot mỗi người, co-op vanilla 80 → đúng band 10 của vanilla);
  `MaxSpirits > 60` thì nới luôn capacity ChrSet.
- `src/chain.rs`: kéo dài chain SpEffect → BuddyParam (`Multiplier`,
  `Amount`, hot reload).
- `src/activate_limit.rs`: luôn nâng giới hạn 60 nhân vật activate của
  engine thêm đúng số spirit đang có, để kẻ địch không bị spirit chiếm
  suất (`ActiveCharacterLimit` chỉ để debug).
- Ini mặc định: `Multiplier=2`, `Amount=0`, `MaxSpirits=60` (cũng là tối
  thiểu), `ReloadKey=F5`; debug: `SlotProbe`, `EnemyProbe` (gây crash khi
  map stream - để `false`), `ActiveCharacterLimit=0`.
- **Đã test trong game:** chơi 1 mình (60/100 spirit, dismiss sạch, kẻ địch
  không biến mất nữa) và Seamless Co-op 2 người (band riêng liền nhau, 2
  máy khớp slot, giới hạn activate tính cả spirit của người kia). Còn nghi
  vấn: đôi khi chỉ đồng bộ 1 phần spirit sau khi 1 bên load lại khu vực.

## Tạo crate, thêm SlotProbe chẩn đoán (2026-09-25)

**Bối cảnh:** kéo dài chain SpEffect -> BuddyParam
(`WorldChrMan.summon_buddy_manager.trigger_speffect_to_buddy_map`) như
`summon_count.rs` đang làm thì dễ, nhưng engine chỉ spawn tối đa 10.
Theo `.docs/x10_summon/er10x.ini`: mỗi người chơi được cấp 1 "band" cố
định 10 slot trong `WorldChrMan.summon_buddy_chr_set` (`ChrSet<ChrIns>`,
chứa cả linh hồn lẫn Torrent), điểm bắt đầu và độ dài band là hằng số
trong code engine.

**Các hướng vượt trần đã cân nhắc:**

- **A. Nới band trong capacity sẵn có** - patch hằng số độ dài/điểm bắt
  đầu band, dùng luôn slot dự phòng (nếu `capacity` thật lớn hơn 10+1,
  vd. để dành cho người chơi co-op). Rủi ro vừa, chỉ ổn khi chơi offline.
- **B. Cấp phát lại mảng `entries` của ChrSet** lớn hơn + patch band -
  không giới hạn nhưng rủi ro cao (engine có thể cache con trỏ/index ở
  chỗ khác).
- **C. Spawn NPC ngoài hệ thống buddy** - bỏ, mất hết AI "theo người
  chơi"/despawn theo Ash.

Hướng nào khả thi phụ thuộc vào `capacity` thật của ChrSet, nên bước đầu
tiên là đo.

**Đã làm:** `src/probe.rs` (ini `SlotProbe`, mặc định `true`), chạy trên
tick `FrameBegin` mỗi 500ms, gate bằng
`common::player::main_player_chr_ins_ptr()` như `summon_count.rs`:

- Log 1 lần: `index`, `capacity` (field) và `get_capacity()` (vtable) của
  `summon_buddy_chr_set`; histogram độ dài chain (bao nhiêu Ash vanilla
  triệu hồi 1/2/3... con).
- Log mỗi khi bố cục slot thay đổi: slot index nào đang có `ChrIns`
  (kèm `npc_param_id`, `chr_load_status`), cùng `active_summon_speffect_id`,
  `last_buddy_slot`, `player_has_alive_summon` của `SummonBuddyManager`.

**Cách test:** bật `SpiritMultiplier.dll` (không bật kèm `SomeTweaks`
`Spirit.Summon.*`/`er10x.dll` để kết quả sạch), vào game, gọi Torrent, gọi
1 Ash nhiều con (vd. Lone Wolf Ashes), dismiss, gọi Ash khác, rồi gửi lại
`SpiritMultiplier.log`.

**Bước tiếp:** tuỳ con số `capacity` - dò trong IDA/Ghidra chỗ engine
tính "band start/length" (bắt đầu từ `band_occupancy`/`kSigGate` của
`er10x.dll`, xem `crates/sometweaks/README.md` mục 2026-08-25/26), rồi đưa
logic chain rebuild của `summon_count.rs` lên `shared/` (`common`) để cả
`sometweaks` lẫn mod này dùng chung thay vì copy.

**Chưa test trong game.**

## Kết quả SlotProbe: capacity 80, band người chơi bắt đầu ở slot 20 (2026-09-25)

Người dùng test trong game (thư mục `mod\` của bản Steam, không có
`er10x.dll`/`SomeTweaks`; lưu ý toml có bật SeamlessCoop):

- `summon_buddy_chr_set`: `index=113`, **`capacity=80`** (field và
  `get_capacity()` khớp nhau) - lớn hơn nhiều so với 10+1, nên **hướng A
  khả thi**, không cần hướng B.
- Histogram chain vanilla: `{1: 58, 2: 18, 3: 8, 4: 4, 5: 6}` (94 Ash,
  nhiều nhất 5 con).
- **Slot #0 = Torrent** (`npc=80020000`), được tạo sẵn ngay khi vào world
  dù chưa gọi ngựa (lúc đầu tưởng nhầm là do gọi ngựa - người dùng xác
  nhận chưa thao tác gì). Gọi ngựa chỉ đổi trạng thái slot #0 (`Active` ->
  `ReadyForActivation`), không đi qua `SummonBuddyManager`
  (`active_summon_speffect_id` vẫn `-1`).
- Lone Wolf Ashes (SpEffect `232000`, 3 con) -> slot **#20, #21, #22**,
  `last_buddy_slot` 0 -> 3. Ash 5 con tiếp theo (SpEffect `240000`) ->
  slot **#23..#27**, `last_buddy_slot` 3 -> 8 - tức là tiếp nối chứ không
  quay lại #20.

- Ash 2 con tiếp theo (SpEffect `212000`) -> slot **#28, #29**,
  `last_buddy_slot` 8 -> **0** (quay vòng: `(8 + 2) % 10`). Test này chạy
  **offline, không bật SeamlessCoop** (người dùng xác nhận) - trạng thái
  `NetworkInitializing` trong log chỉ là 1 bước khởi tạo bình thường,
  không phải do online.

**Đã xác nhận bằng hành vi (chưa thấy trong code):** band 10 slot, cấp
phát vòng tròn, `cursor = (cursor + n) % 10`.

**Suy luận (chưa xác nhận trong code):** band người chơi local là slot
#20-#29, cấp phát kiểu **ring buffer** với con trỏ `last_buddy_slot`
(`slot = 20 + cursor % 10`). `80 - 20 = 60 = 6 × 10` gợi ý mỗi người chơi
trong phiên (host/co-op/invader) có 1 band riêng `20 + player_index × 10`;
chơi 1 mình thì #30-#79 bỏ trống -> hướng A có thể lên ~60 spirit bằng
cách nới band của người chơi local ra 60 slot.

**Bước tiếp:** tìm trong `eldenring.exe` chỗ tính slot (hằng `0x14`/`0xA`,
modulo theo `SummonBuddyManager::last_buddy_slot`), bắt
đầu từ `band_occupancy`/`kSigGate` của `er10x.dll`.

## Tìm ra trần 10 trong eldenring.exe, nới band lên 60 (2026-09-25)

Thử Ghidra trước (project `D:/tmp/ghidra_proj_1171`, quét hàm có
`[reg+0xBC]` + hằng `0x14`/`0xA`) - quá nhiễu (270 hàm), chuyển sang
IDA 9.3 (database có sẵn `D:/tmp/ida_re/ida_eldenring_current.i64` =
`eldenring_2710.exe`, đúng bản 2.7.1.0 đang test). Các script IDA/Ghidra
dùng lần này: `D:/tmp/ida_re/ida_spirit{1..6}.py`,
`.docs/reverse_engineering/{DecompileByName,FindBuddySlotFns}.java`.

Các bước:

1. Decompile `band_occupancy` của `er10x.dll` - xác nhận nó đếm cứng
   index `0x14..0x1E` của `summon_buddy_chr_set` (`WorldChrMan+0x10FA8`
   = `entries`, `+0x140` = 20 × 16 byte).
2. Offset thật (tính bằng `offset_of!` trên `fromsoftware-rs`):
   `WorldChrMan.summon_buddy_chr_set = +0x10F90`,
   `summon_buddy_manager = +0x1E538`,
   `SummonBuddyManager.last_buddy_slot = +0xBC`.
3. Quét hàm dùng displacement `0x1E538`/`0x10F90..0x10FA8` → 14 hàm, trong
   đó `sub_14050BC40` gọi `sub_1404B6170(alloc(0x110), chr_set)` =
   constructor `SummonBuddyManager`.
4. Decompile vùng method quanh đó, lọc `+ 188`/`% 10`/`20` → tìm ra:
   - `sub_1404BAEA0` (spawn 1 spirit):
     `slot = sub_140493380(chr_set, local_player_index /* <6 */, info, last_buddy_slot);
     last_buddy_slot = sub_140493D60(slot);`
   - `sub_140493380`: `band_start = 10 * (player_index + 2)`,
     `cursor %= 10`, duyệt 10 slot tìm entry trống, hết thì trả null
     (spirit bị bỏ) - **đây là trần 10**.
   - `sub_140493D60`: `(slot - 20 + 1) % 10`.
5. Quét tiếp code khác có hằng band hard-code (`+ 320`, `- 20`, `30`...)
   trong cùng vùng - không thấy chỗ nào khác duyệt band (dismiss/despawn
   có vẻ đi qua `groups`, chưa kiểm chứng trong game).

**Đã làm** (`src/band.rs`, cài 1 lần lúc khởi động):

- AOB `0F B6 C2 83 C0 02 8D 04 80 44 8D 14 00 B8 67 66 66 66 41 F7 E9`
  (`0x1404933AF`, duy nhất trong 2.7.1.0): thay toàn bộ vòng tìm slot
  (0x6A byte, tới lối "band đầy" `mov rax, r15`) bằng stub: band dài
  `BAND_LEN = 60` cho player index 0 (vẫn 10 cho index khác), kiểm tra
  `slot < capacity`, thoát đúng 2 lối của vanilla (`rbx` = entry, `edi` =
  slot khi tìm thấy → `0x140493421`; không thấy → rơi về `mov rax, r15`).
  Kiểm tra byte 2 lối thoát trước khi patch. Stub đã disassemble lại bằng
  capstone (test `dump_stubs`) để xác nhận encoding/đích nhảy.
- AOB `83 E8 14 78 22 8D 48 01 B8 67 66 66 66 F7 E9 C1 FA 02`
  (`0x140493D7C`, duy nhất): ghi đè tại chỗ `(x+1) % 10` → `% 60`.

`src/chain.rs`: **copy** logic chain rebuild của
`sometweaks/src/spirit/summon_count.rs` (không đưa lên `common` - người
dùng yêu cầu 2026-09-25), cap `BAND_LEN` thay vì 10, key `Amount`/
`Multiplier` (không có tiền tố `Spirit.`), đặt cả 2 về mặc định sau khi
hot reload thì chain trả về độ dài vanilla.

**Giới hạn đã biết:** slot 20..79 của host trùng với band của người chơi
co-op (index 1-5 → slot 30..79) - chỉ dùng khi chơi 1 mình. Hiệu năng với
~60 spirit chưa đo.

**Chưa test trong game.**

## Test trong game bản 0.2.0: 30 sói, dismiss sạch, quay vòng đúng (2026-09-25)

Người dùng test offline (`Multiplier=10`, `SlotProbe=true`), log xác nhận:

- Cả 2 patch cài được ngay lúc khởi động (`Band: next-cursor ... patched`,
  `Band: slot search widened to 60 slots`), `Chain: rebuilt 94 chain(s)`.
- Lone Wolf Ashes -> **30 sói ở slot #20-#49**, `last_buddy_slot` 0 -> 30.
- Dismiss -> còn đúng Torrent ở #0: **spirit ở slot >= #30 cũng được dọn
  sạch** - xác nhận dismiss/despawn không duyệt band 20-29 hard-code (rủi
  ro lớn nhất còn lại ở mục trước).
- Gọi lại -> **30 sói ở #50-#79**, `last_buddy_slot` 30 -> 0: ring cursor
  `% 60` chạy đúng, dùng hết 60 slot, không crash.

**Chưa đo:** FPS với nhiều spirit, gọi Ash khi band đã gần đầy.

## MaxSpirits: nới luôn capacity của ChrSet, test 100 con (2026-09-25)

Người dùng hỏi 60 có phải trần cứng không. Không phải: 60 chỉ là phần còn
trống của mảng 80 slot game tự tạo. Tìm trong IDA chỗ game khởi tạo
ChrSet summon - `sub_1404954B0` (gọi từ `sub_14050BC40`, lúc dựng world):
`alloc(0x500)` (80 × 16 byte), `capacity = 0x50` (cũng là số vòng lặp
init từng entry), `memset(.., 0x500)` - 3 imm32 nằm ngay trong lệnh, sửa
tại chỗ được. Kiểm tra thêm trước khi làm:

- Handle nhân vật mã hoá slot bằng mask `0xFFFFF` (bảng `0x143B37920`,
  entry type 1) - 20 bit, không giới hạn capacity cỡ vài trăm.
- Vector `reserve(80)` trong constructor WorldChrMan chỉ là dung lượng cấp
  trước (tự nới), không phải trần.
- Hàm spawn spirit từ mạng (`sub_140493210`) dùng slot do mạng gửi tới,
  nếu slot đã có nhân vật thì **trả về nhân vật đó** thay vì tạo mới -
  lý do mod không hợp co-op (xem mục trước).

**Đã làm:**

- Key mới `MaxSpirits` (mặc định `60`, kẹp 10..1000), đọc 1 lần lúc khởi
  động (cần restart game, F5 không áp dụng). `band.rs`: `BAND_LEN` từ
  hằng số thành `AtomicU32` (`band_len()`); patch thứ 3 `CAPACITY_INIT_AOB`
  (`0x140495534`, duy nhất trong 2.7.1.0) ghi `20 + MaxSpirits` vào 3 imm32
  khi `MaxSpirits > 60`. Nếu patch capacity lỗi, band tự lùi về 60.
- `chain.rs`: cap theo `band_len()` thay vì hằng số.

**Test trong game** (`MaxSpirits=100`, `Amount=100`, offline):

- `Band: summon ChrSet capacity 80 -> 120 patched` lúc khởi động, và khi
  vào world `capacity(field)=120` - patch chạy **trước** khi world được
  tạo, đúng như mong đợi.
- Lone Wolf Ashes -> **100 sói ở slot #20-#119**, load dần trong ~2 giây
  (~25-30 con mỗi 0.5s, `NetworkInitializing` -> `ReadyForActivation`).
- Khi summon kết thúc, cả 100 slot được dọn sạch, gồm cả #80-#119 (7 con
  cuối còn `Active` thêm ~4 giây rồi mới biến mất - chưa rõ lý do, đang
  hỏi người dùng).
- Không crash. Người dùng: càng nhiều spirit FPS càng tụt - đây chỉ là
  thử nghiệm khả năng, **dừng ở 100, không tìm tiếp trần heap nhân vật**
  của engine (không phụ thuộc RAM máy). `MaxSpirits` mặc định giữ `60`
  (không cần patch capacity).

## Chốt cấu hình mặc định: Multiplier=2, Amount=0, MaxSpirits=60 là min (2026-09-25)

Người dùng chốt cấu hình mặc định lần đầu: `Multiplier=2`, `Amount=0`,
`MaxSpirits=60`. Để giảm rủi ro, **60 vừa là mặc định vừa là mức tối
thiểu** (`band::install` kẹp `60..=1000` thay vì `10..=1000`): ở mức
60 mảng slot 80 của game giữ nguyên như vanilla (patch capacity chỉ chạy
khi `MaxSpirits > 60`). Vượt 60 là người dùng tự chịu rủi ro (FPS, có thể
crash vì hết heap nhân vật) - ghi rõ trong comment ini. 2 patch band
(`BAND_ALLOC_AOB`, `NEXT_CURSOR_AOB`) vẫn luôn chạy, vì thiếu chúng
thì không thể vượt 10.

## Test Seamless Co-op, band riêng cho từng người chơi (2026-09-26)

Test 2 người (host + 1 người join qua Seamless Co-op, cả 2 cài mod,
`Amount=30`, `SlotProbe=true`, log host + log máy join trong Sandboxie):

- **Seamless tự nới capacity ChrSet summon lên 1000** trên cả 2 máy (log
  `capacity(field)=1000` mà không có dòng `capacity ... patched` của mod
  này - `MaxSpirits=60`).
- **Slot #1 = Torrent của người chơi kia** (`npc=80020000`, xuất hiện khi
  người thứ 2 vào) - slot #1..#19 dành cho Torrent của người chơi khác,
  không được đụng tới.
- Spirit của host (30 con, 2 lượt: #20-#49 rồi #50-#79) hiện **đúng từng
  slot** trên máy join - mạng gửi số slot của chủ nhân.
- Người join (index 1) chỉ ra **10** sói ở #30-#39 dù chain xin 30: do
  stub cũ cố ý giữ band vanilla 10 cho index khác 0 (tránh chồng lên band
  60 của host). Trên máy host chỉ thấy 8 con (#32-#39) - log kết thúc ngay
  đó, chưa rõ 2 con kia mất hay đang load.

Đọc lại `sub_140493210` (tạo spirit nhận qua mạng): **không kiểm tra slot
< capacity** - ghi thẳng `entries[slot]`. Vì vậy band của mọi người chơi
phải nằm gọn trong capacity của MỌI máy, và không được chồng nhau.

**Đã làm** (người dùng yêu cầu bỏ giới hạn 10 cho người join): stub mới
chia band theo capacity thật (`band_layout`, có test):

- Index 0: `[20, 20 + MaxSpirits)` - như cũ.
- Index 1..5: `part = (capacity - 20) / 6`, bắt đầu `20 + index × part`,
  dài `min(MaxSpirits, part)`.
- Capacity 80 (co-op vanilla, không Seamless) → `part = 10` → **đúng y
  band vanilla** của người join (an toàn như cũ). Capacity 1000 (Seamless)
  → band `20, 183, 346, 509, 672, 835`, mỗi band 60, không chồng nhau.
- Next-cursor `% MaxSpirits` giữ nguyên - với band không bắt đầu ở bội
  của `MaxSpirits` nó chỉ đổi vị trí bắt đầu vòng tìm, stub vẫn `% len`.

**Giới hạn còn lại:** người chơi KHÔNG cài mod vẫn dùng band vanilla
#30..#79, trùng với band rộng của host; `MaxSpirits > 163` với Seamless
thì band index 0 chồng lên band index 1. Giả định mọi máy cùng capacity
(đúng khi cùng Seamless).

**Test trong game** (Seamless, 2 người cùng cài mod, `Amount=30`):
người join **triệu hồi đủ 30 con** ở band #183.., cả 2 máy thấy đủ 60
spirit, dọn slot khớp nhau, không crash. 2 vấn đề:

1. **Spirit của người join rải rác** trong band (#183-#240 cho 30 con) -
   lỗi của patch next-cursor: `(slot - 20 + 1) % 60` chỉ đúng với band bắt
   đầu ở #20. **Đã sửa:** patch next-cursor giờ trả về `slot - 19` không
   wrap; stub tự đổi về vị trí trong band của người gọi:
   `(cursor - (start - 20)) mod len` (index 0 cho kết quả y như cũ).
   Chưa test lại trong game.
2. **Đôi khi chỉ đồng bộ 1 phần** - 2 lần, đều ngay sau khi 1 bên vừa load
   lại khu vực: máy join thấy 11/30 spirit của host (10:05), host thấy
   13/30 spirit của người join (10:08); các lần khác đủ 30/30. Nghi do
   đồng bộ mạng của engine/Seamless (hoặc 2 game chạy cùng 1 máy qua
   Sandboxie), không phải do band - chưa xác nhận.

## Kẻ địch biến mất khi có 60 spirit: giới hạn 60 nhân vật active (2026-09-26)

Người dùng báo: đứng trước Stormgate, gọi 60 spirit thì lính ở đó biến
mất. Điều tra (IDA trên `eldenring.exe` 2.7.1.0 + 1 agent tìm cộng đồng/
Paramdex/fromsoftware-rs):

- **Omission** (`ChrIns::omission_mode`, budget
  `WorldChrMan::omission_update_budget_near/far` ở `+0x1F220/+0x1F224`,
  tính trong `sub_14050FEA0`) chỉ hạ tần suất update (30/20/5/1 FPS,
  NoUpdate) - làm quái giật, không làm biến mất.
- **Ngưỡng activate theo khoảng cách**: `sub_1403FBE70` tính
  `ChrIns::chr_activate_threshold` (`+0x40C`) từ khoảng cách + bonus, so với
  ngưỡng vùng của `CSOpenChrActivateThresholdRegionMan` → cờ
  `activate_threshold_exceeded` (`+0x1CA` bit 2). Không đếm số lượng.
- **Giới hạn SỐ LƯỢNG activate - thủ phạm:** `sub_140510970` gom ứng viên
  activate/deactivate (kèm `chr_activate_threshold` làm ưu tiên) vào
  vector `WorldChrMan+0x1F1D0`, đồng thời **tăng bộ đếm
  `WorldChrMan+0x1E618` (124440) cho mọi nhân vật đã/luôn active**.
  `sub_14050F9E0` sắp xếp ứng viên rồi chỉ activate
  `limit - bộ_đếm` con đầu, phần còn lại gọi `sub_1403E93A0(chr, 2)` - bị
  cắt (`ChrIns::load_state.evaluation_value_cutoff`, debug string
  評価値足切り).
- `limit` nằm trong singleton `qword_143D6A208`, constructor
  `sub_145AE6B75` ghi cứng: `+0xE8 = 1` (bật), **`+0xEC = 60`, `+0xF0 =
  60`** (2 giới hạn), `+0xF4` = cờ chế độ thay thế, `+0xF8/+0xFC = 40` (giới
  hạn ở chế độ thay thế).

**Kết luận (chưa xác nhận bằng log trong game):** 60 spirit được tính vào
bộ đếm "đã active" → `60 - 60 = 0` suất cho kẻ địch open-field → chúng bị
cắt. Agent không tìm thấy báo cáo công khai nào về đúng triệu chứng này,
cũng không có mod nào nâng giới hạn này.

`src/enemy_probe.rs` (ini `EnemyProbe`, mặc định `false`): log nhân vật
không phải summon trong bán kính 100m khi có thay đổi - load status,
omission, `activate_threshold_exceeded`, `activation_enabled`,
`evaluation_value_cutoff`, kèm số summon và bộ đếm `+0x1E618`
(`active_count`), để xác nhận giả thuyết trên.

**Hướng sửa đề xuất (chưa làm):** nâng `+0xEC/+0xF0` (và `+0xF8/+0xFC`)
thêm đúng `MaxSpirits`, để kẻ địch vẫn giữ 60 (40) suất như vanilla - đổi
lại CPU nặng hơn khi vừa nhiều spirit vừa nhiều kẻ địch.

## EnemyProbe gây crash, đính chính cách đọc log, thêm thí nghiệm ActiveCharacterLimit (2026-09-26)

- **EnemyProbe gây crash** 2 lần: lần 1 lúc vào world (đọc `ChrIns` của
  entry `Unloaded` - con trỏ cũ, đã free), lần 2 khi di chuyển (sau khi đã
  chỉ đọc entry `Active`/`ReadyForActivation`) - duyệt thủ công mọi
  `WorldChrMan.chr_sets` khi map đang stream vẫn không an toàn. Đã tắt
  (`EnemyProbe=false` trong ini game); không dùng nữa nếu chưa đổi cách
  duyệt (vd. `chr_inses_by_distance` của game).
- **Đính chính mục trước:** log (chưa gọi spirit, `summons=1` = Torrent)
  cho `active_count = 60` và cờ `evaluation_value_cutoff` bật trên 28/29
  nhân vật gần, kể cả 1 NPC cách 2m đang hiện bình thường. Nghĩa là:
  (1) `WorldChrMan+0x1E618` là số nhân vật đã activate trong frame (tính cả
  các con do chính `sub_14050F9E0` activate) - ở chỗ đó game gốc **đã chạm
  trần 60 sẵn**; (2) bit 9 của `load_state` **không** có nghĩa "bị cắt" -
  không dùng làm bằng chứng nữa.
- IDA thêm: giới hạn chỉ áp dụng khi `+0xE8` (bật) hoặc `+0xF4` (chế độ
  thay thế, dùng `+0xF8`) khác 0; `sub_14050F9E0` tính
  `[+0xEC] - [WorldChrMan+0x1E618]`.

**Thí nghiệm** `src/activate_limit.rs` (ini `ActiveCharacterLimit`, mặc
định `0` = không đụng): lấy địa chỉ singleton `qword_143D6A208` từ AOB
`48 8B 05 ?? ?? ?? ?? 80 B8 E8 00 00 00 00 75 0D 80 B8 F4 00 00 00 00 0F 84`
(`0x14050FAEC`, duy nhất trong 2.7.1.0, giải RIP-relative), mỗi giây ghi
giá trị đó vào `+0xEC/+0xF0/+0xF8/+0xFC`, log 1 lần giá trị gốc. Test với
`ActiveCharacterLimit=120` (60 vanilla + 60 spirit) ở Stormgate: nếu lính
không còn biến mất thì giả thuyết "spirit chiếm suất activate" đúng.
**Chưa test trong game.**

## Xác nhận: spirit chiếm suất activate của kẻ địch; ActiveCharacterLimit hot reload (2026-09-26)

Test ở Stormgate với `ActiveCharacterLimit=120`, 60 spirit: **kẻ địch
không còn biến mất** - xác nhận giả thuyết (spirit bị tính vào giới hạn 60
nhân vật activate của `sub_14050F9E0`). Log xác nhận đọc đúng singleton:
`enabled=1 alt_mode=0`, giá trị gốc `+EC/+F0/+F8/+FC = [60, 60, 40, 40]`.

`activate_limit.rs` giờ hot reload: module luôn chạy (kể cả khi key = 0
lúc khởi động), lưu giá trị gốc của game ở lần đầu thấy singleton (trước
khi ghi), mỗi giây đọc lại `ActiveCharacterLimit` - `> 0` thì ghi giá trị
đó, `0` thì trả về giá trị gốc. Log 1 dòng mỗi khi giá trị áp dụng đổi.
**Đã test trong game, hoạt động đúng:** `120` → lính còn; F5 về `60` →
lính biến mất lại; F5 về `0` → trả giá trị gốc, giống `60`.

## KeepEnemySlots: tự nâng giới hạn activate theo số spirit đang có (2026-09-26)

Người dùng chọn phương án "giới hạn = giá trị gốc + số spirit đang có"
thay vì cố định `60 + MaxSpirits`: không có spirit thì game giữ đúng giới
hạn vanilla (không tốn thêm CPU), có spirit thì kẻ địch vẫn giữ đúng số
suất như vanilla.

- Key mới `KeepEnemySlots` (`[Settings]`, mặc định `true`, hot reload):
  `activate_limit.rs` mỗi giây đếm slot đang có nhân vật trong
  `summon_buddy_chr_set` từ `BAND_START` (20) trở lên - bỏ qua Torrent,
  tính cả spirit của người chơi co-op đang hiện trên máy mình (cũng chiếm
  suất) - và đặt `+EC/+F0/+F8/+FC = giá trị gốc + số đó`. Chỉ đọc con trỏ
  entry, không đọc `ChrIns` (tránh lỗi con trỏ cũ như `enemy_probe.rs`);
  spirit đang load/biến mất có thể được đếm thêm 1-2 giây - chỉ làm giới
  hạn rộng hơn chút. Chỉ ghi khi giá trị trong game khác giá trị đích.
- `ActiveCharacterLimit` giờ là ghi đè thủ công (> 0 = giá trị cố định,
  bỏ qua `KeepEnemySlots`); cả 2 tắt thì trả giá trị gốc của game.
- `band::BAND_START` thành `pub` để dùng chung.

Chi phí của mod: mỗi giây ghi tối đa 4 số u32 + duyệt mảng slot (80, hoặc
1000 với Seamless) - không đáng kể. Chi phí thật là game xử lý thêm nhân
vật khi giới hạn được nâng. **Chưa test trong game.**

## Bỏ key KeepEnemySlots - luôn hoạt động (2026-09-26)

Người dùng: kẻ địch biến mất là **lỗi của mod**, không phải tính năng để
bật/tắt - xoá key `KeepEnemySlots` khỏi ini và code; `activate_limit.rs`
luôn đặt giới hạn = giá trị gốc + số spirit đang có. `ActiveCharacterLimit`
giữ lại chỉ để debug (comment ini: "DO NOT CHANGE THIS VALUE"). Các mục
phía trên nhắc `KeepEnemySlots` đã lỗi thời.

## Log in phiên bản game + danh sách DLL đã nạp (2026-09-27)

Đồng bộ với các mod khác sau khi merge `main` (commit `7bf7300`, module
`common::diag`, chi tiết trong `crates/weightmultiplier/README.md` mục
2026-09-26): `lib.rs` gọi `common::diag::log_environment_when_game_ready()`
ngay trước `chain::run()` (tính năng chính, không bao giờ trả về) - chờ
`CSTaskImp` rồi ghi 1 dòng `Game: eldenring.exe v<version> ...` và danh
sách DLL đã nạp (ẩn DLL của Windows/Steam/game kèm theo, `steam_api64`,
`OnlineFix64`; không in đường dẫn đầy đủ), để log gửi kèm báo lỗi tự trả
lời "khác phiên bản game?" và "có mod summon nào khác chạy cùng?". Không
đổi hành vi. Chưa có phần "phát hiện mod khác đã patch AOB" như
`weightmultiplier` - để sau.

## GhostColor: bật/tắt màu ma của spirit (2026-09-27)

Người dùng muốn bật/tắt lớp màu ma (tint xanh/bạc) game gốc áp lên mọi
spirit ash - không phải màu kim loại lạ thấy trước đó (cái đó do Seamless
Co-op, đã xác nhận, không liên quan mod).

- `src/ghost_color.rs` (ini `GhostColor`, `[Settings]`, mặc định `true` =
  vanilla, hot reload 1 chiều): khi `false`, mỗi frame gỡ SpEffect
  295000-295999 (dải tint theo `er10x.ini`) khỏi spirit trong
  `summon_buddy_chr_set`. **Copy** từ `sometweaks/src/spirit/color.rs`,
  không đưa lên `common` (người dùng chọn 2026-09-27).
- Khác bản SomeTweaks: chỉ đọc `ChrIns` của entry `Active`/
  `ReadyForActivation` (tránh con trỏ cũ - bài học từ `enemy_probe.rs`),
  bỏ qua slot < `BAND_START` (Torrent).
- Đổi `false` → `true` bằng F5: spirit mới gọi có lại màu ma, spirit đang
  có mặt giữ nguyên không màu tới khi gọi lại (không áp lại SpEffect).

**Chưa test trong game.**

## GhostColor: chuyển sang sửa NpcParam thay vì gỡ SpEffect mỗi frame (2026-09-27)

Người dùng hỏi sao phải gỡ màu mỗi frame khi màu vốn là 1 tham số
regulation - đúng. Xác định chuỗi tham số (Paramdex + fromsoftware-rs):
`BuddyParam.npcParamId` → `NpcParam.spEffectID26` (offset `0x210`, khớp
`ghost_slot = 528` của er10x) = 295000/295200 → `SpEffectParam.vfxId*` →
`SpEffectVfxParam.phantomParamOverwriteId` → `PhantomParam` (màu RGBA thật).
**Người dùng xác nhận trong Smithbox:** NpcParam `140700000` (Lone Wolf)
có `spEffectID26 = 295000` "[Spirit Summon] Color". (Tính offset bằng
Paramdex ban đầu lệch 1 byte - `u8 disableParam_NT:1` +
`dummy8 disableParamReserve1:7` là cùng 1 byte.)

`src/ghost_color.rs` viết lại (bỏ bản gỡ SpEffect mỗi frame):

- Lần đầu vào game (gate `main_player`): `common::params::check` cho
  `BuddyParam`/`NpcParam`; gom `npcParamId`/`npcParamId_ridden` của mọi
  hàng BuddyParam; lưu `(row index, giá trị gốc)` của các hàng NpcParam
  thuộc tập đó có `spEffectID26` trong 295000-295999.
- Mỗi giây đọc `GhostColor`, chỉ khi đổi mới ghi `-1` (tắt) hoặc giá trị
  gốc (bật) - hot reload 2 chiều, áp cho spirit gọi sau khi đổi.
- ID hàng NpcParam đọc qua **`common::params::row_ids`**
  (`shared/src/params.rs`): đọc row descriptor của file param (không qua
  bảng lookup), kiểm tra `data_offset` khớp địa chỉ hàng thật, không khớp
  thì trả `None`. (Lúc đầu tự viết 1 bản chỉ cho descriptor 64-bit; khi
  merge `main` 2026-09-29 thì `main` đã có sẵn `row_ids` cùng chữ ký, xử lý
  cả descriptor 32/64-bit theo `format_2d` - giữ bản của `main`, bỏ bản
  riêng.) Không dùng `get_mut`/bảng lookup - trên regulation mod (vd.
  Convergence) bảng đó có thể có entry rác, tra ID có thể ra nhầm hàng.

**Đã test trong game, hoạt động đúng** (người dùng xác nhận).

## Regen: spirit tự hồi máu (2026-09-29)

Key mới `Regen` (`[Settings]`, mặc định `0.5`, `0` = tắt, hot reload):
`src/regen.rs` mỗi giây hồi `Regen`% HP tối đa (ít nhất 1 điểm, không vượt
max, bỏ qua con đã chết) cho mọi nhân vật trong `summon_buddy_chr_set` -
**cả spirit lẫn Torrent** (người dùng chọn).

**Copy** từ `sometweaks/src/spirit/regen.rs` (`Spirit.Regen`), không đưa
lên `common` (người dùng chọn 2026-09-29). Khác bản SomeTweaks: chỉ đọc
`ChrIns` của entry `Active`/`ReadyForActivation` thay vì `characters()`
(mọi entry có con trỏ) - cùng bài học con trỏ cũ từ `enemy_probe.rs`.

Ghi chú: mặc định `GhostColor` trong ini mẫu người dùng đã tự đổi thành
`false`.

Commit không chạy test riêng trong game: người dùng xác nhận logic đã test
qua `Spirit.Regen` của SomeTweaks, bản này chỉ thêm bộ lọc an toàn.

## Tra cứu: cờ hành vi spirit và vì sao GhostColor chỉ cần ô 26 (2026-09-29)

Tra trong Smithbox cùng người dùng (không đổi code):

- **Cờ hành vi (SpEffect trong NpcParam của spirit):** mọi spirit có
  `297000` "Follow & Warp to Player" (ô 28). Kiểu đi theo nằm ở ô 29:
  `297100` WALK_FOLLOW (chỉ Putrid Corpse - đi bộ), `297101` REAR_FOLLOW
  (23 spirit đánh xa/phép/hỗ trợ - đứng sau người chơi), `297102`
  FLY_FOLLOW (Winged Misbegotten, Spirit Jellyfish, Warhawk, Stormhawk),
  `297103` NO_FOLLOW (Latenna - đứng yên); không có cờ = đi theo mặc định
  (Lone Wolf, Mimic Tear...). `297200` INTERCEPT_LONGRANGE (ô 24, 8 spirit
  đánh xa) là cờ cách đánh, không phải cách đi theo. Tên `PLAN_SP_EFFECT_*`
  gợi ý đây là cờ cho AI; chưa đổi thử trong game.
- **Puppet có thêm SpEffect 295201-295204 (ô 24)**, cũng trong dải 295xxx
  nhưng GhostColor không đụng tới (chỉ đọc ô 26). Tra `SpEffectVfxParam`:
  `295200` "Color (Puppet)" → VFX `57100` có `phantomParamOverwriteType=2,
  Id=201` (đổi màu nhân vật - đây mới là màu ma); `295201`-`295204` → VFX
  `57101`-`57104`/`57130`-`57133` **không** ghi đè phantom, chỉ gắn SFX
  (`60104x`/`60105x` ở dummy poly 199/905) - trang trí của Puppet, không
  phải màu ma. Nên GhostColor giữ nguyên chỉ ô 26 là đúng.

## Tra cứu cơ chế dịch chuyển spirit, thêm WarpDistance/WarpBlockedTime để thử nghiệm (2026-09-29)

Người dùng thấy spirit không dịch chuyển về khi cưỡi Torrent bỏ xa. Tra
param (Smithbox export):

- `GameSystemCommonParam`: `buddyWarp_TriggerDistToPlayer = 33`,
  `buddyWarp_TriggerTimeRayBlocked = 5`,
  `buddyWarp_ThresholdTimePathStacked = 3`,
  `buddyWarp_ThresholdRangePathStacked = 1`. Engine giữ bản sao lúc chạy
  ở `SummonBuddyManager.warp_manager` (`SummonBuddyWarpManager`, các giai
  đoạn `RequestWarp → Warping → FadeIn`). Chỉ xa thôi không đủ (quan sát
  của người dùng) - còn phải khuất tầm nhìn và/hoặc bị kẹt; cách kết hợp
  AND/OR chưa đọc trong code.
- `NpcThinkParam` Latenna và Lone Wolf giống nhau ở mọi field "quay về"
  (`maxBackhomeDist 9999`, `backhomeDist 9979`, `isBuddyAI 1`,
  `backToHomeStuckAct 0`) → đi theo/dịch chuyển không do các field này;
  Latenna đứng yên do SpEffect `297103` NO_FOLLOW (script AI khác nhau:
  `battleGoalID` 317000 vs 407000; Latenna không nhảy/rơi, có
  `rangedAttackId`). Nhận định của GPT "warp nằm ở AI/NpcThinkParam" không
  khớp dữ liệu này.

**Đã làm** `src/warp.rs` (ini `WarpDistance=33`, `WarpBlockedTime=5`, hot
reload): mỗi giây, sau khi vào game, ghi 2 giá trị vào
`warp_manager.trigger_dist_to_player` / `trigger_time_ray_block` nếu khác
(log mỗi lần đổi). Mục đích: người dùng tăng/giảm trong game để tìm cách
các điều kiện kết hợp.

**Kết quả test (người dùng):** log xác nhận giá trị được ghi (tới `0 m`/`0 s`;
game tự đặt lại 33/5 khi load khu vực - manager nạp lại từ
`GameSystemCommonParam` - và mod ghi đè lại), nhưng **không có tác dụng**:
spirit vẫn không dịch chuyển. Chỉ sửa ngưỡng là chưa đủ - nghi dịch chuyển
chỉ xét spirit đã có trong `SummonBuddyWarpManager::entries` (cần 1 cơ chế
khác yêu cầu trước), hoặc engine đọc thẳng param ở chỗ khác. **Đã gỡ
`src/warp.rs` và 2 key `WarpDistance`/`WarpBlockedTime`** khỏi bản sắp phát
hành; người dùng tạm dừng hướng này, làm lại sau khi đăng 1.0.0 (bước tiếp:
đọc hàm update của `SummonBuddyWarpManager` trong IDA).

## Chuẩn bị phát hành 1.0.0 (2026-09-29)

- `Cargo.toml`: version `0.2.0` → `1.0.0`.
- `DESCRIPTION.bbcode` mới (theo `template/DESCRIPTION.bbcode`): intro 1
  câu, Features (nhân spirit theo hệ số/số cố định, mọi Ash kể cả Ash của
  mod khác, hồi máu, bỏ màu ma, Seamless Co-op, phím reload), Notes (60 là
  ngưỡng an toàn, có thể tăng nếu máy đủ khoẻ), EAC disclaimer, Credits
  (10x Spirit Summons, fromsoftware-rs), changelog `1.0.0: Initial
  release`. Người dùng bỏ phần "Why this mod?"; việc kẻ địch không biến
  mất không ghi là tính năng (đó là lỗi do chính mod gây ra, đã sửa).
- Ini mặc định chốt: `Multiplier=2`, `Amount=0`, `MaxSpirits=60`,
  `GhostColor=false`, `Regen=0.75`, `LogFile=true`; các key debug
  (`SlotProbe`, `EnemyProbe`, `ActiveCharacterLimit`) giữ, mặc định tắt.
- Tính năng dịch chuyển spirit để sau 1.0.0 (xem mục trước).
- Đã deploy lên Nexus: **mod 11168** (Spirit Multiplier, category
  Gameplay), file `SpiritMultiplier` 1.0.0 (mod file đầu tiên, tạo bằng
  `POST /mod-files` vì trang mới chưa có update group nào), build từ commit
  `f3454eb`. Trang mod lúc upload còn ở trạng thái chưa công khai.

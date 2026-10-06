# TODO

Việc cần làm, chia theo mod. Lý do / chi tiết kỹ thuật ghi trong
`crates/<mod>/README.md` (nhật ký phát triển), không ghi ở đây.

## SpiritMultiplier

- [x] GhostColor=false không có tác dụng với ELDEN RING Reforged (Nexus, Quantum240, 2026-09-30) - sửa ở 1.1.2 (tắt vfx màu, không gỡ SpEffect)
- [ ] Spirit dịch chuyển tới người chơi khi bị bỏ lại (hoãn từ trước 1.0.0; chỉ chỉnh `buddyWarp_*` không có tác dụng)
- [x] Reforged: Spirit Fury / Fortune of the Spiritcaller hỏng - "cannot resummon/aggro the spirits", Spiritcaller ring không hoạt động (Nexus, puffintoast, 2026-10-01; chưa rõ bản mod họ dùng)
  - Cơ chế Reforged (CHANGELOG.txt của ERR): bấm lại Ash khi spirit đang ở ngoài = **kích nộ (Spirit Fury)**, không phải cho về; cho về bằng item Spirit-Severing Blade. "Aggro" = Spirit Fury, "un-aggro'ed" = trạng thái bình tĩnh mặc định
  - Nghi MultiSpirit: chiếm đúng thao tác bấm lại Ash (patch `DoSummon`, `DisappearAll`, `GetBuddyState`/`CanUseItem`/UI) - có thể đè lên hook của `reforged.dll` ở cùng hàm. Đã nhờ họ thử `MultiSpirit=false` (F5), chờ phản hồi
  - Hướng sửa lâu dài: nhận diện Reforged (vd. `reforged.dll` trong module đã nạp) rồi nhường thao tác bấm lại Ash cho Spirit Fury; cần tự tái hiện (cần fortune Spiritcaller, rơi từ boss Road's End Catacombs)
- [ ] Reforged: spirit gọi ra mặc định ở trạng thái đỏ (Spirit Fury) thay vì trắng "passive" (puffintoast giải thích 2026-10-01, sau khi xác nhận 1.1.4 chạy tốt)
  - Muốn: gọi ra ở trạng thái trắng như spirit thường; hoặc tuỳ chọn bảng màu trong ini (vd. bản đen của trạng thái đỏ)
  - Chưa rõ màu đỏ là do mod (vd. `GhostColor=false` tắt vfx 70000/70010/70020 của Reforged, chỉ còn màu Fury) hay thiết kế Reforged - cần test `GhostColor=true` vs `false` trên Reforged
- [ ] Reforged + MultiSpirit: kích Spirit Fury riêng cho từng Ash (hiện Fury 1 nhóm thì mọi Ash đều xám) - cần tìm cách Reforged đánh dấu "đang Fury" và cái gì làm Ash xám (SpEffect trên người chơi/spirit? `reforged.dll`?); lưu ý cân bằng
- [ ] Reforged: NoRestResummon không có tác dụng sau khi cho về bằng Spirit-Severing Blade - Ash đã gọi trước đó không gọi lại được tới khi nghỉ (người dùng test 2026-10-01)
  - Là luật của Reforged gốc (người dùng xác nhận: sau Blade ô Ash xám tới khi ngồi grace), không phải do mod; NoRest chỉ gỡ khoá vanilla (`summonedEventFlagId` của bia đá)
  - Không phải SpEffect triệu hồi chuẩn: `9540/9541/9545/9547/9560` (GameSystemCommonParam `onBuddySummon_*`) trong Reforged gần như rỗng, `9560` "Spirit Summon Disappeared" không có tác dụng → nghi event flag của script Reforged (EMEVD/ESD) hoặc `reforged.dll`
- [ ] Seamless Co-op: spirit mất màu ma, trông như kẻ địch thường; muốn có lại (Nexus, DVM20, 2026-09-30) - tạm dừng 2026-10-02
  - Đã loại trừ (probe `GhostProbe`, code ở `git stash` "seamless ghost probe"): PhantomParam 200/201 giống hệt offline; spirit vẫn có SpEffect 295000; module vfx (`ChrIns.modules +0xB8`, field +0x44) vẫn chọn 200; team (47 offline / 1 Seamless) không phải nguyên nhân
  - Nguyên nhân: Seamless hook đầu `sub_1403F0C20` (2.7.1.0) = hàm "PhantomParam của ChrIns" (override +0x540 → vfx +0x44 → mặc định theo chr type). Tìm bằng diff code game trong RAM vs `eldenring.exe` (offline vs Seamless: 363 chỗ chỉ có ở Seamless). Seamless còn hook `sub_1403F1C90` (get team)
  - Đặt `phantom_param_override = 200` trên spirit: không có tác dụng → hook bỏ qua cả override
  - Hướng sửa còn lại: đổi rel32 của lệnh `E9` Seamless đặt ở đầu `sub_1403F0C20` sang stub của mình - spirit (vfx +0x44 >= 0, trong summon_buddy_chr_set) trả id đó, còn lại nhảy tiếp vào hook Seamless; chỉ cài sau khi Seamless đã hook xong (byte đầu = E9)
- [x] Bug: bấm lại Ash đã nâng cấp (+1..+10) gọi thêm spirit thay vì cho về - MultiSpirit (Nexus bug report "unsummon", MonkeyDLuffy2426, 2026-10-01, Mimic Tear / Lhutel +4, bản 1.1.2) - sửa ở 1.1.3 (làm tròn `100 * (id / 100)` như `DoSummon`)

## AutoRegen

- [ ] Bug: `Regen.PerHit.ExcludeAow=true` vẫn hồi FP khi dùng Unsheathe (Nexus, Brokensword7, 2026-10-04)
  - Nghi: Unsheathe = bấm `L2` vào tư thế, rồi bấm `R1`/`R2` để chém. Latch `LAST_ATTACK_WAS_SKILL` (`regen.rs::update_last_attack_input`) thấy `R1`/`R2` → chốt "đòn thường" → nhát chém bị tính là đòn thường. Các Ash 2 bước tương tự (Quickstep? Storm Stomp thì không; kiểm tra: Unsheathe, Sacred Blade, Lion's Claw, Bloodhound's Step...) có thể bị cùng lỗi
  - Cần: xác nhận bằng log (`atkId` + `l2`/`r1` lúc trúng); hướng sửa: nếu đang trong anim/action của Ash (giữ latch tới khi anim chính kết thúc) thay vì đổi ngay khi `R1`/`R2` bấm. Chưa có ini đầy đủ của người dùng (ảnh bị cắt) - đã hỏi lại trong comment

## SpeedMultiplier

- [x] Tách tốc độ đi bộ / chạy cho player (Nexus, cfzlbj, 2026-10-01) - xong 2026-10-02: `PlayerWalk` / `PlayerRun` / `PlayerSneak`, bỏ `PlayerMovement` (bàn phím chỉ có đi bộ + chạy giữ Shift, không có sprint riêng)
- [x] Tách tốc độ Torrent theo kiểu di chuyển (cùng yêu cầu của cfzlbj) - xong 2026-10-02: `[Torrent]` `Walk` (`0021xx`) / `Run` (`0022xx`) / `Jump` (`0061xx`, `0074xx`) / `Other`
- [x] ~~Bug: uống bình HP/FP khi đang di chuyển thì không được tăng tốc~~ (Nexus, 13586927500, 2026-10-01) - **giới hạn, không sửa** (người dùng chốt 2026-10-02)
  - Uống khi chạy: `anim_queue` giữ anim chạy (`0201xx`), anim uống chỉ ở lớp nửa thân trên → mod áp `PlayerMovement`, không phải `PlayerItem` (người dùng xác nhận: tăng `PlayerMovement` thì uống khi chạy nhanh lên). Game chỉ có 1 `animation_speed` cho cả nhân vật
  - Đã thử: `time_act +0xD0` = anim nửa thân trên (050110→050111→050112) nhưng **không bị xoá khi uống xong** (giữ tới khi anim chính đổi) → dùng nó thì tốc độ Item kéo dài tới lúc ngừng chạy. `+0xC8`=-1, `+0xCC`=thời gian blend 0.2/0.233, `+0xD4`=0 - không có field "đang chạy"
  - Hardware breakpoint (DR0) trên `animation_speed`: game chỉ đọc ở 1 chỗ - getter `sub_14036834A` (`[rcx+18h]`, rcx = behavior+0x17B0, Arxan) gọi từ `sub_14041DCA0` (update behavior: dt × animation_speed × behavior+0x15C0 → hkbCharacter)
- [ ] Seamless Co-op: đồng bộ hình ảnh tốc độ giữa các máy - hướng B, gửi tốc độ qua mạng (người dùng chọn 2026-10-03)
  - Hiện tại (đã sửa Seamless hook getter `animation_speed`, `src/seamless.rs`): mỗi máy chỉ tăng tốc nhân vật của chính nó; nhân vật người khác trên máy mình chạy anim ở tốc độ thường → lệch hình ảnh (test 2 game qua Sandboxie, join `Attack = 5`: host thấy đòn chậm hơn). Sát thương/thời điểm trúng vẫn do máy người ra đòn quyết định - chỉ là hình ảnh
  - Hướng B: mỗi máy gửi `animation_speed` hiện tại của nhân vật mình (khi đổi) cho người khác qua Steam P2P (như `soulsteleport/src/steam.rs`, kênh riêng); máy nhận áp đúng giá trị đó cho ChrIns của người gửi (ghép Steam ID ↔ ChrIns như `soulsteleport/src/party.rs`; stub trả giá trị nhận được cho các ChrIns đó). Đúng cả khi 2 bên cấu hình khác nhau; trễ ~1 ping ở đầu mỗi đòn. Cần cả 2 bên cài mod
  - Đã cân nhắc hướng A (áp cấu hình của mình cho người khác, không cần mạng - chỉ đúng khi 2 bên cùng cấu hình) - người dùng chọn B
- [ ] Nghiên cứu: chỉnh tốc độ cho kẻ địch (Nexus, bloodaxis hỏi 2026-10-05: "an enemy only value"; đã trả lời "chưa chắc khả thi"; chưa rõ muốn tăng hay làm chậm, mọi kẻ địch hay chỉ boss)
  - bloodaxis kể đã thử làm kiểu tương tự qua HKS script của game (giống chế độ slow/turbo của Reforged) nhưng bỏ ý tưởng đó - hướng ghi `animation_speed` bên dưới là hướng khác
  - Khả thi về kỹ thuật: giống Torrent (`speed.rs`) - ghi `chr.modules.behavior.animation_speed`; danh sách nhân vật lấy từ `WorldChrMan.chr_sets` / `ChrSet::characters()` (`world_chr_man.rs:365`)
  - Khó: anim id kẻ địch khác người chơi (mỗi con 1 bộ) nên `group_of()` không dùng được → bản đơn giản = `[Enemy]` chỉ có `All`; cần lọc kẻ địch thật (team / chr type, bỏ NPC, spirit, người chơi khác); Seamless: thêm vào danh sách `set_own` của stub, máy khách không ghi (host điều khiển kẻ địch); anim bắt cặp (critical, bị túm), boss đổi phase/cutscene dễ lỗi
- [ ] Nghiên cứu: gắn tốc độ với equip load (Nexus, Sannh, 2026-10-05; comment chỉ ghi "Possible to tie this to equip load?", chưa rõ ý - đã hỏi lại)
  - Đọc được: `ChrCtrl.weight_type` (u32, `chr_ins.rs:601` - nghi là mức tải light/medium/heavy/overweight, chưa biết số nào ứng mức nào → log khi đổi trang bị) và `PlayerGameData.max_equip_load` (f32). Tải hiện tại thì không có field - phải tự cộng cân nặng trang bị hoặc đọc từ hook của `weightmultiplier`
  - Hướng rẻ nhất: dùng `weight_type` làm điều kiện mới (cạnh `SpEffect` trong `[[Override]]`, hoặc key `Light`/`Medium`/`Heavy`); theo % tải liên tục thì tốn công hơn
- [ ] Bug: Placidusax's Ruin bị lệch tia laser khi chỉnh tốc độ niệm phép (Nexus, bloodaxis, 2026-10-04)
  - Nghi: mod chỉ tăng tốc anim người chơi, tia laser (hiệu ứng/đạn game tạo ra) chạy theo thời gian riêng nên lệch nhịp - giống lỗi critical/backstab đã xử lý bằng `Player.Critical`
  - Cần: log anim id khi niệm phép này; hướng sửa có thể là giữ 1.0 cho anim đó (hoặc nhóm phép tương tự), cần xem có phép nào khác bị không
- [ ] Hệ số tốc độ đánh riêng cho từng loại vũ khí (Nexus, InvertedButt, 2026-10-01)
  - Prefix TAE của đòn đánh đã theo loại vũ khí (`a020`-`a062` loại chung, `a1xx`/`a2xx` vũ khí đặc biệt - xem `.docs/Elden Ring tae list updated for SOTE.txt`) - có thể map prefix → loại vũ khí; cần nghĩ cách đặt key ini cho gọn (~40 loại)
- [ ] Tốc độ giương cung / nạp nỏ (Nexus, TechnoMonkeyDJ, 2026-10-04: "bow draw speed and crossbows reload rate"; đã trả lời "sẽ xem")
  - Hiện chưa có nhóm riêng: các anim cung/nỏ rơi vào `Attack`/`Other` tuỳ đuôi anim id. Cần log anim id khi giương cung (giữ R1/R2), bắn, nạp nỏ để biết đuôi nào, rồi thêm key (vd. `Player.Bow`/`Player.Crossbow`) - có thể gộp chung với mục "theo loại vũ khí" ở trên
  - Lưu ý: nếu chỉ tăng tốc anim giương mà thời gian tụ lực/bắn tính theo timer riêng thì có thể lệch (giống lỗi Placidusax's Ruin) - cần test
- [x] Bug: critical (backstab / riposte) và sneak attack lệch nhịp với anim của kẻ địch (Nexus, InvertedButt, 2026-10-01) - xong 2026-10-02: `Player.Critical` (đuôi `031700`-`031799`, mặc định 1.0); đâm lén từ tư thế ngồi cũng là `0317xx`
  - Là anim cặp (pair anim) - player bị tăng tốc nhưng kẻ địch không. Hướng: giữ 1.0 cho các anim này (cùng ý với nhóm "luôn 1.0" cho anim chết / bị túm đã bàn trước 1.0.0); cần log anim id của critical/sneak attack

- [x] Hệ số tốc độ theo SpEffect đang có trên người chơi (Nexus, Lwingr - Linear Convergence, 2026-10-02)
  - Trong `SpeedMultiplier.toml` (đã chuyển sang TOML, commit `00d1321`): danh sách `[[Override]]` - tên chọn 2026-10-02 vì nói đúng việc nó làm (ghi đè `[Speed]` khi khớp)
  - Chỉ 1 loại điều kiện: `SpEffect` nằm thẳng trong `[[Override]]` (bỏ bảng con `When` - mod chỉ đổi tốc độ anim, SpEffect đã gồm buff/debuff/talisman; HP/vũ khí/giờ... là thừa, người dùng chốt 2026-10-02). Cần thêm điều kiện sau này thì thêm key cạnh `SpEffect`
  - `SpEffect = 1234` hoặc `SpEffect = [1234, 1235]` = có **bất kỳ** id nào. Id là số, nhận cả chuỗi `"1234"`. Override không có `SpEffect` = lỗi (báo dòng)
  - **Xếp chồng** (người dùng chốt 2026-10-02): mọi override khớp đều áp, từ trên xuống; trùng key thì override viết sau thắng. Mỗi key: bắt đầu từ `[Speed]` → lần lượt các override khớp có ghi key đó → giá trị cuối. `PlayerAll` cũng đi qua quy trình này (khác 1 → đè mọi `Player*` như cũ)
  - Mỗi frame duyệt `chr.special_effect.entries()` trước khi chọn giá trị trong `speed.rs::apply`
  - Không làm "reload khi load khu vực" (override áp theo frame nên không cần); `HideReloadMessage` cân nhắc sau

## DropMultiplier

- [ ] Tăng số lượng nhặt được từ cây/hoa hái trên bản đồ (Erdleaf Flower, Trina's Lily...) - không phải đồ quái rơi (Nexus, LordSoulOfNito, 2026-10-02)
  - Mod hiện chỉ chỉnh `ItemLotParam_enemy` (quái rơi); đồ hái/nhặt trên map nằm ở `ItemLotParam_map` - cần xem nên nhân `lotItemNum` (số lượng mỗi lần hái) hay tỉ lệ, và lọc đúng các dòng là cây/hoa (không đụng rương/đồ đặt sẵn); có thể là 1 key riêng, vd. `GatherMultiplier`

## RuneMultiplier

- [ ] Nghiên cứu: hệ số riêng cho từng nguồn rune, vd. giết địch x2, bán đồ x0.5 (Nexus, julianpratt, 2026-08-31)
  - Hiện tại 1 hệ số chung cho mọi nguồn (giết địch, dùng item, bán đồ...) - cần xem các nguồn có đi qua cùng 1 hàm cộng rune không, và có phân biệt được nguồn tại đó không

## Chung

- [ ] ~~Menu chỉnh cấu hình trong game dùng chung cho mọi mod~~ - **tạm dừng 2026-10-02** (người dùng chốt: chỉ dùng file cấu hình). Code ở nhánh `feat/modmenu` (không merge): bản v1 chạy được trong game (F10, SpeedMultiplier làm host)
  - Thiết kế đã chốt (dùng lại nếu quay lại): mỗi mod 1 tab; giao tiếp giữa DLL = file TOML + hàm C export (`alr_menu_schema_v1` / `alr_menu_config_path_v1` / `alr_menu_reload_v1`, host gọi reload sau khi ghi file); định nghĩa menu = file viết tay `<Mod>.menu.toml` nhúng vào DLL (bảng = nhóm, `Kind` Slider/Int/Bool/Key/Text/IdList, `Kind = "List"` + `Title` + `Optional` cho `[[Override]]`); UI chép từ SoulsTeleport (`common::menu`, feature `menu`)
  - Lý do dừng: SoulsTeleport có hudhook riêng (+ SoulsChat) → 2 hudhook trong game vẫn xảy ra dù bỏ SoulsTeleport ra, chưa test; còn bầu host giữa các phiên bản menu, Override, lưu vị trí, bắt phím; mỗi mod +~1 MB ImGui và phải phát hành lại khi DX12/hudhook đổi; chưa người dùng nào yêu cầu menu - file TOML (comment, reload, lỗi kèm dòng, tự thêm key, migration) đã đủ
  - Nếu quay lại: làm thành **1 mod menu riêng, tuỳ chọn**, chỉ đọc file cấu hình + `.menu.toml` của các mod (không nhét ImGui vào từng mod); test 2 hudhook (SoulsTeleport/SoulsChat) trước tiên

- [x] Thêm key `ReloadBanner` (bật/tắt banner "Config reloaded", banner lỗi luôn hiện) cho mọi mod có phím reload - SpeedMultiplier đã có từ 2026-10-02 (TOML, `reload::run_with`)
  - Mod dùng `common::reload::run(ini)`: dropmultiplier, infiniteailment, passiverunes, risearcher, sometweaks (bỏ), spiritmultiplier - đổi `reload::run` đọc `config::get_bool("ReloadBanner", true)` (thay `|| true`) rồi thêm key vào ini mẫu của từng mod (`[General]`, cạnh `ReloadKey`)
  - Xong 2026-10-05: autoregen + 5 mod dùng `common::reload::run`. runemultiplier / weightmultiplier có watcher riêng nhưng chưa từng hiện banner nên chưa thêm key (thêm banner mới là đổi hành vi, chưa làm)
- [x] Phiên bản mod trong log - hiện mọi DLL mod hiện `v0.0.0.0` ở "Loaded modules" (2026-10-03)
  - `v0.0.0.0` = DLL không có version resource (Windows "File version", `common::diag` đọc bằng `GetFileVersionInfoW`); `cdylib` của Rust không tự nhúng, `version` trong `Cargo.toml` không vào DLL
  - (1) Dòng khởi động ghi phiên bản: `Activating <Mod> <ver>...` từ `env!("CARGO_PKG_VERSION")` - mỗi mod 1 dòng (Cargo.toml đã khớp Nexus)
  - (2) Nhúng version resource bằng `build.rs` (crate `winresource`) điền từ `CARGO_PKG_VERSION` → "Loaded modules" và Properties của file hiện đúng, các mod thấy phiên bản của nhau trong log; cần `rc.exe` (Visual Studio Build Tools); làm helper chung để mỗi mod chỉ thêm `build.rs` ngắn
- [x] Đồng bộ `version` trong `Cargo.toml` của các mod cũ với version mới nhất trên Nexus - xong 2026-10-02 (autoregen 2.6.3, runemultiplier 1.0.4, weightmultiplier 2.0.1, passiverunes 2.1.1, dropmultiplier 1.1.0, infiniteailment 1.0.1, soulsteleport 1.1.0; risearcher 2.0.0 theo changelog local - trang Nexus ghi 1.17.1, API không có changelog)
- [ ] Đăng `windowresize` lên Nexus

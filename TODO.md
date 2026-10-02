# TODO

Việc cần làm, chia theo mod. Lý do / chi tiết kỹ thuật ghi trong
`crates/<mod>/README.md` (nhật ký phát triển), không ghi ở đây.

## SpiritMultiplier

- [x] GhostColor=false không có tác dụng với ELDEN RING Reforged (Nexus, Quantum240, 2026-09-30) - sửa ở 1.1.2 (tắt vfx màu, không gỡ SpEffect)
- [ ] Spirit dịch chuyển tới người chơi khi bị bỏ lại (hoãn từ trước 1.0.0; chỉ chỉnh `buddyWarp_*` không có tác dụng)
- [ ] Quyết định giữ hay bỏ log `stones:` trong SlotProbe (thêm để dò lỗi MultiSpirit/SummonAnywhere)
- [ ] `EnemyProbe` dễ crash - sửa hoặc bỏ hẳn
- [ ] Reforged: Spirit Fury / Fortune of the Spiritcaller hỏng - "cannot resummon/aggro the spirits", Spiritcaller ring không hoạt động (Nexus, puffintoast, 2026-10-01; chưa rõ bản mod họ dùng)
  - Cơ chế Reforged (CHANGELOG.txt của ERR): bấm lại Ash khi spirit đang ở ngoài = **kích nộ (Spirit Fury)**, không phải cho về; cho về bằng item Spirit-Severing Blade. "Aggro" = Spirit Fury, "un-aggro'ed" = trạng thái bình tĩnh mặc định
  - Nghi MultiSpirit: chiếm đúng thao tác bấm lại Ash (patch `DoSummon`, `DisappearAll`, `GetBuddyState`/`CanUseItem`/UI) - có thể đè lên hook của `reforged.dll` ở cùng hàm. Đã nhờ họ thử `MultiSpirit=false` (F5), chờ phản hồi
  - Hướng sửa lâu dài: nhận diện Reforged (vd. `reforged.dll` trong module đã nạp) rồi nhường thao tác bấm lại Ash cho Spirit Fury; cần tự tái hiện (cần fortune Spiritcaller, rơi từ boss Road's End Catacombs)
- [ ] Reforged: làm rõ đề xuất "summon spirits un-aggro'ed" (cùng người báo) - chờ họ giải thích
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

## SpeedMultiplier

- [ ] Tách tốc độ đi bộ / chạy / chạy nhanh (sprint) thành key riêng, cho cả player lẫn Torrent (Nexus, cfzlbj, 2026-10-01)
  - Hiện tại `PlayerMovement` và `Torrent` mỗi bên chỉ có 1 hệ số chung - cần dò đuôi anim từng kiểu: player đã thấy `020010`/`020110`/`020210` trong log nhưng chưa rõ cái nào là đi bộ / chạy / sprint; Torrent có `020000`-`020004`
- [ ] Bug: uống bình HP/FP khi đang di chuyển thì không được tăng tốc, chỉ đúng khi đứng yên (Nexus, 13586927500, 2026-10-01)
  - Nghi uống khi di chuyển dùng anim khác (bản vừa đi vừa uống / nửa thân trên) không khớp danh sách anim đang tăng tốc - cần log anim id lúc uống khi đứng yên vs khi di chuyển
- [ ] Hệ số tốc độ đánh riêng cho từng loại vũ khí (Nexus, InvertedButt, 2026-10-01)
  - Prefix TAE của đòn đánh đã theo loại vũ khí (`a020`-`a062` loại chung, `a1xx`/`a2xx` vũ khí đặc biệt - xem `.docs/Elden Ring tae list updated for SOTE.txt`) - có thể map prefix → loại vũ khí; cần nghĩ cách đặt key ini cho gọn (~40 loại)
- [ ] Bug: critical (backstab / riposte) và sneak attack lệch nhịp với anim của kẻ địch (Nexus, InvertedButt, 2026-10-01)
  - Là anim cặp (pair anim) - player bị tăng tốc nhưng kẻ địch không. Hướng: giữ 1.0 cho các anim này (cùng ý với nhóm "luôn 1.0" cho anim chết / bị túm đã bàn trước 1.0.0); cần log anim id của critical/sneak attack

- [ ] Hệ số tốc độ theo SpEffect đang có trên người chơi (Nexus, Lwingr - Linear Convergence, 2026-10-02)
  - Trong `SpeedMultiplier.toml` (đã chuyển sang TOML, commit `00d1321`): danh sách `[[Override]]` - tên chọn 2026-10-02 vì nói đúng việc nó làm (ghi đè `[Speed]` khi khớp), vẫn đúng nếu sau này thêm điều kiện khác SpEffect (vũ khí, % máu...)
  - Điều kiện tách vào bảng con `When = { ... }` (không lẫn với key tốc độ; thêm loại điều kiện mới = thêm field). Giữa các loại điều kiện là "và"; `When` rỗng = lỗi; điều kiện lạ (mod cũ gặp field mới) = lỗi, không bỏ qua
  - `SpEffect = 1234` (1 id) hoặc `SpEffect = [1234, 1235]` = **phải có đủ** mọi id (người dùng chốt 2026-10-02); "hoặc" = viết nhiều `[[Override]]`. Id là số, nhận cả chuỗi `"1234"`
  - Thứ tự trong file = ưu tiên (override đầu tiên khớp thắng); chỉ ghi key muốn đổi (`Player*`, `Torrent`), còn lại giữ giá trị `[Speed]`
  - Mỗi frame duyệt `chr.special_effect.entries()` trước khi chọn giá trị trong `speed.rs::apply`
  - Không làm "reload khi load khu vực" (override áp theo frame nên không cần); `HideReloadMessage` cân nhắc sau

## RuneMultiplier

- [ ] Nghiên cứu: hệ số riêng cho từng nguồn rune, vd. giết địch x2, bán đồ x0.5 (Nexus, julianpratt, 2026-08-31)
  - Hiện tại 1 hệ số chung cho mọi nguồn (giết địch, dùng item, bán đồ...) - cần xem các nguồn có đi qua cùng 1 hàm cộng rune không, và có phân biệt được nguồn tại đó không

## Chung

- [ ] Menu chỉnh cấu hình trong game dùng chung cho mọi mod: mỗi mod cài vào = 1 tab (ý tưởng người dùng, chốt thiết kế 2026-10-02)
  - **File TOML là giao diện duy nhất giữa các mod** - không đăng ký qua C ABI, không gọi hàm qua lại giữa các DLL
  - Format: giá trị ở bảng thường (`[Speed]`...), metadata GUI ở `[Meta]`: `Version` (phiên bản format menu), `Tab`; `[Meta.<Bảng>]` mỗi key 1 inline table `{ Label, Description, Min, Max, Step }`, kiểu control suy từ kiểu giá trị (số → thanh trượt, bool → checkbox), `Kind = "Key"` → ô bắt phím. Khai báo `[Meta]` trong template ở repo (`crates/<mod>/<Mod>.toml`)
  - `toml_config` ghi đè `[Meta]` từ template mỗi lần khởi động (để metadata luôn đúng phiên bản mod), không đụng giá trị người dùng; struct `Config` của mod bỏ qua `[Meta]`
  - Host tìm mod: duyệt module đã nạp (`common::diag`), tìm `<TênDLL>.toml` cạnh DLL; có `[Meta]` với `Version` host hiểu → 1 tab; không có file / lỗi cú pháp / không có `[Meta]` → bỏ qua
  - GUI chỉnh → host ghi file bằng `toml_edit` (giữ comment); mod tự reload khi mtime file đổi (poll ~1s) - kèm luôn lợi ích sửa tay không cần F5 (F5 vẫn giữ)
  - Bầu host: chỉ 1 DLL được gắn `hudhook` (2 hudhook trong 1 process xung đột). Mọi mod mang code menu trong `common`; mod có phiên bản menu mới nhất làm host - trao đổi số phiên bản qua vùng nhớ dùng chung có tên (named file mapping)
  - Dùng lại từ SoulsTeleport: `hudhook` (dx12) + imgui (`ui.rs`), chặn input game khi mở menu (`input_block.rs`). Rủi ro: overlay khác (ReShade, mod imgui khác)
  - Thứ tự: (1) `[[Override]]` SpeedMultiplier trước (để biết metadata cho "danh sách bảng"); (2) `common::menu` chạy với 1 mod (SpeedMultiplier), chưa bầu host; (3) bầu host + thử với mod thứ 2; (4) chuyển dần các mod khác sang TOML + `[Meta]`

- [ ] Đồng bộ `version` trong `Cargo.toml` của các mod cũ với version mới nhất trên Nexus (vd. `autoregen` vẫn `2.0.0`)
- [ ] Đăng `windowresize` lên Nexus

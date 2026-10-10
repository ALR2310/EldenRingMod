# TODO

Chỉ liệt kê việc **còn mở**, mỗi việc 1 dòng. Việc xong xoá đi (lịch sử ở
`mods/<mod>/HISTORY.md`); lý do / chi tiết kỹ thuật ở `mods/<mod>/docs/`.
Quy ước đầy đủ: `CLAUDE.md`, mục "Viết `TODO.md` gọn".

## SpiritMultiplier

Chi tiết: `mods/spiritmultiplier/docs/open_issues.md`, `docs/warp_research.md`

- [ ] Reforged: spirit gọi ra mặc định ở trạng thái đỏ (Fury) thay vì trắng
- [ ] Reforged + MultiSpirit: kích Fury riêng cho từng Ash (hiện Fury 1 nhóm thì mọi Ash đều xám)
- [ ] Reforged: NoRestResummon không có tác dụng sau Spirit-Severing Blade (nghi luật Reforged, không phải mod)
- [ ] Seamless Co-op: spirit mất màu ma (tạm dừng 2026-10-02)
- [ ] Spirit dịch chuyển tới người chơi khi bị bỏ lại (hoãn)
- [ ] Cho triệu hồi cùng 1 Ash nhiều lần, tối đa X con, quá X thì con cũ nhất biến mất (almasakmal123, 2026-10-08)
- [ ] [research] Spirit cỡ lớn (mod Elden Lord Army of Summons) đứng im đến khi địch lại gần người chơi; muốn chủ động tấn công (almasakmal123, 2026-10-08); thử bằng `SpiritThinkParam.ini`, xem `mods/spiritmultiplier/docs/think_override.md`
- [ ] Spirit không bị sát thương khi rơi từ chỗ cao (almasakmal123, 2026-10-08); phần đưa về chỗ người chơi khi rơi khỏi map gộp vào warp hoãn ở trên

## SpeedMultiplier

- [ ] Seamless Co-op: đồng bộ hình ảnh tốc độ giữa các máy bằng Steam P2P (hướng B, chốt 2026-10-03; xem `docs/seamless.md`)
- [ ] Test `Critical` khác 1 với nạn nhân backstab/riposte (`docs/critical_victim.md`)
- [ ] Hệ số tốc độ đánh theo loại vũ khí (InvertedButt, 2026-10-01; gộp cả cung/nỏ riêng, tạm không làm)
- [ ] Tuỳ chọn: Seamless cho Spirit / Enemy / nạn nhân critical; chia nhóm boss / spirit

## DropMultiplier

- [ ] Nhân số lượng đồ hái trên map (Erdleaf Flower, Trina's Lily...; `ItemLotParam_map`, LordSoulOfNito, 2026-10-02)

## RuneMultiplier

- [ ] Test trong game reload qua `common::reload` + banner (2026-10-07), rồi phát hành bullet ở `[Unreleased]`
- [ ] Nghiên cứu hệ số rune riêng theo nguồn (giết địch / bán đồ...; julianpratt, 2026-08-31)

## Chung

- [ ] `ReloadKey` lọt sang instance game khác: `common::reload` và autoregen nên dùng `common::input::is_key_pressed` (xem `mods/soulsteleport/docs/position_exchange.md`)
- [ ] `risearcher` (`weapon.rs`, `bullet.rs`), `infiniteailment` (`status_effect.rs`): chuyển sang `common::params::for_each_row_mut` (xem `mods/dropmultiplier/docs/convergence_panic.md`)
- [ ] `ReloadBanner` cho `weightmultiplier` - cần quyết định (phải bỏ thiết kế không dùng task game)
- [ ] Dọn tham chiếu `.docs/...` lỗi thời (~47 dòng / 15 file)
- [ ] `mods/risearcher/project.json` chứa đường dẫn máy: git-ignore hoặc đổi thành file mẫu

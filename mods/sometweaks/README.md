# SomeTweaks

> **Đã bỏ** (2026-09-29, không có kế hoạch quay lại) · **chưa từng đăng Nexus**

DLL gộp nhiều mod Quality-of-Life cho Elden Ring thành 1 file (`SomeTweaks.dll`),
mỗi tính năng bật/tắt riêng qua `SomeTweaks.ini`. Tên cũ **LifeBetween** (chơi
chữ "Lands Between" + "Better Life"; đổi 2026-08-18 khi gộp vào workspace).
Mod không còn được phát triển: từng tính năng được tách ra thành mod riêng, tốt
hơn và có đăng Nexus. **Đừng backport sửa lỗi vào đây**; nếu cần 1 tính năng thì
port sang mod đích (bảng dưới). Thư mục được giữ lại vì còn vài tính năng chưa
có ở mod nào khác (GraceMenu, TorrentAnywhere, WarpAnywhere, UnlockAshesOfWar,
UnlockEnchantments, Rune.KeepOnDeath) và phần kiến thức dịch ngược.

## Cấu hình

Cấu hình trong [`SomeTweaks.ini`](SomeTweaks.ini) (nhúng vào DLL, tự tạo nếu thiếu, key mới thêm vào file có sẵn, key đã đổi tên dồn vào `[Legacy]`); tên key, mặc định và ý nghĩa nằm ở chú thích file đó. Mỗi section ini ứng với đúng 1 thư mục trong `src/`.

## Các tính năng và nơi chúng sống bây giờ

| Section / tính năng | Trạng thái | Dùng thay thế / tài liệu |
|---|---|---|
| `[Regen Per Tick]`, `[Regen Per Hit]` (`src/regen/`) | đã test | mod `autoregen` (bản mới hơn, nhiều tính năng hơn) |
| `[Rune Reward]` passive + milestone (`src/rune/reward.rs`) | đã test | mod `passiverunes` |
| `Rune.Multiplier` (`src/rune/multiplier.rs`) | đã test | mod `runemultiplier` |
| `WeightMultiplier` (`src/misc/weight_multiplier.rs`) | đã test | mod `weightmultiplier` |
| `[Drop Rate]` (`src/drop_rate/`) | đã test | mod `dropmultiplier` |
| `[Spirit]` color/regen/Summon.Amount/Multiplier/Anywhere (`src/spirit/`) | Anywhere xung đột với Seamless | mod `spiritmultiplier`; kiến thức: [docs/spirit.md](docs/spirit.md) |
| `[Grace Menu]` Nâng cấp / Mua / Bán / UnlockShop (`src/grace_menu/`) | đã test, giật 1-2s ở "Mua" | không có mod khác; [docs/grace_menu.md](docs/grace_menu.md) |
| `TorrentAnywhere`, `WarpAnywhere`, `UnlockAshesOfWar`, `UnlockEnchantments`, `Rune.KeepOnDeath` (`src/misc/`, `src/rune/keep_on_death.rs`) | đã test | không có mod khác; [docs/misc_patches.md](docs/misc_patches.md) |
| `Misc.GraceOnTorrent`, `Enemy Scaling` | đã xoá | lý do trong [docs/grace_menu.md](docs/grace_menu.md), [docs/misc_patches.md](docs/misc_patches.md) |
| Menu cấu hình trong game (`hudhook`) | đã thử và bỏ | [docs/misc_patches.md](docs/misc_patches.md); nhánh `feat/modmenu` |

## Cách hoạt động / kiến trúc

- Crate `cdylib` Rust; đọc/ghi `WorldChrMan`/`ChrIns`/`CSChrDataModule` qua `fromsoftware-rs` (`eldenring` 0.14.0), phần không có API (hook hit, patch code, ESD) tự viết bằng AOB + trampoline (`global_asm!`) trong `common::codepatch`.
- Mọi tick đăng ký trên `CSTaskGroupIndex::FrameBegin`, chờ `CSTaskImp` đúng 1 lần cho cả DLL (retry `InvalidRva`), gate "đã vào world" bằng `common::player` trước khi chạm `WorldChrMan`/`SoloParamRepository` (nếu không crash không log).
- Param sửa theo kiểu "snapshot giá trị gốc 1 lần rồi luôn tính từ snapshot" để hot-reload không cộng dồn (xem lỗi nhân dồn ở [docs/spirit.md](docs/spirit.md)); duyệt qua `common::params::for_each_row_mut` (xem `mods/dropmultiplier/docs/convergence_panic.md`).
- Quy ước log: mọi dòng của 1 tính năng mở đầu bằng `<IniKeyPrefix>: `; level `INFO/WARN/ERROR/DEBUG`.

## Giới hạn

- Hook hit và các patch code verify trên 1 vài bản game (đến 2.7.1.0); không đảm bảo trên bản mới.
- `Spirit.Summon.Anywhere` xung đột runtime với Seamless Co-op (chưa giải quyết ở mod này).
- Không dùng chung với `er10x.dll`, `DisableRuneLoss.dll`, hay bất kỳ DLL tham khảo nào patch cùng chỗ.

## Tài liệu liên quan

- [HISTORY.md](HISTORY.md): dòng thời gian phát triển.
- [docs/grace_menu.md](docs/grace_menu.md): chèn mục vào menu Site of Grace bằng ESD sống, text tuỳ biến, giật khi mua.
- [docs/spirit.md](docs/spirit.md): giải mã `er10x.dll`, các ngõ cụt của Summon.Anywhere, bài học cô lập test.
- [docs/misc_patches.md](docs/misc_patches.md): TorrentAnywhere, WarpAnywhere, UnlockAshesOfWar/Enchantments, KeepOnDeath, EnemyScaling.

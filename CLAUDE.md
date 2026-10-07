# Project Rules

## Tài liệu của mỗi mod: README, CHANGELOG, HISTORY, docs

Mỗi mod có 4 loại tài liệu, mỗi loại 1 vai trò, không trộn lẫn:

- `mods/<mod>/README.md` = **trạng thái hiện tại** (không phải nhật ký):
  version, Nexus ID, trạng thái test; bảng key cấu hình; cách hoạt động;
  giới hạn; link tới CHANGELOG, HISTORY và docs. Khuôn mẫu:
  `mods/dropmultiplier/README.md`.
- `mods/<mod>/CHANGELOG.md` = ghi chú phát hành cho **người dùng cuối**, theo
  từng version (mới nhất ở trên); mục `## [Unreleased]` gom các thay đổi chưa
  phát hành. Mỗi bullet 1 dòng ngắn, văn bản thuần không định dạng, nói điều
  người dùng thấy, không ghi tên key ini hay chi tiết nội bộ (văn phong: xem
  memory `feedback_changelog_style`). Skill `deploy-nexus-mod` gửi nội dung này
  lên tab Changelog của Nexus; `nexus_page.bbcode` (trang mod) **không** chứa
  changelog.
- `mods/<mod>/HISTORY.md` = dòng thời gian, bảng `Ngày | Thay đổi | Chi
  tiết`, mỗi thay đổi **1 dòng**, mới nhất ở dưới cùng; chuyện nào có phân
  tích dài thì cột "Chi tiết" link sang file trong `docs/` của mod.
- `mods/<mod>/docs/<chủ đề>.md` = phân tích theo chủ đề (vd.
  `mods/dropmultiplier/docs/materials.md`), viết theo **trạng thái hiện
  tại** (không viết kiểu nhật ký theo ngày): mở đầu bằng `**Status <ngày>:
  ...**`, rồi kết luận, bằng chứng, hướng đã thử/bỏ (kèm lý do), tham chiếu
  code. Chủ đề liên mod và tư liệu dùng chung (trang Nexus, prompt
  logo/thumbnail, spec Nexus API) nằm ở `docs/` gốc repo. Công cụ dịch ngược
  nằm ở `scripts/` (IDA: `scripts/ida/`); dữ liệu sinh ra từ dịch ngược nằm ở `dumps/` (git-ignored,
  ngoại lệ `dumps/seamless-diff/`).

Sau khi hoàn thành một thay đổi đáng kể ở 1 mod (thêm tính năng, đổi ini
key, sửa bug về hành vi, đổi cấu trúc code/thư mục):

1. **Luôn thêm 1 dòng** vào bảng `HISTORY.md`, ghi ngày hôm nay: đã đổi gì và
   vì sao (1-2 câu, đặc biệt nếu sửa 1 hiểu lầm/bug trong thiết kế trước đó).
2. **Nếu người dùng nhìn thấy thay đổi đó** (tính năng, sửa lỗi, đổi hành vi):
   thêm 1 bullet vào `## [Unreleased]` trong `CHANGELOG.md`.
3. **Sửa `README.md` tại chỗ** nếu trạng thái, bảng key, cách hoạt động hay
   giới hạn thay đổi (không thêm mục có ngày vào README); sửa mọi đường dẫn
   file/tên key lỗi thời được nhắc tới.
4. **Viết mới hoặc bổ sung `mods/<mod>/docs/<chủ đề>.md`** chỉ khi có phát hiện
   đáng giữ: nguyên nhân gốc, hướng đã thử/bỏ, số liệu, RVA/AOB. Cập nhật dòng
   `Status`, rồi link từ dòng HISTORY và từ README.

Mod chưa được tách (README vẫn là nhật ký dài theo ngày) thì giữ kiểu cũ
cho tới khi tách: thêm 1 mục có ngày vào README của mod đó. Đã tách xong: `dropmultiplier`.
`CHANGELOG.md` đã có cho mọi mod (trừ `sometweaks`) và áp dụng cho cả mod chưa tách.

Không cần hỏi lại người dùng trước khi làm việc này - tự làm ngay sau khi
code đã ổn định, coi đây là 1 bước không thể thiếu của việc "xong việc",
giống như build/test.

## Cập nhật `.vscode/tasks.json` khi thêm / xoá / đổi tên mod

Mỗi mod trong `mods/` có 1 task build riêng trong `.vscode/tasks.json`
và nằm trong `dependsOn` của task `Cargo: Build All (Release)`. Khi
**thêm, xoá hoặc đổi tên** 1 mod, **luôn cập nhật file này cùng lúc**:

- Thêm: 1 task mới theo đúng mẫu các task sẵn có - `label` =
  `Cargo: Build <Mod> (Release)`, `-Mod <Mod>` với `<Mod>` lấy từ
  `[lib] name` trong `mods/<mod>/Cargo.toml` (PascalCase, không tự
  viết hoa tên thư mục) - và thêm label đó vào `dependsOn` của Build All.
- Xoá: bỏ task của mod đó và dòng tương ứng trong `dependsOn`.
- Đổi tên: sửa `label`, `detail`, `-Mod` và dòng trong `dependsOn`.

Sau khi sửa, kiểm tra file vẫn là JSON hợp lệ và số task build riêng
bằng đúng số thư mục trong `mods/`. Không cần hỏi lại người dùng.
Người dùng yêu cầu (2026-09-30) sau khi phát hiện thiếu task của
FasterRevival, SpeedMultiplier, SpiritMultiplier, WindowResize.

## Đối chiếu changelog thật trên Nexus Mods trước khi chốt version trong `CHANGELOG.md`

Trước khi cắt `[Unreleased]` thành 1 mục version (hoặc sửa mục version cũ)
trong `mods/<mod>/CHANGELOG.md`, **luôn gọi Nexus Mods API để lấy changelog
thật hiện có trên trang mod** trước, rồi mới chốt version nối tiếp đúng theo đó - không tự đặt số phiên
bản/nội dung dựa trên suy đoán hay dựa vào lịch sử trong README/HISTORY (lịch sử
này ghi theo ngày phát triển nội bộ, không phải số phiên bản đã publish
trên Nexus, 2 thứ có thể lệch nhau).

Cách gọi:

```bash
set -a; . <(tr -d '\r' < .env); set +a   # nạp NEXUS_API_KEY
curl -sS -H "apikey: $NEXUS_API_KEY" \
  "https://api.nexusmods.com/v1/games/eldenring/mods/<mod_id>/changelogs.json"
```

Mod ID theo từng mod trong workspace này (game domain luôn là `eldenring`):

- `autoregen` → 10548
- `runemultiplier` → 10630
- `weightmultiplier` → 10549 (Weight Multiplier)
- `passiverunes` → 10528
- `risearcher` → 5807
- `sometweaks` → **chưa có, chưa đăng lên Nexus**
- `soulsteleport` → 11119 (Souls Teleport)
- `fasterrevival` → 11160 (Faster Revival)
- `spiritmultiplier` → 11168 (Spirit Multiplier)
- `speedmultiplier` → 11173 (Speed Multiplier)
- `windowresize` → **chưa có, chưa đăng lên Nexus**

Key API cá nhân của người dùng lưu ở biến `NEXUS_API_KEY` trong `.env` ở
gốc repo (mẫu: `.env.example`; chỉ dùng nội bộ cho quy trình phát hành,
không mod nào đọc nó lúc chạy/build; đã
gitignore, không commit). Nếu API trả về thiếu 1 vài version cũ so với thực
tế trên trang web (đã xảy ra 1 lần, 2026-09-03) - đó là do giới hạn của
API, không phải version đó không tồn tại - hỏi lại người dùng xác nhận
qua trang web thật trước khi tự xoá bất kỳ mục changelog cũ nào.

## Bump `version` trong `Cargo.toml` cùng lúc với version mới trong `CHANGELOG.md`

Khi cắt `[Unreleased]` thành 1 mục version mới trong
`mods/<mod>/CHANGELOG.md` (thường do skill `deploy-nexus-mod` làm lúc phát
hành), **luôn đổi luôn `version` trong
`mods/<mod>/Cargo.toml` thành đúng số version đó** (vd. changelog
`1.1.0` → `version = "1.1.0"`), rồi build lại để `Cargo.lock` cập nhật
theo. Version crate phải luôn khớp với version mới nhất trên Nexus.

Không cần hỏi lại người dùng - coi đây là 1 phần của việc chốt version.
Người dùng yêu cầu (2026-09-29, SpiritMultiplier 1.1.0) vì trước đó version
crate không được bump (vd. `autoregen` vẫn `2.0.0` trong khi Nexus đã lên
2.6.x).

## Kiểm tra `shared/` (`common`) trước khi viết helper mới

Trước khi viết 1 hàm/struct/module mới (helper gate "đã vào game", chờ
singleton, đăng ký task, quét AOB, đọc/ghi config, log, banner thông
báo...), **luôn tìm xem đã có sẵn chưa** - trước hết trong `shared/src`
(crate `common`, mọi mod dùng chung), rồi đến các mod khác trong `mods/`
(có thể đã tự viết 1 bản chưa kịp đưa lên `common`). Tìm theo cả tên lẫn
chức năng (vd. grep `main_player`, `WorldChrMan`, `instance()`), không chỉ
theo đúng tên định đặt.

- Đã có trong `common` → dùng luôn, không tự viết bản riêng.
- Có ở 1 mod khác nhưng chưa ở `common`, và mod đang sửa cũng cần → đề
  xuất đưa lên `common` thay vì copy thêm 1 bản.
- Chỉ viết mới khi chắc chắn không có gì tương đương; nếu bản sẵn có
  không đủ (vd. chỉ đọc, cần ghi), nói rõ lý do trong comment.

Ví dụ đã xảy ra (2026-09-23): tự viết `in_game()` trong
`passiverunes/src/rune.rs` trong khi `common::player::main_player_chr_ins_ptr()`
đã làm đúng việc đó và được `DropMultiplier`/`SomeTweaks`/`AutoRegen` dùng
sẵn.


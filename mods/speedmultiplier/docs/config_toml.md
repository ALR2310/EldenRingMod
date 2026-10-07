# SpeedMultiplier: cấu hình TOML, ghi đè theo điều kiện, migration

**Status 2026-10-06: ĐANG PHÁT HÀNH (1.1.0 trở đi), đã test trong game.** Code:
`mods/speedmultiplier/src/config.rs`, `shared/src/toml_config.rs`. Mod này là mod
duy nhất dùng TOML (`SpeedMultiplier.toml`), các mod còn lại dùng `.ini`.

## Vì sao chuyển từ ini sang TOML

Yêu cầu trên Nexus (Lwingr, Linear Convergence): hệ số tốc độ khác nhau theo SpEffect
đang có trên người chơi. Đó là một **danh sách quy tắc**, mà ini của workspace không chứa
nổi: `common::config` gộp mọi `[Section]` vào một namespace (phải đánh số
`PlayerMovement01`, `02`...), không có mảng, và `migrate` dựa theo tên key. Người dùng
chọn chuyển hẳn mod này sang TOML trước khi làm tính năng, để còn mở rộng.

Không tự chuyển giá trị từ ini cũ (ini còn ít key); người cập nhật từ 1.0.0 được file
mặc định mới và ini cũ không còn được đọc.

## `common::toml_config` (dùng chung, các mod ini không đổi gì)

- Mỗi mod khai báo một struct serde (`#[serde(default, deny_unknown_fields)]`). Key
  sai tên hoặc sai kiểu là lỗi kèm số dòng (thông báo của crate `toml`); **file lỗi không
  bao giờ thay cấu hình đang chạy** (khởi động: chạy bằng mặc định của template; reload: giữ
  bản cũ).
- Khởi động: chưa có file thì ghi template; có rồi thì thêm key/bảng còn thiếu từ template
  bằng `toml_edit` (giữ comment, giá trị và bố cục của người dùng), tương đương
  `config::migrate` của ini. File có sẵn **không nhận comment mới** của template (chỉ thêm
  key thiếu).
- Cấu hình là snapshot `Arc<T>`: mỗi tick lấy một lần rồi đọc field, không tra chuỗi mỗi giá
  trị.
- `ReloadKey` nhận chuỗi hoặc số (`key_name`): trong TOML `0x75` không có nháy là số nguyên
  (ini nhận cả hai), lần đầu từng báo lỗi "invalid type: integer `117`, expected a string".
- `logger::set_enabled(bool)` cho mod TOML tự bật/tắt log; `reload::run_with(key_fn,
  banner_fn, reload_fn)` là watcher F5 cho cấu hình không phải ini: reload lỗi thì ghi log
  và hiện banner "Config error - see log" thay cho "Config reloaded" (banner lỗi luôn hiện,
  tắt luôn thì reload hỏng sẽ im lặng). `reload::run(ini)` giờ gọi lại `run_with`.
- Dependency workspace thêm: `serde`, `toml`, `toml_edit`. `scripts/build-mod.ps1` tìm
  `<Mod>.toml` trước, rồi `<Mod>.ini`.

## Mặc định chỉ nằm trong template (2026-10-02, nhánh `feat/modmenu`)

Trước đó mỗi giá trị mặc định viết hai lần (template TOML và `impl Default` trong
`config.rs`) và giữ khớp bằng test. Giờ cấu hình **xếp lớp**: template là lớp dưới, file
người dùng là lớp trên. `parse` đọc hai lượt: (1) chỉ file người dùng, cho lỗi kèm số dòng
(key thiếu nhận giá trị rỗng của `#[derive(Default)]`); (2) giá trị người dùng đặt lên
template (bảng trộn từng key, giá trị/mảng/`[[...]]` thay nguyên) ra cấu hình thật. File
lỗi chạy bằng mặc định của template. Struct chỉ `#[derive(Default, Serialize)]` (Default
là rỗng, không bao giờ là cấu hình đang chạy); `config::template()` là mặc định. Test
`template_sets_every_field` báo field nào template chưa khai báo. Đây là bước đầu của việc
làm menu trong game (menu lấy metadata từ struct, mặc định từ template); việc menu đang
được tạm dừng.

Hiệu năng: đọc hai lượt chỉ chạy lúc khởi động hoặc reload (cỡ trăm µs); DLL lớn lên
một chút. Người dùng chốt kích thước vài MB không đáng lo, chỉ quan tâm hiệu năng.

## Bố cục `[Player]` / `[Torrent]`

`[Speed]` với key `Player*`/`Torrent` đổi thành hai bảng: `[Player]` (`All`, `Walk`,
`Run`, `Sneak`, `Jump`, `Roll`, `Ladder`, `Attack`, `Critical`, `Skill`, `Cast`, `Item`,
`Other`) và `[Torrent]` (`All`, `Walk`, `Run`, `Jump`, `Other`). `All` khác 1 đè mọi key
của bảng đó. Giá trị tốc độ bị kẹp trong một khoảng an toàn (NaN/vô cực về 1). Danh sách
nhóm và cách phân loại: `action_groups.md`.

## `[[Override]]`: tốc độ khác theo điều kiện

Thiết kế chốt với người dùng sau vài vòng:

- **Tên `[[Override]]`** (không phải `Rule`/`Condition`/`Profile`): nói đúng việc nó làm,
  ghi đè giá trị khi khớp. `[[...]]` là danh sách bảng TOML, viết bao nhiêu khối cũng được.
- **Điều kiện `SpEffect`** nằm thẳng trong khối (đã bỏ bảng con `When`): mod chỉ đổi tốc độ
  anim, SpEffect đã gồm buff/debuff/talisman; HP/vũ khí/giờ trong game là thừa.
  `SpEffect = 1234` hoặc `[1234, 1235]` = có **bất kỳ** id nào (từng cân nhắc "mảng = phải có
  đủ" rồi bỏ). Id số hoặc chuỗi; mảng rỗng, id không phải số, thiếu điều kiện, key lạ = lỗi
  kèm dòng.
- **Điều kiện `EquipLoad`** (2026-10-06, yêu cầu Nexus Sannh): thêm cạnh `SpEffect`, không
  thêm bảng mới. Lúc đầu tên `Load`, đổi ngay thành `EquipLoad` vì `Load` đơn lẻ dễ nhầm
  với "load game/config". Giá trị `Light`/`Medium`/`Heavy`/`Overweight` (không phân biệt hoa
  thường) hoặc danh sách. Override có `SpEffect`, `EquipLoad` hoặc cả hai (cả hai phải
  khớp); không có điều kiện nào là lỗi.
- **Xếp chồng:** mọi override khớp đều áp từ trên xuống, trùng key thì khối viết sau thắng
  (hai buff độc lập, một tăng lăn một tăng đánh, có tác dụng cùng lúc, không phải viết khối
  gộp như kiểu "khối đầu tiên thắng"). `All` cũng đi qua quy trình này. Torrent theo
  SpEffect của người chơi.
- Cú pháp key trong override: dotted key `Player.Roll = 1.5` / `Torrent.Run = 2` (bảng con
  `[Override.Player]` cũng được); key nào bỏ thì giữ giá trị ở bảng gốc.

Cách nhận biết trong game: `SpeedProbe` có hai công cụ riêng, `EffectProbe` (log SpEffect
của người chơi khi đổi, `P SpEffect +[thêm] -[mất] now [...]`; tách khỏi `SpeedProbe` cùng
ngày, không quảng cáo trong comment vì người cần tính năng tự biết tra id) và dòng log
`P weight_type=N max_equip_load=X`. Ví dụ đã đo: Golden Vow (phép) ứng với SpEffect `1660000`/
`1660001`/`1660002`, khoảng 80 giây (Smithbox xác nhận; bản kỹ năng vũ khí là `1730`, bản
item `20503170`). Lớp tải: `ChrCtrl.weight_type` = 1, 2, 3, 4 theo đúng thứ tự (0 = chưa
nạp dữ liệu lúc mới vào game), anim lăn đổi theo (`027110` ở 2, `027120` ở 3, `027130` ở
4), và `max_equip_load` đổi độc lập (đổi talisman). Chốt: 1 Light, 2 Medium, 3 Heavy, 4
Overweight. Lớp tải chưa rõ (0 hoặc ngoài 1-4) thì override có `EquipLoad` không khớp.
`weightmultiplier` đổi tổng tải nên lớp tải cũng đổi theo. Tải liên tục theo % chưa làm.

Log: `Speed: active overrides: #1, #2` / `none` khi tập override đang bật đổi.

## Migration theo phiên bản

ini chuyển key lạ vào `[Legacy]`, TOML trước đây báo lỗi. Giờ lúc khởi động, key không có
trong template được gỡ và ghi lại thành comment ở cuối file (log cảnh báo); F5 vẫn báo
lỗi kèm dòng (người dùng đang sửa file); `[[Override]]` được giữ.

Bản đầu dùng danh sách đổi tên phẳng, nhưng khi cân nhắc nhảy phiên bản (1.0.0 lên 1.2.0)
thì danh sách phẳng phụ thuộc thứ tự, không tách/quy đổi được key, và xoá một cặp là mất
giá trị. Thay bằng **migration theo phiên bản**: `General.ConfigVersion` (phiên bản hiện
tại = giá trị trong template), `STEPS[i]` đưa file từ `i+1` lên `i+2`, mỗi bước chạy đúng
một lần mỗi file; helper `rename_key`/`get_value`/`remove_value`/`set_value`; file không
có version = 1; file mới hơn mod (hạ cấp) không bị đụng. **Bước đã phát hành không bao giờ
được sửa**, sửa sai bằng bước mới. Hiện `ConfigVersion = 1` là định dạng TOML phát hành đầu
(1.1.0) và `STEPS` rỗng; test `steps_cover_every_version`.

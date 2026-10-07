# SpeedMultiplier: phân nhóm hành động

**Status 2026-10-06: ĐANG PHÁT HÀNH (1.2.0), mọi nhóm đã test trong game.** Code:
`speed::group_of` trong `mods/speedmultiplier/src/speed.rs` (có test cho từng dải).
Anim id đang chạy lấy từ `CSChrTimeActModule.anim_queue`, dạng `aXXX_YYYYYY`; tài liệu
tra prefix `XXX`: `tae-ids.md`.

## Quy tắc phân loại

1. **Theo prefix TAE** cho phép và Sword Art, vì hai loại này chia sẻ đuôi `04xxxx`
   nên không tách được bằng đuôi:
   - `400..600` → **Cast** (sorcery/incantation); ví dụ `a435_045100/045110/045111` là
     incantation Golden Vow.
   - `600..1000` → **Skill** (Ash of War/Sword Art), trừ một số prefix của bộ chiêu vũ khí
     đặc biệt nằm trong dải đó (`WEAPON_TAES_IN_SKILL_RANGE`); ví dụ `a885_040xxx` là AoW
     Carian Sovereignty.
2. **Theo đuôi** (`anim_id % 1_000_000`) cho mọi thứ còn lại (bảng dưới).
3. Mọi anim không khớp → **Other**.

| Nhóm | Đuôi | Ghi chú |
|---|---|---|
| Walk | `0201xx`, `0221xx` | đi bộ (và dừng) |
| Run | `0202xx`, `0222xx` | chạy (giữ phím chạy) |
| Sneak | `300000`-`399999` | đi lén; đứng yên khi lén `300000`, vào tư thế `390000` |
| Jump | `2020xx`, `2021xx` | cất nhảy (`202000` đứng, `202020` đi, `202030/40` chạy), tiếp đất (`202100`, `202115`, `202126`); chỉ lấy dải đã thấy, không cả `20xxxx` |
| Roll | `027xxx` | lăn, lùi |
| Ladder | `028000`-`028999` | thang: vào `028999`, đứng yên `028030`, leo lên `0280 1x` (và `028100`), leo xuống `0280 2x`, trượt `028000`-`028002`, đánh/đá `028040`/`028045`, chuyển tiếp `028036` |
| Critical | `031700`-`031799` | đòn chí mạng (riposte `031700`, backstab `031719` → `031710`), đặt trước Attack |
| Attack | `030000`-`039999` | đòn đánh thường |
| Skill | `040000`-`049999` | AoW (phần không đi qua prefix) |
| Item | `050000`-`059999` | dùng item; xem ghi chú dưới về `050190` |
| Other | còn lại | đứng yên `000000`, `0200xx`, ngồi nghỉ `068xxx`, anim lúc load `063000`... |

Nhóm Torrent (người cưỡi chạy `12xxxx`): đứng `000000`, đi `0021xx`, chạy (giữ
phím chạy, cả lần bấm thêm và giảm tốc) `0022xx`, quẹo/xoay `0051xx`, nhảy `0061xx`
(`006110` đứng, `006130` đi hoặc chạy, cùng id) và tiếp đất `0074xx` → **Walk / Run /
Jump / Other**. Torrent theo SpEffect của **người chơi** khi dùng `[[Override]]`.

## Các quyết định và đã thử/bỏ

- **Key tổng `All`:** giá trị khác 1 áp cho **mọi** hành động của đối tượng và ghi đè mọi
  key riêng; `All = 1` thì dùng key riêng. Torrent không bị key tổng của người chơi ảnh
  hưởng.
- **Nhóm `Movement` đã bỏ** (2026-10-02) và tách thành Walk / Run / Sneak sau yêu cầu trên
  Nexus: bàn phím chỉ có hai kiểu đi (đi bộ và chạy), không có sprint riêng; chữ số thứ 3
  của đuôi là kiểu di chuyển. `0200xx` (`020010`) chỉ thấy khi dùng item lúc di chuyển (chân
  trong hai đoạn đầu của uống bình), không phải "chạy thường" như đoán trước đó. Tên
  ban đầu `Crouch` đổi thành `Sneak`.
- **Item mở rộng dần:** ban đầu chỉ `050xxx`. Throwing dagger và crystal dart dùng đuôi
  `055000` nên rơi vào Other, dải được nới ra `05xxxx` (không anim nào không phải item
  mang đuôi `05xxxx` trong các log đã có: nghỉ grace là `068xxx`, load là `063000`).
- **`050190` (Spectral Steed Whistle) xếp vào Other**, không phải Item: dù là item,
  trong game nó là bước đầu của động tác lên ngựa (`050190 → 101004 nhảy lên → 100000
  đang cưỡi`), nên đi cùng anim cưỡi ngựa `1xxxxx`/`2xxxxx`.
- **Đòn chí mạng (Critical):** anim của nạn nhân không được mod tăng tốc nên người
  chơi nhanh hơn rút kiếm xong trong khi kẻ địch vẫn đang ngã (người dùng xác nhận).
  Bản đầu ép cố định 1.0 (kể cả dưới `All` và `[[Override]]`); cùng ngày thành key riêng,
  mặc định vanilla, hành xử như mọi key khác của người chơi; comment cảnh báo lệch nhịp
  nếu lớn hơn 1. Chưa thử vũ khí khác `a023` (vd. dao găm có đòn chí mạng riêng).
- **Placidusax's Ruin (`a451`, chỉ thần chú này):** phần phóng tia `045110`-`045119` là hiệu
  ứng game tạo ra, chạy theo đồng hồ riêng nên lệch khi anim bị tăng tốc (Nexus
  bloodaxis). Xếp vào Other, đặt trước nhánh Cast; phần niệm đầu `045100` vẫn Cast.
  Tốc độ phần tia theo Other (và `All` nếu khác 1), không có key riêng. Test trong game
  với nhiều giá trị Cast: hết lệch, `045110` luôn ~8-9 giây bất kể Cast. Khi đọc log:
  dòng `a451_045110 ... Other speed=X` ghi `X` là Cast của anim *trước* (log in trước khi
  áp dụng).
- **Thang (Ladder):** tách riêng thay vì để trong Other vì tăng tốc leo/trượt có thể lệch
  nếu game tính quãng đường theo root motion hay timer riêng; người dùng tự chọn. Mặc
  định cho file cấu hình mới là hơi tăng; file có sẵn giữ giá trị đang ghi.

## Giới hạn đã biết

- **Uống bình khi đang chạy thuộc nhóm Walk/Run**, không phải Item. Animation uống khi
  chạy **không vào `anim_queue`** (giữ anim chạy `0201xx`/`0202xx`), nên mod áp tốc độ của
  nhóm di chuyển; người dùng xác nhận bằng bản DLL cũ rằng tăng tốc di chuyển thì uống khi
  chạy cũng nhanh lên. Anim nửa thân trên nằm ở `CSChrTimeActModule +0xD0` nhưng giá trị
  đó **không bị xoá khi anim xong** (giữ tới khi anim chính đổi), nên dùng nó làm điều
  kiện kéo dài tốc độ cao tới lúc ngừng chạy; `+0xC8`/`+0xCC`/`+0xD4` cũng không báo được
  anim còn chạy hay không. Hơn nữa game chỉ có **một** tốc độ animation cho cả nhân vật
  (xem `animation_speed_research.md`), nên không thể tăng riêng lớp nửa thân trên. Người
  dùng chốt: giữ giới hạn này, đã gỡ toàn bộ code thử (dò `+0xD0`, `watch.rs`).
- **Anim đi bộ khi quá tải là `020020`**, nằm ngoài dải Walk nên không được áp lúc đó
  (vào Other).
- **Hình ảnh không đồng bộ giữa hai máy trong co-op** (xem `seamless.md`).
- Nhóm "luôn 1.0" cho anim chết / bị túm chưa làm (chờ đồng ý, dùng chung danh sách anim
  chết với `fasterrevival` qua `common`).

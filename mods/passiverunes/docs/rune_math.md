# PassiveRunes: cách tính rune mỗi chu kỳ và mốc thời gian

**Status 2026-09-23: ĐANG PHÁT HÀNH (2.1.x).** Công thức chi phí lên cấp đã xác
minh trong game (2026-09-23). Code: `mods/passiverunes/src/rune.rs`.

Mỗi chu kỳ (`Rune.Passive.Interval`, mili giây) mod cộng một tổng gồm **ba thành
phần độc lập, cộng dồn với nhau**; đặt thành phần nào bằng `0` là tắt riêng nó.
Kết quả luôn kẹp trong `0..=999.999.999` (trần rune thật của game), không bao giờ
xuống dưới 0. Nếu `Rune.Passive.Enabled=false` thì mod không làm gì (tắt cả chu kỳ lẫn
mốc thời gian).

## 1. Số cố định (`Rune.Passive.Amount`)

Cộng thẳng `Amount` rune mỗi chu kỳ. Đây là bản gốc của mod.

## 2. Phần trăm chi phí lên cấp (`Rune.Passive.Percent`)

Yêu cầu từ bình luận trên Nexus: số cố định nhanh chóng vô nghĩa khi lên cấp cao,
người chơi phải sửa ini liên tục. Mỗi chu kỳ cộng thêm
`floor(level_up_cost(level) * Percent / 100)` rune, **tối thiểu 1 rune** khi
`Percent != 0`.

- **Cộng dồn, không thay thế.** Ban đầu định cho chọn một trong hai (`Percent > 0`
  thì bỏ qua `Amount`), cuối cùng chọn kết hợp: mỗi tick nhận `Amount` cộng phần %.
  Dùng chung một đồng hồ `Interval` nên dễ tính thời gian (vd. `Percent=1`,
  `Interval=10000` = 100 tick, tức khoảng 16,7 phút/cấp chỉ tính phần thụ động).
- **Luôn tính theo cấp hiện tại**, không theo số rune đang giữ: gom rune mà không
  lên cấp thì vẫn nhận đều % của cấp kế tiếp mỗi tick.
- **Level** đọc từ `PlayerGameData.level` (field có sẵn cạnh `rune_count`).
- **Chi phí lên cấp** không có param trong game; đó là công thức cứng trong exe, cộng
  đồng đã giải: `l = level + 81`, `x = max(0, (l - 92) * 0.02)`,
  `cost = floor((x + 0.1) * l^2 + 1)` (`level_up_cost()` trong `rune.rs`). Cấp 1 lên 2
  ra 673, khớp màn hình lên cấp. **Đã xác minh trong game (2026-09-23)**: cấp 257
  lên 258 ra 573.505 rune, khớp đúng con số màn hình lên cấp hiển thị.

## 3. Lãi kép trên rune đang giữ (`Rune.Passive.Interest`)

Ý tưởng của người dùng: chế độ thứ ba, rune theo % số rune đang giữ. Ban đầu đề
xuất dạng `Mode=1/2/3`, bỏ vì đi ngược quyết định "cộng dồn" ở trên; thay bằng một
key thứ ba cũng cộng dồn, mỗi phần tắt độc lập bằng `0`.

- **Tăng theo hàm mũ.** Mô phỏng với `Interval=10000`, chỉ phần lãi: `Interest=1` gấp đôi
  mỗi ~70 tick (~11,6 phút), từ 229 triệu rune chạm trần trong ~25 phút, từ 10.000 rune
  mất ~3,2 giờ; `Interest=0.1` từ 1.000 rune chạm trần ~38 giờ. Vì vậy ini gợi ý
  khoảng `0.05`-`0.2`. Bản đầu có thêm `Rune.Passive.Interest.Max` (trần rune lãi mỗi
  tick), **đã bỏ trong cùng ngày** theo quyết định người dùng: không cần giới hạn, để
  lãi kép chạy tự do và người dùng tự chọn `Interest` nhỏ nếu muốn chậm.
- **Cộng dồn phần lẻ** (`InterestState::carry`): nếu làm tròn xuống mỗi tick thì
  `0.1%` của 1.000-1.999 rune luôn chỉ ra 1 rune (1.000 tick mới gấp đôi thay vì
  ~694), và dưới 1.000 rune thì lãi bằng 0 vĩnh viễn, không phải lãi kép thật. Phần
  lẻ giữ lại sang tick sau, **reset** khi số rune đang giữ thấp hơn sau lần cộng
  trước (`last_held`: đã tiêu hoặc chết mất rune), khi tắt `Interest`, hoặc khi đổi
  dấu `Interest` lúc hot reload.
- **Không có mức tối thiểu** (khác `Percent`, vốn tối thiểu 1 rune): giữ 0 rune thì lãi
  0. Chết mất rune là mất luôn "tiền gốc", rủi ro và phần thưởng tự nhiên. Tính trên rune
  đang giữ, không tính rune rơi dưới đất.
- Log (khi bật) ghi `+ {interest} from {Interest}% interest on {principal}`, trong đó
  principal là rune đang giữ **trước** khi cộng tick này.

## Giá trị âm: trừ rune theo thời gian (2026-09-23)

`Amount=-50` mỗi chu kỳ trừ 50 rune; tương tự `Percent`, `Interest` và bonus trong
`Rune.Milestone`. Trước đó mọi giá trị âm bị kẹp về 0 (tắt).

- Toàn bộ phép tính chuyển sang `i64`; `add_runes(u32)` thành `apply_runes(i64)`, kết
  quả kẹp `0..=999.999.999`.
- `Percent` âm: `-(floor(cost * |p| / 100))`, vẫn tối thiểu 1 rune (lần này là -1) khi
  `p != 0`, đối xứng với chiều dương.
- `Interest` âm là suy giảm kép (mất `|p|%` số rune đang giữ mỗi tick). Phần lẻ làm tròn
  về phía 0 (`trunc`) để đúng cho cả hai chiều.
- Các thành phần vẫn cộng dồn, vd. `Amount=-50` cùng `Percent=0.25` ra trừ 50 nhưng cộng
  0,25% chi phí lên cấp.
- `Interval` vẫn kẹp `>= 0` (interval âm vô nghĩa). Milestone có **số giây âm** bị bỏ qua
  (trước đó được parse và thưởng ngay khi vào game).
- **Khoảng giá trị được kẹp theo đúng khoảng ghi trong ini:** `Amount` trong
  `-999999999..999999999`, `Percent` và `Interest` trong `-100..100`. Trước đó code
  không kẹp `Percent`/`Interest` (đặt 500 vẫn chạy); ini thêm mẫu `Range:`/`Default:`
  cho các key này và code thêm `.clamp(...)` để khoảng ghi khớp hành vi thật.

## Mốc thời gian (`Rune.Milestone`)

Chuỗi `giây:thưởng` ngăn cách bằng dấu phẩy. Mục không parse được hoặc có số giây âm
bị bỏ qua; chuỗi rỗng parse ra danh sách rỗng (tắt mốc thời gian riêng, mà không
cần tắt chu kỳ). Danh sách được sắp tăng dần theo thời gian.

- **Threshold-crossing, không so khớp tuyệt đối.** Bản C++ so `elapsed == atSeconds`;
  nếu tick lệch nhịp (giật frame, hoặc `Interval` không chia hết mốc) thì bỏ lỡ bonus
  vĩnh viễn. Bản Rust so `session_elapsed >= milestone.at`, nên không thể bỏ sót mốc.
- **Chỉ tiến sang mốc kế khi cộng rune thành công.** Trước đây vòng lặp tăng
  `next_milestone` kể cả khi `add_runes` thất bại (vd. chưa vào game), làm mất bonus
  vĩnh viễn; nay thất bại thì dừng và tick sau thử lại đúng mốc đó.
- **Đồng hồ session/interval chỉ chạy khi đã vào game.** Trước đó chúng đếm ngay từ lúc
  task đăng ký, nên thời gian ở màn hình tiêu đề và màn hình tải cũng tính vào mốc
  (log thật cho thấy tick đầu ghi `session 90s` dù chưa vào game). Chưa ở trong world
  thì tick trả về ngay, không cộng thời gian vào đồng hồ nào. **Tạm dừng, không reset**
  khi rời world, vì reset ở màn hình tải sẽ xoá tiến độ mốc mỗi lần dịch chuyển. Hệ
  quả: thoát ra title rồi load save khác vẫn đếm tiếp từ số cũ, chỉ không đếm phần
  thời gian ở ngoài. Việc kiểm tra "đã vào game" dùng
  `common::player::main_player_chr_ins_ptr` (hàm chung sẵn có; bản đầu tự viết `in_game()`
  riêng và đã thay).
- Mốc vẫn là số cố định, chưa hỗ trợ `%`.

## Mặc định đã đổi (2026-09-23)

Theo quyết định người dùng, mặc định mỗi 10 giây đổi thành 50 rune cố định cộng 0,25%
chi phí lên cấp (~400 tick, ~67 phút/cấp chỉ tính phần %). Lưu ý với người dùng cũ:
`config::load_or_create_default` chỉ **thêm** key còn thiếu, không ghi đè key đã có.
Ini cũ giữ nguyên `Amount` cũ, nhưng `Rune.Passive.Percent` là key mới nên được thêm với
giá trị `0.25`, tức bản 2.1 **bật sẵn** phần % cho cả người dùng cũ (cộng dồn với
`Amount` cũ của họ). Chấp nhận, vì đây là tính năng chính của bản đó.

## Log chi tiết

Với `LogFile` bật, dòng interval ghi: `+{amount} runes ({fixed} fixed + {percent} from
{Percent}% of {cost} next-level cost) -> held {held}, level {level}, session {s}s`. Dòng
milestone thêm `-> held, level`. `apply_runes()` trả `Option<Granted { held, level }>` để
lấy hai giá trị này trong cùng một lần truy cập `PlayerGameData`.

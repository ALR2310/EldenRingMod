# Project Rules

## Cập nhật README.md sau khi thêm/sửa tính năng

Mỗi mod trong `crates/<mod>/README.md` được viết theo kiểu "nhật ký phát
triển": các mục có tiêu đề kèm ngày (`(YYYY-MM-DD)`), ghi lại lý do quyết
định, hướng đã thử/bỏ, và trạng thái hiện tại - không chỉ là hướng dẫn dùng.

Sau khi hoàn thành một thay đổi đáng kể ở 1 mod (thêm tính năng, đổi ini
key, sửa bug về hành vi, đổi cấu trúc code/thư mục), **luôn thêm 1 mục mới**
vào `README.md` của mod đó, ghi ngày hôm nay, mô tả: đã đổi gì, vì sao (đặc
biệt nếu sửa 1 hiểu lầm/bug trong thiết kế trước đó), và cập nhật lại mọi
đường dẫn file/tên key cũ được nhắc tới ở phần trên của README nếu chúng đã
lỗi thời.

Không cần hỏi lại người dùng trước khi làm việc này - tự thêm ngay sau khi
code đã ổn định, coi đây là 1 bước không thể thiếu của việc "xong việc",
giống như build/test.

## Đối chiếu changelog thật trên Nexus Mods trước khi viết `[Changelog]` trong DESCRIPTION.bbcode

Trước khi thêm/sửa mục changelog trong `crates/<mod>/DESCRIPTION.bbcode`,
**luôn gọi Nexus Mods API để lấy changelog thật hiện có trên trang mod**
trước, rồi mới viết mục mới nối tiếp đúng theo đó - không tự đặt số phiên
bản/nội dung dựa trên suy đoán hay dựa vào lịch sử trong README (lịch sử
README ghi theo ngày phát triển nội bộ, không phải số phiên bản đã publish
trên Nexus, 2 thứ có thể lệch nhau).

Cách gọi:

```bash
API_KEY=$(cat .secrets/nexus_api_key.txt)
curl -sS -H "apikey: $API_KEY" \
  "https://api.nexusmods.com/v1/games/eldenring/mods/<mod_id>/changelogs.json"
```

Mod ID theo từng mod trong workspace này (game domain luôn là `eldenring`):

- `autoregen` → 10548
- `runemultiplier` → 10630
- `weightmultiplier` → 10549 (Reduction Weight)
- `passiverunes` → 10528
- `risearcher` → 5807
- `sometweaks` → **chưa có, chưa đăng lên Nexus**

Key API cá nhân của người dùng lưu ở `.secrets/nexus_api_key.txt` (đã
gitignore, không commit). Nếu API trả về thiếu 1 vài version cũ so với thực
tế trên trang web (đã xảy ra 1 lần, 2026-09-03) - đó là do giới hạn của
API, không phải version đó không tồn tại - hỏi lại người dùng xác nhận
qua trang web thật trước khi tự xoá bất kỳ mục changelog cũ nào.

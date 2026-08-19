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

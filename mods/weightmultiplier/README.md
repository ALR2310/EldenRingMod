# WeightMultiplier

> Nexus mod 10549 · **đã phát hành** (trước là "Reduction Weight")

Mod DLL cho Elden Ring: **nhân Trọng Tải (equip load) hiện tại với một hệ số**,
hoặc **đặt cứng nó bằng một số**, thay vì ghi đè thành một số tuỳ ý như các mod
"NoWeight" thông thường. Một hook duy nhất, áp cho cả số hiển thị trên UI lẫn
hành vi thật (roll, chạy). Đổi tên từ **ReductionWeight** ngày 2026-08-18 khi
port sang Rust, vì hệ số < 1 giảm, > 1 tăng tải trọng.

## Cấu hình

Cấu hình trong `WeightMultiplier.ini` cạnh DLL (tự tạo nếu thiếu). Có thể chỉnh:

- chế độ: nhân tổng tải với một hệ số, hoặc đặt cứng bằng một số (hệ số cho phép số âm);
- độ trễ sau khi game khởi động trước khi áp dụng mod;
- phím nạp lại cấu hình (chỉ nhận khi cửa sổ game đang focus);
- ghi log để chẩn đoán.

Tên key, giá trị mặc định và ý nghĩa nằm ở chú thích trong [`WeightMultiplier.ini`](WeightMultiplier.ini); file mẫu này được nhúng vào DLL nên là nguồn sự thật duy nhất, README không lặp lại để khỏi lệch.

Đổi chế độ, hệ số hoặc giá trị cố định rồi bấm phím reload là có hiệu lực ngay, không cần khởi động lại game.

## Cách hoạt động

- **Điểm hook:** game cộng trọng lượng 5 slot trong một vòng lặp rồi copy tổng ra
  bằng `movaps xmm0,xmm6`. Mod patch đúng lệnh này (chạy một lần sau vòng lặp),
  nên nhân hay ghi đè ở đó cho một con số sạch, không dồn qua từng slot. Hai
  điểm hook thử trước đó đều sai (không tác dụng, hoặc chỉ đổi UI). Chi tiết:
  [docs/hook_point.md](docs/hook_point.md).
- **Tìm điểm patch bằng AOB** `FF C3 83 FB ?? 7C ?? 4C 8D 5C 24 ??` + 12 byte. Mẫu
  duy nhất trên 2.6.2.0, 2.7.0.0 và 2.7.1.0, **cùng RVA** ở cả ba.
- **Patch:** vì chỉ có 7 byte, dùng JMP tương đối 5 byte (`E9 rel32`) tới stub
  được cấp phát gần đó (`common::codepatch`). Stub nhân hoặc ghi đè `xmm6` rồi chạy
  lại hai lệnh gốc.
- **Hot reload:** đổi giá trị chỉ cần ghi lại một biến mà stub đọc mỗi lần chạy.
  Đổi `Mode` thì mod vá lại 4 byte opcode trong stub (`mulss` ↔ `movss`).
- **Khởi động:** `DllMain` tạo một thread, nạp/tạo ini, mở log, chờ `LoadDelay`,
  ghi phiên bản game và danh sách DLL (`common::diag`), rồi cài hook. Sau đó
  thread thăm dò `ReloadKey` mỗi 100 ms.
- **Mod khác hook đè cùng chỗ:** nếu không tìm thấy neo, mod kiểm tra xem có phải
  một mod trọng lượng khác đã hook không và ghi tên DLL đó vào log.
- **Không phụ thuộc `fromsoftware-rs`** và không dùng bộ lập lịch task của game:
  mod chỉ patch bộ nhớ một lần từ một thread thường. Vì vậy phím reload dùng
  `common::input` (thăm dò phím từ thread riêng) và **không có banner "Config
  reloaded"/`ReloadBanner`** như các mod khác, kết quả reload chỉ hiện trong log.

Code: [src/hook.rs](src/hook.rs) (toàn bộ logic), [src/lib.rs](src/lib.rs) (entry point).

## Giới hạn

- Đây là patch code sống: nếu game đổi bố cục vùng này, mod không tìm thấy neo và
  tự tắt cho phiên đó (log ghi lỗi); chưa kiểm chứng ngoài 2.6.2.0, 2.7.0.0, 2.7.1.0.
- Đổi `Mode` lúc đang chạy ghi đè code có thể đang thực thi; người dùng đã chấp
  nhận rủi ro này. Nếu vá thất bại, mod giữ `Mode` cũ.
- Nếu mod trọng lượng khác đã hook cùng chỗ, mod này không cài được hook (có ghi
  log). Một báo lỗi "anchor not found" trên Nexus (2026-09-26) chưa rõ nguyên
  nhân.
- Tài liệu phát triển cũ không ghi kết quả test in-game của bản Rust; khi sửa
  hook cần kiểm cả số hiển thị trên UI và hành vi thật (roll, chạy).
- Chỉ dùng ở chế độ offline, tắt EAC.

## Tài liệu liên quan

- [CHANGELOG.md](CHANGELOG.md): ghi chú phát hành cho người dùng.
- [HISTORY.md](HISTORY.md): dòng thời gian phát triển.
- [docs/hook_point.md](docs/hook_point.md): lịch sử tìm điểm hook, stub, tương thích.
- [nexus_page.bbcode](nexus_page.bbcode): mô tả trang Nexus.

# WeightMultiplier

> **v2.0.1** · Nexus mod 10549 · **đã phát hành** (trước là "Reduction Weight")

Mod DLL cho Elden Ring: **nhân Trọng Tải (equip load) hiện tại với một hệ số**,
hoặc **đặt cứng nó bằng một số**, thay vì ghi đè thành một số tuỳ ý như các mod
"NoWeight" thông thường. Một hook duy nhất, áp cho cả số hiển thị trên UI lẫn
hành vi thật (roll, chạy). Đổi tên từ **ReductionWeight** ngày 2026-08-18 khi
port sang Rust, vì hệ số < 1 giảm, > 1 tăng tải trọng.

## Cấu hình (`WeightMultiplier.ini`)

File mẫu nhúng trong DLL (`include_str!`), là nguồn sự thật duy nhất cho giá trị
mặc định.

| Section | Key | Mặc định | Ý nghĩa |
|---|---|---|---|
| `[General]` | `ReloadKey` | `F5` | Phím nạp lại ini; chỉ nhận khi cửa sổ game đang focus |
| `[General]` | `LoadDelay` | `5000` | Mili giây chờ sau khi game khởi động trước khi áp dụng mod |
| `[Features]` | `Mode` | `0` | `0` = nhân hệ số (`Multiplier`), `1` = đặt cứng (`FixedValue`) |
| `[Features]` | `Multiplier` | `0.5` | Hệ số nhân tổng tải; chỉ dùng khi `Mode=0`. `0.5` = một nửa, `1.0` = giữ nguyên, `2.0` = gấp đôi. Cho phép số âm |
| `[Features]` | `FixedValue` | `0` | Tải luôn bằng đúng số này, bất kể đang mặc gì; chỉ dùng khi `Mode=1` |
| `[Logging]` | `LogFile` | `true` | Ghi `WeightMultiplier.log` cạnh DLL; `false` thì không tạo file |

Đổi `Multiplier`/`FixedValue`/`Mode` rồi bấm `ReloadKey` là có hiệu lực ngay,
không cần khởi động lại game.

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

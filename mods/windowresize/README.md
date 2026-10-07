# WindowResize

> **v1.0.0** · **không đăng lên Nexus, dùng nội bộ** · đã test trong game

Mod DLL cho Elden Ring: cho phép **kéo viền để đổi kích thước cửa sổ** ở chế
độ Windowed, nhỏ hơn mức 800x450 mà menu độ phân giải của game cho chọn. Làm
cho nhu cầu cá nhân; ban đầu định đăng Nexus nhưng đã bỏ ý định đó (2026-10-07),
nên mod chỉ dùng nội bộ và không có mod ID, không có bản phát hành.

## Cấu hình (`WindowResize.ini`)

File cạnh DLL, tự tạo nếu thiếu; log ở `WindowResize.log`. **Đổi ini cần khởi
động lại game** (mod không có hot reload).

| Section | Key | Mặc định | Ý nghĩa |
|---|---|---|---|
| `[Window]` | `Resizable` | `true` | Bật/tắt viền kéo resize (chỉ ở chế độ Windowed) |
| `[Window]` | `Width` / `Height` | trống | Kích thước vùng hình (pixel thật) đặt khi game khởi động; trống cả 2 = giữ kích thước của game, điền 1 = cái còn lại theo tỉ lệ |
| `[Window]` | `AspectWidth` / `AspectHeight` | `16` / `9` | Khoá tỉ lệ vùng hình khi kéo; `0` ở 1 trong 2 = kéo tự do |
| `[Window]` | `MinWidth` / `MinHeight` | `160` / `90` | Kích thước vùng hình nhỏ nhất (pixel thật) |
| `[Logging]` | `LogFile` | `true` | Ghi `WindowResize.log` cạnh DLL |

## Cách hoạt động

- **Chỉ dùng Win32**, không AOB/RVA, không đụng struct của game, không dùng
  hệ thống task của game: không vỡ khi game cập nhật và chạm vào game ít nhất.
- Game tạo cửa sổ **không có `WS_THICKFRAME`** nên vốn không kéo resize được.
  Mod tìm cửa sổ game (`EnumWindows`), thêm lại `WS_THICKFRAME`, và subclass
  WndProc để: hiện mũi tên kéo ở viền (`WM_SETCURSOR`), áp kích thước nhỏ nhất
  (`WM_GETMINMAXINFO`) và khoá tỉ lệ trên vùng hình (`WM_SIZING`).
- **Watchdog 1 giây** thêm lại viền khi game ghi đè style (đổi screen mode hay
  độ phân giải) và hook lại nếu cửa sổ bị tạo lại. Fullscreen/Borderless thì
  bỏ qua.
- **Kích thước khởi động** (`Width`/`Height`) được áp lại mỗi giây trong 20
  giây đầu, dừng ngay khi người chơi bắt đầu kéo viền.
- Chi tiết, các thử nghiệm ban đầu và lý do từng quyết định:
  [docs/resize_approach.md](docs/resize_approach.md).

Code: [src/window.rs](src/window.rs) (toàn bộ logic), [src/lib.rs](src/lib.rs)
(entry point).

## Giới hạn

- Chỉ hoạt động ở chế độ **Windowed**.
- Game vẫn render ở độ phân giải trong setting rồi co lại, nên FPS không tăng
  khi cửa sổ nhỏ.
- Chưa kiểm chứng: khi game focus nó `ClipCursor` con trỏ vào vùng hình, nên có
  thể phải click ra ngoài cửa sổ trước khi kéo viền (xem
  [docs/resize_approach.md](docs/resize_approach.md)).

## Tài liệu liên quan

- [HISTORY.md](HISTORY.md): dòng thời gian phát triển.
- [docs/resize_approach.md](docs/resize_approach.md): khảo sát và thiết kế.
- [nexus_page.bbcode](nexus_page.bbcode): bản nháp trang Nexus từ lúc còn định
  đăng, **không còn dùng**.

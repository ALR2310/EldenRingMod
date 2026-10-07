# WindowResize: vì sao chỉ cần `WS_THICKFRAME` và subclass WndProc

**Status 2026-09-29: HOÀN THÀNH, đã xác nhận trong game (kéo resize hoạt động
như mong đợi).** Code: `mods/windowresize/src/window.rs`. Không cần dịch
ngược: mod chỉ dùng Win32, không AOB/RVA, không đụng struct của game, nên
không vỡ khi game cập nhật.

## Câu hỏi gốc

Menu độ phân giải của game có mức nhỏ nhất 800x450 (trước đó 800x420). Có cần
RE trong IDA để bỏ giới hạn này không? Trả lời: **không**. Thử từ bên ngoài
bằng PowerShell trước khi mở IDA:

1. `SetWindowPos` ép cửa sổ về 480x300: game **giữ nguyên** kích thước đó, vẫn
   render đủ cảnh (DXGI tự co swapchain vào client area) và không tự kéo về.
   Vậy không có chỗ nào trong game cần patch.
   - Lần thử đầu tưởng thất bại: PowerShell không DPI-aware, màn hình để 150%
     nên "480x300" thành 720x450 pixel thật. Phải `SetProcessDPIAware` trước
     khi đo hay đặt kích thước.
2. Style cửa sổ gốc là `0x14CA0000`, **không có `WS_THICKFRAME`**, nên vốn
   không kéo resize được; 800x450 chỉ là mục nhỏ nhất trong list độ phân giải.
3. Thêm `WS_THICKFRAME` từ ngoài: `WM_NCHITTEST` ở mép trả đúng
   HTLEFT/HTRIGHT/HTBOTTOM/HTBOTTOMRIGHT, nhưng **không hiện mũi tên kéo**
   vì game tự xử lý `WM_SETCURSOR` và ép con trỏ của nó.

Kết luận: (a) thêm `WS_THICKFRAME`, (b) subclass WndProc để trả `WM_SETCURSOR`
ở vùng viền cho `DefWindowProcW`.

## Thiết kế đã chọn

- **Tìm cửa sổ:** `EnumWindows` lấy cửa sổ visible của process hiện tại có
  class bắt đầu bằng `ELDEN RING` (process còn có cửa sổ ẩn `IME`, `DIEmWin`).
  Không dùng `soulsteleport::ui::game_window()` vì hàm đó chỉ trả về khi game
  đang là foreground window.
- **Subclass** bằng `SetWindowLongPtrW(GWLP_WNDPROC)`, không dùng
  `SetWindowSubclass` vì hàm đó từ chối khi gọi từ thread khác thread tạo cửa
  sổ (mod chạy trên worker thread).
- **WndProc riêng**, chỉ can thiệp khi viền do mod thêm đang bật:
  - `WM_SETCURSOR` với hit-test 10..=17: giao cho `DefWindowProcW` (hiện mũi
    tên).
  - `WM_GETMINMAXINFO`: `ptMinTrackSize` = `MinWidth/MinHeight` + khung.
  - `WM_SIZING`: ép min và tỉ lệ `AspectWidth:AspectHeight` trên **client
    area**, chỉ dịch cạnh đang kéo. Kéo cạnh trên/dưới thì chiều cao quyết
    định chiều rộng; mọi cạnh/góc khác thì chiều rộng quyết định chiều cao.
  - Kích thước khung tính bằng `AdjustWindowRectExForDpi` thay vì đọc rect
    thật (rect = 0 khi cửa sổ đang minimize).
- **Watchdog 1s:** game ghi đè style mỗi lần đổi screen mode hay độ phân giải
  nên viền được thêm lại định kỳ; tự hook lại nếu cửa sổ bị tạo lại.
  Fullscreen/Borderless (không có `WS_CAPTION`) thì bỏ qua. `Resizable=false`
  gỡ viền, chỉ gỡ nếu chính mod đã thêm.

## Kích thước khởi động (`Width`/`Height`)

Hai key này là kích thước **client area** (pixel thật). Khi tìm thấy cửa sổ
game, mod `SetWindowPos` về kích thước đó (giữ vị trí). Để trống cả 2 thì
không đụng, giữ kích thước theo setting của game. Điền 1 key thì key còn lại
tính theo `AspectWidth:AspectHeight` (nếu tắt tỉ lệ thì giữ kích thước hiện
tại cho trục đó). Vẫn bị kẹp bởi `MinWidth/MinHeight`.

- **Áp lại mỗi 1s trong 20 giây đầu** (`START_SIZE_WINDOW`) sau khi hook, vì mod
  hook cửa sổ ngay lúc mới tạo, trước khi game kịp đặt độ phân giải trong
  setting lên nó; áp 1 lần có thể bị game ghi đè ngay sau. Ban đầu là 10 giây,
  tăng lên 20 sau khi test.
- **Dừng áp ngay khi người chơi bắt đầu kéo viền** (`WM_ENTERSIZEMOVE` đặt
  `USER_RESIZED`), để không giành lại kích thước họ vừa chọn.
- **DPI:** thread của mod gọi `SetThreadDpiAwarenessContext(PER_MONITOR_AWARE_V2)`
  nên `Width`/`Height` luôn là pixel thật, không bị Windows nhân theo scale
  như lần test PowerShell. Game vốn đã DPI-aware (`GetDpiForWindow` = 144 ở
  150%, bản unaware luôn trả 96), đây chỉ là phòng hờ. Khung cửa sổ khi đặt
  kích thước khởi động đo từ rect thật (`GetWindowRect - GetClientRect`) để
  cùng hệ toạ độ với `SetWindowPos`. Log lúc hook in DPI của cửa sổ để kiểm
  tra.

## Đã bỏ: hot reload và log môi trường

Bỏ `ReloadKey` và `common::reload::run`, vì hot reload là phần duy nhất của mod
đăng ký code vào hệ thống task của game (`CSTaskImp` qua `fromsoftware-rs`) và
gọi hàm game (banner "Config reloaded"); phần resize chỉ dùng Win32. Bỏ đi để
mod chạm vào game ít nhất có thể, nên đổi ini cần khởi động lại game. Bỏ luôn
`common::diag::log_environment_when_game_ready` (chờ `CSTaskImp` rồi log
phiên bản exe và DLL đang nạp): mod không cần log đó, và giờ không còn dùng gì
của game lúc chạy. `window::run` tự poll tìm cửa sổ mỗi 1s nên không cần chờ
game khởi động xong. Ini cũ còn `ReloadKey` được `config::migrate` tự chuyển
vào `[Legacy]`.

## Chưa kiểm chứng / giới hạn

- Khi game đang focus, nó `ClipCursor` con trỏ vào client area (xem
  `mods/soulsteleport/src/ui.rs`, phần "Cursor unpin"): có thể không rê chuột
  tới viền được lúc đang chơi, khi đó phải click ra ngoài cửa sổ trước rồi mới
  kéo viền. Nếu gây phiền thì cân nhắc hook `ClipCursor` để nới rect ra thêm
  độ dày viền.
- Game vẫn render ở độ phân giải trong setting rồi co lại, nên FPS không tăng
  khi cửa sổ nhỏ.

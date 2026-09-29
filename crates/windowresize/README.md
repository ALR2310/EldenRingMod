# WindowResize

Mod làm cho nhu cầu cá nhân, đăng Nexus để ai cần thì dùng (mô tả:
[DESCRIPTION.bbcode](DESCRIPTION.bbcode)): cho phép kéo viền để đổi kích thước
cửa sổ Elden Ring ở chế độ Windowed, nhỏ hơn mức 800x450 nhỏ nhất mà menu
độ phân giải của game cho chọn.

- DLL: `WindowResize.dll`, config: `WindowResize.ini` (cạnh DLL, tự tạo nếu
  thiếu), log: `WindowResize.log`.
- Code: [src/window.rs](src/window.rs) (toàn bộ logic), [src/lib.rs](src/lib.rs)
  (entry point).
- Chỉ dùng Win32 API, không AOB/RVA, không đụng struct của game - không
  vỡ khi game cập nhật.

## Ini

| Key | Mặc định | Ý nghĩa |
|---|---|---|
| `Resizable` | `true` | Bật/tắt viền kéo resize (chỉ ở chế độ Windowed) |
| `Width` / `Height` | trống | Kích thước vùng hình đặt khi game khởi động; trống cả 2 = giữ kích thước của game, điền 1 = cái còn lại theo tỉ lệ |
| `AspectWidth` / `AspectHeight` | `16` / `9` | Khoá tỉ lệ vùng hình khi kéo; `0` ở 1 trong 2 = kéo tự do |
| `MinWidth` / `MinHeight` | `160` / `90` | Kích thước vùng hình nhỏ nhất (pixel thật) |
| `LogFile` | `true` | Ghi log |

## Khảo sát ban đầu (2026-09-29)

Câu hỏi gốc: có cần RE trong IDA để bỏ giới hạn 800x420/800x450 không?
Thử từ bên ngoài bằng PowerShell trước khi mở IDA:

1. `SetWindowPos` ép cửa sổ về 480x300 → game **giữ nguyên** kích thước đó,
   vẫn render đủ cảnh (DXGI tự co swapchain vào client area), không tự kéo
   về lại. → Không có chỗ nào trong game cần patch.
   - Lần thử đầu tưởng thất bại: PowerShell không DPI-aware, màn hình để
     150% nên "480x300" thành 720x450 pixel thật. Phải `SetProcessDPIAware`
     trước khi đo/đặt kích thước.
2. Style cửa sổ gốc là `0x14CA0000` - **không có `WS_THICKFRAME`**, nên vốn
   không kéo resize được; 800x450 chỉ là mục nhỏ nhất trong list độ phân giải.
3. Thêm `WS_THICKFRAME` từ ngoài → `WM_NCHITTEST` ở mép trả đúng
   HTLEFT/HTRIGHT/HTBOTTOM/HTBOTTOMRIGHT, nhưng **không hiện mũi tên kéo**:
   game tự xử lý `WM_SETCURSOR` và ép con trỏ của nó.

Kết luận: chỉ cần (a) thêm `WS_THICKFRAME`, (b) subclass WndProc để trả
`WM_SETCURSOR` ở vùng viền cho `DefWindowProcW`. Không cần IDA.

## Bản đầu tiên (2026-09-29)

- Tìm cửa sổ: `EnumWindows` lấy cửa sổ visible của process hiện tại có
  class bắt đầu bằng `ELDEN RING` (process còn có cửa sổ ẩn `IME`,
  `DIEmWin`). Không dùng lại `soulsteleport::ui::game_window()` vì hàm đó
  chỉ trả về khi game đang là foreground window.
- Subclass bằng `SetWindowLongPtrW(GWLP_WNDPROC)` (không dùng
  `SetWindowSubclass` vì hàm đó từ chối khi gọi từ thread khác thread tạo
  cửa sổ - mod chạy trên worker thread).
- WndProc riêng, chỉ can thiệp khi viền do mod thêm đang bật:
  - `WM_SETCURSOR` với hit-test 10..=17 → `DefWindowProcW` (hiện mũi tên).
  - `WM_GETMINMAXINFO` → `ptMinTrackSize` = `MinWidth/MinHeight` + khung.
  - `WM_SIZING` → ép min + tỉ lệ `AspectWidth:AspectHeight` trên **client
    area**, chỉ dịch cạnh đang kéo. Kéo cạnh trên/dưới thì chiều cao quyết
    định chiều rộng; mọi cạnh/góc khác thì chiều rộng quyết định chiều cao.
  - Kích thước khung tính bằng `AdjustWindowRectExForDpi` thay vì đọc rect
    thật (rect = 0 khi cửa sổ đang minimize).
- Watchdog 1s: game ghi đè style mỗi lần đổi screen mode/độ phân giải nên
  viền được thêm lại định kỳ; tự hook lại nếu cửa sổ bị tạo lại. Fullscreen/
  Borderless (không có `WS_CAPTION`) thì bỏ qua. `Resizable=false` gỡ viền
  (chỉ gỡ nếu chính mod đã thêm).

Chưa kiểm chứng trong game:
- Khi game đang focus, nó `ClipCursor` con trỏ vào client area (xem
  `soulsteleport/src/ui.rs`, phần "Cursor unpin") - có thể không rê chuột
  tới viền được lúc đang chơi; khi đó phải click ra ngoài cửa sổ trước rồi
  mới kéo viền. Nếu gây phiền thì cân nhắc hook `ClipCursor` để nới rect ra
  thêm độ dày viền.
- Game vẫn render ở độ phân giải trong setting rồi co lại → FPS không tăng
  khi cửa sổ nhỏ.

## Đổi ý: đăng lên Nexus (2026-09-29)

Ban đầu định chỉ dùng nội bộ; sau khi test ổn trong game thì quyết định
đăng lên Nexus. Thêm `DESCRIPTION.bbcode`: cố ý viết đơn giản, giọng cá
nhân ("làm cho mình dùng, đăng lên để ai cần thì dùng"), không có mục
Features (chỉ 1 tính năng, dòng mở đầu đã nói đủ), không có mục "Why this
mod?". Crate version lên `1.0.0` khớp với mục changelog đầu tiên. Chưa có
mod ID trên Nexus.

Đã xác nhận trong game: kéo resize hoạt động như mong đợi.

## Bỏ hot reload (2026-09-29)

Bỏ `ReloadKey` và `common::reload::run`. Hot reload là phần duy nhất của mod
đăng ký code vào hệ thống task của game (`CSTaskImp` qua `fromsoftware-rs`)
và gọi hàm game (banner "Config reloaded"); phần resize chỉ dùng Win32.
Bỏ đi để mod chạm vào game ít nhất có thể - đổi ini giờ cần khởi động lại
game. Bỏ luôn `common::diag::log_environment_when_game_ready` (chờ
`CSTaskImp` rồi log exe version + DLL đang nạp): mod này không cần log đó,
và giờ mod không còn dùng gì của game lúc chạy - chỉ Win32 (`fromsoftware-rs`
vẫn được compile qua `common` nhưng LTO bỏ hết phần không gọi tới).
`window::run` tự poll tìm cửa sổ mỗi 1s nên không cần chờ game khởi động xong.
Ini cũ còn `ReloadKey` sẽ được `config::migrate` tự chuyển vào `[Legacy]`.
## Kích thước khởi động `Width`/`Height` (2026-09-29)

Thêm 2 key `Width`/`Height` (kích thước **client area**, pixel thật): khi
tìm thấy cửa sổ game, mod `SetWindowPos` về kích thước này (giữ vị trí).
Để trống cả 2 → không đụng, giữ kích thước theo setting của game. Điền 1
key → key còn lại tính theo `AspectWidth:AspectHeight` (nếu tắt tỉ lệ thì
giữ kích thước hiện tại cho trục đó). Vẫn bị kẹp bởi `MinWidth/MinHeight`.

Áp lại mỗi 1s trong **20s đầu** sau khi hook (`START_SIZE_WINDOW`), vì từ
khi bỏ `diag` mod hook cửa sổ ngay lúc mới tạo, trước khi game kịp tự
đặt độ phân giải trong setting lên nó - áp 1 lần có thể bị game ghi đè
ngay sau. Ngừng áp ngay khi người chơi bắt đầu kéo viền
(`WM_ENTERSIZEMOVE` → `USER_RESIZED`), để không giành lại kích thước họ
vừa chọn. Ban đầu là 10s; tăng lên 20s cùng ngày theo yêu cầu sau khi test.

DPI: thread của mod gọi `SetThreadDpiAwarenessContext(PER_MONITOR_AWARE_V2)`
nên `Width`/`Height` luôn là pixel thật, không bị Windows nhân theo scale
như lần test ngoài bằng PowerShell (480x300 → 720x450 ở 150%). Game vốn đã
DPI-aware (`GetDpiForWindow` = 144 ở 150%, bản unaware luôn trả 96), đây chỉ
là phòng hờ. Khung cửa sổ khi đặt kích thước khởi động đo từ rect thật
(`GetWindowRect - GetClientRect`) cho cùng hệ toạ độ với `SetWindowPos`.
Log lúc hook in DPI của cửa sổ để kiểm tra.
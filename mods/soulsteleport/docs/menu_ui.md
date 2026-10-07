# SoulsTeleport: menu ImGui, input và hiển thị

**Status 2026-09-25: ĐANG PHÁT HÀNH (1.1.0), đã test trong game** (chạy chung với
QuestPath, cả ba chế độ màn hình, danh sách dài có thanh cuộn). Code: `src/ui.rs`,
`src/input_block.rs`, `assets/`. Chưa hỗ trợ điều khiển menu bằng tay cầm.

## Từ addon SoulsChat sang mod độc lập có menu riêng

Ban đầu (2026-09-24) mod là addon cho SoulsChat với lệnh `/teleport <tên>`. Phân tích SoulsChat
v0.1.2 (IDA, bản đã giải nén UPX): không có API plugin (chỉ export `DllMain` và hai hàm hot-patch);
hàm xử lý lệnh `sub_18000CA1B` so tên lệnh bằng `sub_18001B864`. Bản hook đã chạy được: hook
hàm `trim` dùng chung `sub_18009B504` (22 chỗ gọi), chỉ xử lý khi địa chỉ trả về đúng là chỗ gọi
lúc Enter; nếu dòng là `/teleport ...` thì ghi lại tên, trả chuỗi rỗng để SoulsChat tự xoá ô nhập;
patch `mov rax, imm64; jmp rax` 12 byte (8 lệnh `push` đầu của `trim` nên không cắt ngang lệnh nào);
hai mẫu AOB (thân `trim`, chỗ gọi lúc Enter) khớp đúng một chỗ trong bộ nhớ đã giải nén.

**Đã bỏ hướng đó (2026-09-25)** vì muốn có gợi ý lệnh/tên như `/warpto` thì phải can thiệp sâu vào
dữ liệu và code nội bộ của mod người khác. Đổi sang mod độc lập có GUI riêng, tên giữ "Souls
Teleport". Đã xoá `src/soulschat.rs` và hoàn tác hai thay đổi `common` chỉ dùng cho nó. Tên
"SoulsChat: Teleport" từng bị loại vì kiểu "Mod gốc: Tính năng" dễ bị hiểu là module chính thức
(báo lỗi nhầm chỗ, gắn tên tác giả khi chưa xin phép); "Souls Teleport" cùng "họ" nhưng không mượn
nguyên tên.

## Công nghệ GUI

Đã tham khảo hai mod khác: QuestPath (Dear ImGui + MinHook, hook DXGI `Present` /
`ExecuteCommandLists` của D3D12, có đường dự phòng cửa sổ trong suốt) và ZB_BossReviver (không hook
DirectX, cửa sổ Win32 riêng vẽ bằng GDI). Chọn **ImGui qua hudhook** (chỉ feature `dx12`), giống
SoulsChat, chạy mọi chế độ màn hình. Menu vẽ trên luồng render của game (trong `Present` đã hook);
mọi thứ đụng tới phiên, Steam hay warp vẫn ở task `FrameBegin`. Hai bên chỉ chia sẻ một snapshot
danh sách đồng đội (task làm mới mỗi 500 ms khi menu mở), trạng thái và SteamID người được bấm.

## Con trỏ chuột và chặn input của game

Game ẩn và khoá con trỏ rồi đọc chuột qua raw input, nên `WM_MOUSEMOVE` của hudhook không dùng được.
Cách làm hiện tại:

- **Đưa chuột vào ImGui** (`feed_mouse`, trong `before_render` khi menu mở và cửa sổ game đang được
  chọn): tự đọc `GetCursorPos` rồi `ScreenToClient`, quy đổi từ pixel client sang kích thước swapchain
  (`display_size`, xử lý cả trường hợp Windows scale 150% khi game không DPI-aware), nút chuột đọc bằng
  `GetAsyncKeyState` (chỉ gửi khi đổi trạng thái, nhả ra khi đóng menu). `before_wnd_proc` trả `Break`
  cho `WM_INPUT` và mọi `WM_MOUSE*` khi menu mở để các sự kiện tương đối của hudhook (cộng lên giá trị
  "không có vị trí" của ImGui, vị trí không bao giờ hợp lệ) không đè lên vị trí tuyệt đối.
- **Hook `SetCursorPos` và `ClipCursor`** (MinHook có sẵn trong hudhook, theo cách QuestPath làm):
  game liên tục khoá lại con trỏ và kéo về giữa để xoay camera. Khi menu mở, `SetCursorPos` không di
  chuyển (giả vờ thành công) và `ClipCursor` luôn truyền `NULL`.
- **Chặn chuột trong game (`input_block.rs`):** `message_filter` không có tác dụng vì game không đọc
  input qua tin nhắn cửa sổ: import của `eldenring.exe` chỉ có `DINPUT8!DirectInput8Create` và
  `XINPUT1_4` theo ordinal. Mod tạo một `IDirectInput8` và thiết bị bàn phím tạm để đọc vtable, hook
  `GetDeviceState` (slot 9) và `GetDeviceData` (slot 10) (dinput8 dùng chung cài đặt thiết bị nên trúng
  luôn thiết bị của game). Menu mở thì vẫn gọi hàm gốc (để rút cạn bộ đệm, đóng menu không bị "dồn"
  phím) rồi xoá trạng thái chuột (`DIMOUSESTATE`/`2`) và bỏ sự kiện của thiết bị nhận ra là chuột.
  **Chỉ chặn chuột**, vẫn cho di chuyển bằng bàn phím và tay cầm (theo yêu cầu người dùng). Không
  xoá định dạng joystick vì `DIJOYSTATE` toàn 0 là cần gạt bị đẩy hết cỡ. Bản đầu có hook XInput và
  chặn cả bàn phím, đã bỏ.

### Lỗi mất con trỏ ở cửa sổ nhỏ: `ScaleAllSizes` làm tròn cỡ con trỏ

Ở cửa sổ 1680x945 trở xuống không thấy con trỏ dù log cho thấy vị trí đúng hoàn toàn (người dùng rê
chuột mù vẫn bấm trúng), nên lỗi ở phần **vẽ**. Nguyên nhân: `ImGuiStyle::ScaleAllSizes` có
`MouseCursorScale = ImFloor(MouseCursorScale * scale_factor)`. Scale theo độ phân giải: 1080 px cho 1.0
thành 1; 1440 px cho 1.48 thành 1; 945 px cho 0.875 thành **0**, con trỏ vẽ với cỡ 0. Sửa: sau
`scale_all_sizes`, đặt lại `mouse_cursor_scale = gốc × scale` không làm tròn. (Gợi ý ban đầu rằng 1680x945 là
cửa sổ lớn nhất vừa màn hình 2560x1440 ở scale 150% không phải nguyên nhân thật.)

## Hiển thị

- **Tự co giãn theo độ phân giải:** chuẩn là cửa sổ game 1080p; mọi thứ nhân theo chiều cao cửa sổ
  chia 1080 (kẹp trong 0.6-3.0), đọc từ `io.display_size` mỗi frame nên đổi độ phân giải lúc chơi vẫn
  theo kịp. Font nạp một lần ở 40 px rồi hiển thị ở `22 px × scale` qua `font_global_scale`; style
  `scale_all_sizes(scale)` từ một bản style gốc lưu lúc khởi tạo (không áp chồng lên bản đã scale).
  Khi scale đổi, lần mở kế tiếp đặt lại kích thước/vị trí cửa sổ một lần.
- **`MenuScale`:** hệ số người dùng, nhân thêm trên scale tự động; đọc một lần lúc khởi động (đổi
  xong phải khởi động lại game). Chữ, style và **kích thước** menu theo scale tổng; **vị trí** chỉ theo
  độ phân giải (tách `pos_scale`) để đổi `MenuScale` không làm menu trôi khỏi chỗ đã đặt. Font raster
  `40 × MenuScale` kẹp 40-64 px (không cao hơn vì atlas còn chứa chữ CJK).
- **Theme riêng** (`apply_theme`, áp một lần **trước** khi lưu style gốc nên co giãn vẫn đúng): màu
  theo tông menu của game (nền đen ấm hơi trong suốt, chữ kem, điểm nhấn vàng đồng), bo góc, padding
  rộng, tiêu đề căn giữa. Theme mặc định của ImGui (nền đen, viền xanh dương) bị người dùng chê quá thô.
- **Bố cục:** bỏ thanh tiêu đề mặc định; đầu menu là tên mod căn giữa màu vàng, nút `X` đóng ở góc phải
  và nút **Reset** nhỏ ở góc trái; mỗi người một dòng kèm vai trò; dòng trạng thái đổi màu đỏ nhạt khi
  lỗi.
- **Dòng trạng thái tự xoá** sau 15 giây (`STATUS_TTL`), trừ khi đang có request chờ. Trước đó có hai
  lỗi: "Teleporting to X..." không bao giờ bị xoá sau warp thành công, và đóng/mở lại menu xoá dòng
  trạng thái trong khi nút vẫn khoá. Đã bỏ toast banner hệ thống vì trạng thái đã hiện trong menu.
- **Lưu vị trí và kích thước menu** vào ini: bốn key trong mục `[Menu]`, giá trị -1 là mặc định. Lưu
  theo đơn vị 1080p (chia cho UI scale). Mỗi frame đọc vị trí/kích thước thật; khi khác bản đã lưu từ 1
  đơn vị trở lên **và** người chơi đã nhả chuột trái thì ghi ini một lần (không ghi mỗi frame trong
  lúc kéo). Mở menu lần đầu không ghi gì. Hàm mới trong `common`: `config::set_values(ini_path,
  &[(key, value)])` sửa đúng dòng `key=value` tại chỗ (giữ comment, section, dòng khác; key chưa có thì nối
  cuối file; cập nhật luôn map trong bộ nhớ); trước đó cả workspace không có hàm nào ghi giá trị vào
  ini. Nút Reset ghi -1 cho cả bốn key và đặt lại bố cục ngay frame sau, xoá mốc so sánh để bố cục mặc
  định vừa áp không bị ghi ngược thành số.

## Font nhúng

`FONT_CANDIDATES` cũ ghi cứng `C:\Windows\Fonts\segoeui.ttf`/`arial`/`tahoma`: hỏng nếu Windows cài
ổ khác, máy thiếu font, hay chạy **Proton/Steam Deck** (font Windows thường không có trong Wine). Không
crash (rơi về font mặc định ImGui) nhưng font đó chỉ có ASCII nên tên có dấu thành `?`. Giờ:

- **Font chính nhúng trong DLL:** `assets/NotoSans-Subset.ttf` (Noto Sans Regular, cắt bằng
  `pyftsubset` còn dải U+0020-024F, U+0300-036F, U+1E00-1EFF, U+2000-206F, U+20AC: 1007 glyph, 91 KB).
  Giấy phép SIL OFL 1.1 (`assets/NotoSans-OFL.txt`, không có Reserved Font Name nên giữ nguyên tên
  khi cắt). Khi đăng cần ghi công font và kèm file giấy phép: `scripts/build-mod.ps1` khi `-Zip` gói
  thêm mọi `*.txt` trong `mods/<crate>/assets/`.
- **CJK tuỳ chọn:** tìm thư mục Windows thật bằng `GetWindowsDirectoryW`, lấy font đầu tiên có trong
  `msyh`/`msjh`/`meiryo`/`msgothic`/`simsun`, ghép vào với dải `chinese_simplified_common`. Không có
  thì bỏ qua. **Chưa hỗ trợ tiếng Hàn:** dải Hangul của ImGui là toàn bộ 11k âm tiết, quá lớn cho
  atlas ở cỡ raster 40 px.
- Không còn `Box::leak`: `FontAtlas::add_font` tự chép dữ liệu font vào atlas.

## Phím mở menu

Bản thử nghiệm có nhiều phím (lưu/warp một chỗ đã lưu, log danh sách người, `ReloadKey`...), đã bỏ
hết (2026-09-25); cấu hình chỉ còn phím mở menu cùng vài thiết lập menu và log. Phím mặc định đổi từ
`F10` sang **Tab**; `parse_virtual_key` không hiểu tên "TAB" nên ini ghi mã hex và có link bảng mã
phím. **Lưu ý:** mặc định của SoulsChat cũng gán Tab cho `MatchmakingUIKey`, nên ai dùng cả hai mod
với mặc định sẽ bị trùng phím.

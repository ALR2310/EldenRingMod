# DropMultiplier

> **v1.1.0** · Nexus mod 11075 · **đã phát hành**, test trong game (1.17 vanilla, 1.16.2 + Convergence)

Mod DLL cho Elden Ring chỉnh hai thứ: **tỉ lệ rơi đồ của quái** (`ItemLotParam_enemy`)
và **số lượng nguyên liệu** nhận được mỗi lần nhặt (`ItemLotParam_enemy` +
`ItemLotParam_map`). Cấu hình trong `DropMultiplier.ini`, bấm `ReloadKey` để
nạp lại mà không cần khởi động lại game.

## Cấu hình (`DropMultiplier.ini`)

File mẫu nhúng trong DLL (`include_str!`), là nguồn sự thật duy nhất cho giá
trị mặc định. Key có sẵn trong file người dùng không bị ghi đè khi nâng cấp.

| Section | Key | Mặc định | Ý nghĩa |
|---|---|---|---|
| `[General]` | `ReloadKey` | `F5` | Phím nạp lại ini (chỉ hoạt động khi cửa sổ game đang focus) |
| `[General]` | `ReloadBanner` | `true` | Hiện banner "Config reloaded" sau khi reload |
| `[Drop]` | `Mode` | `0` | `0` = nhân tỉ lệ rơi của từng món; `1` = ép tỉ lệ rơi về giá trị cố định |
| `[Drop]` | `Multiplier` | `3` | Hệ số, chỉ dùng khi `Mode=0` (1 = vanilla) |
| `[Drop]` | `Percentage` | `30` | Tỉ lệ cố định %, chỉ dùng khi `Mode=1` (100 = chắc chắn rơi) |
| `[Materials]` | `Crafting` | `2` | Hệ số số lượng nguyên liệu craft farm được (Rowa Fruit, Herba, bướm, xương...) |
| `[Materials]` | `Upgrade` | `2` | Hệ số nguyên liệu nâng cấp farm được (Smithing Stone, Glovewort...) |
| `[Materials]` | `Unique` | `1` | Hệ số nguyên liệu chỉ nhặt được 1 lần (Smithing Stone trên xác, Sacred Tear, Scadutree Fragment...) |
| `[Logging]` | `LogFile` | `true` | Ghi `DropMultiplier.log` cạnh DLL; `false` thì không tạo file |

Key cũ `[Settings] ChancePercent` được tự chuyển thành `[Drop] Percentage`
khi nạp (giữ giá trị người dùng đã chỉnh).

## Cách hoạt động

- **Khởi động:** `DllMain` tạo 1 thread, nạp/tạo ini, mở log, chạy watcher
  `ReloadKey` (`common::reload`), rồi chờ game sẵn sàng (`CSTaskImp`,
  `SoloParamRepository`, `WorldChrMan.main_player`) **không giới hạn thời
  gian**; chỉ nhắc trong log mỗi 30 giây. Nhờ đó mở game rồi vào world muộn vẫn
  tự áp dụng.
- **Tỉ lệ rơi (`src/drop_rate.rs`):** mỗi dòng `ItemLotParam_enemy` có tối đa 8
  ô, ô N rơi với xác suất `lotItemBasePoint0N / tổng 8 ô`. Chỉ ô chứa đồ thật
  (`lotItemId0N != 0`) được nhân; ô "không rơi gì" giữ nguyên (`Mode=0`) hoặc
  làm điểm cố định để giải ra trọng số mới (`Mode=1`: tổng xác suất "có rơi gì
  đó" đúng bằng `Percentage`, tỉ lệ tương đối giữa các món cùng dòng giữ
  nguyên). Công thức đầy đủ và các trường hợp biên nằm ở comment đầu file.
- **Nguyên liệu (`src/materials.rs`):** nhân `lotItemNum0N` (làm tròn, tối thiểu
  1, tối đa 255), phân nhóm từng ô bằng `EquipParamGoods.goodsType` đọc lúc
  chạy và cờ nhặt. Chi tiết phân loại: [research/dropmultiplier_materials.md](../../research/dropmultiplier_materials.md).
- **Hot reload không cộng dồn:** trọng số/số lượng gốc được chụp một lần trước
  mọi chỉnh sửa; mỗi lần áp hoặc reload tính lại từ bản chụp đó.
- **Duyệt param an toàn:** dùng `common::params::for_each_row_mut` (duyệt theo
  index), không dùng `rows_mut()` của fromsoftware-rs vì panic với regulation
  bị lệch header: [research/dropmultiplier_convergence_panic.md](../../research/dropmultiplier_convergence_panic.md).
- **Log:** ghi phiên bản game và danh sách DLL đã nạp (`common::diag`, không in
  đường dẫn đầy đủ, ẩn DLL của game/Steam/bản crack).

## Giới hạn

- Chỉ chỉnh **tỉ lệ cơ bản**; chỉ số Discovery của người chơi vẫn cộng thêm như
  vanilla.
- `Mode=1` có thể biến một món vốn chắc chắn rơi (100%) thành rơi theo xác suất.
- Chỉ ảnh hưởng `ItemLotParam_enemy` (tỉ lệ rơi, và số lượng) và
  `ItemLotParam_map` (số lượng nguyên liệu); các bảng lot khác giữ nguyên.
- Chỉ dùng ở chế độ offline, tắt EAC.

## Tài liệu liên quan

- [HISTORY.md](HISTORY.md): dòng thời gian phát triển.
- [research/dropmultiplier_materials.md](../../research/dropmultiplier_materials.md): thiết kế và phân loại nguyên liệu.
- [research/dropmultiplier_convergence_panic.md](../../research/dropmultiplier_convergence_panic.md): panic với Convergence và cách sửa.
- [DESCRIPTION.bbcode](DESCRIPTION.bbcode): trang Nexus (kèm changelog đã phát hành).

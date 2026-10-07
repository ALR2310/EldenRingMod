# DropMultiplier: nhân số lượng nguyên liệu (`[Materials]`)

**Status 2026-09-28: PHÁT HÀNH trong 1.1.0, người dùng xác nhận chạy ổn trong
game.** Code: `mods/dropmultiplier/src/materials.rs`.

## Vì sao có tính năng này

Đề xuất từ người báo lỗi Convergence trên Nexus (sau khi 1.0.2 sửa xong): cho
multiplier ảnh hưởng cả nguyên liệu craft và đồ hái ngoài thế giới mở. Trên
Nexus không có mod DLL nào làm việc này; các mod tương tự (#2238 "Reasonable
Material Drop rate and Amount Increase", #537 "Better Gathering and Hunting")
đều thay nguyên `regulation.bin` và sửa tay từng món, nên xung đột với overhaul
và không hỗ trợ DLC.

## Thiết kế ini

```ini
[Materials]
Crafting=1   ; nguyên liệu craft farm được
Upgrade=1    ; nguyên liệu nâng cấp farm được
Unique=1     ; mọi nguyên liệu chỉ nhặt được 1 lần
```

Chốt cùng người dùng sau vài vòng. Chọn 3 key vì tách được nhóm ảnh hưởng cân
bằng nhiều nhất (Scadutree Fragment, Sacred Tear, Smithing Stone trên xác - đều
nằm ở `Unique`) khỏi nhóm farm vô hại.

Đã thử và bỏ:
- chia 2 key theo nguồn `Farmable`/`Unique` (không tách craft và nâng cấp);
- 1 key duy nhất cho mọi nguyên liệu;
- tên `Repeatable`/`OneTime`: `Multiplier` trong tên key dễ nhầm với tỉ lệ rơi;
  "Unique" dùng được vì section `[Materials]` đã giới hạn ngữ cảnh (không bị
  hiểu là vũ khí độc nhất).

## Phân loại từng ô

Dựa trên export vanilla 1.17 của `ItemLotParam_map` và `EquipParamGoods` (CSV
trích bằng Smithbox, để trong `tmp/`, tạo lại được). Mỗi ô của 1 dòng lot rơi
vào đúng một nhóm, hoặc không nhóm nào:

1. **Không đụng**: ô không phải Goods (`lotItemCategory0N != 1`), hoặc Goods có
   `EquipParamGoods.goodsType` khác 2 (nguyên liệu craft, 106 món) và 14 (nâng
   cấp: Smithing/Somber Stone, Glovewort, Golden Seed, Sacred Tear, Scadutree
   Fragment, Revered Spirit Ash). Nhờ vậy vũ khí, key item, đồ tiêu hao (Flask,
   Kukri, Golden Rune: đều `goodsType 0`) và các dòng lạ như `ItemLotParam_map`
   ID `2` (quay ngẫu nhiên bình Crimson Tears, không cờ, 60% trắng tay) bị loại.
   Bài học: "không có cờ" không đồng nghĩa "điểm hái"; phải lọc theo
   `goodsType` trước.
2. **`Unique`**: có cờ nhặt (`getItemFlagId` của dòng hoặc `getItemFlagId0N`
   của ô khác 0), tức chỉ nhận được một lần.
3. **`Crafting` / `Upgrade`**: còn lại, hồi lại sau khi nghỉ ở grace, tách theo
   `goodsType`.

Điểm hái là các dòng `ItemLotParam_map` không cờ, ID `9965xx`-`9993xx` (base
game) và `463xxxx` (DLC), vd. `997200` Rowa Fruit ra 1/2/3/5. Smithing Stone
farm được đến từ quái hồi sinh (thợ mỏ, golem, lính, trong `ItemLotParam_enemy`);
không có "mạch quặng" hồi lại. Mô phỏng trên CSV vanilla: 1602 ô crafting, 1046
ô upgrade, 1659 ô unique.

`goodsType` đọc từ `EquipParamGoods` lúc chạy (không hardcode ID) để overhaul
như Convergence tự phân loại đúng đồ của nó. Tra theo ID đi qua
`common::params::row_ids::<P>()` chứ không qua `repo.get()`, vì `repo.get()`
binary search trên chính lookup table gây panic (xem
`dropmultiplier_convergence_panic.md`).

## Công thức và thứ tự áp

- Nhân `lotItemNum0N` (`u8`): làm tròn, tối thiểu 1, tối đa 255; ô số lượng 0
  giữ nguyên.
- Snapshot số lượng gốc và nhóm của từng ô một lần, mỗi lần áp/reload tính lại
  từ snapshot (cùng pattern baseline với `drop_rate`, để reload không cộng dồn).
- Áp cho cả `ItemLotParam_map` (điểm hái, đồ đặt trong thế giới) lẫn
  `ItemLotParam_enemy` (rơi từ quái/thú), sau `drop_rate::apply`, cả lúc khởi
  động lẫn khi bấm ReloadKey. Hai tính năng ghi hai cột khác nhau
  (`lotItemBasePoint` so với `lotItemNum`) nên độc lập.

## Đổi tên ini đi kèm (cùng ngày)

Người dùng đổi template: `[Settings]` thành `[Drop]`, `ChancePercent` thành
`Percentage`. `common::config` bỏ qua section nên chỉ đổi tên key mới có ảnh
hưởng: trước đây migrate sẽ đẩy `ChancePercent` cũ vào `[Legacy]` và đặt
`Percentage` về mặc định, làm mất giá trị đã chỉnh. Thêm
`common::config::load_or_create_default_with_renames` (có unit test): key cũ có
mà key mới chưa có thì chuyển giá trị sang key mới và bỏ key cũ. `lib.rs` truyền
`("ChancePercent", "Percentage")`.

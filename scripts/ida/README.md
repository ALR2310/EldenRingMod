# Công cụ IDAPython cho `eldenring.exe`

Bộ script dùng lại được để hỏi database IDA có sẵn (`dumps/ida/<version>/eldenring.exe.i64`)
những câu lặp đi lặp lại khi viết mod: hàm này làm gì, ai gọi nó, AOB này có
duy nhất ở mọi bản exe không, field `+0x17C8` bị ai đọc ghi. Mỗi lần chạy mất
~15 giây (mở lại database, không phân tích lại).

## Chạy

```powershell
pwsh scripts/ida/run.ps1 <script> [-Version <ver|all>] <tham số...>
```

- `-Version`: thư mục trong `dumps/ida/` (vd. `2.7.1.0` = patch 1.17.1,
  `2.7.0.0` = 1.17, `2.6.2.0` = 1.16.2); `all` chạy lần lượt mọi version;
  bỏ trống = version mới nhất.
- Output ghi vào `dumps/ida/<version>/out/` (cùng file log `<script>.log`),
  không vào git. Địa chỉ trong output đều là **RVA**.
- Địa chỉ nhập vào: RVA (`1e92100`), VA (`0x141e92100`) hoặc tên symbol. Địa
  chỉ nằm giữa hàm cũng được (vd. địa chỉ crash `exe+0x1E92100`).
- Cần đóng IDA GUI trước (database không mở được 2 chỗ). idat mặc định ở
  `D:\Programs\IDA Professional 9.3\idat.exe`; đổi bằng biến môi trường
  `ER_IDAT`.
- Script chỉ đọc, không đổi tên hay type trong database. Mỗi lần thoát idat vẫn
  ghi lại file `.i64` (~1,4 GB, kích thước thay đổi không đáng kể).

## Công cụ

| Script | Dùng để | Ví dụ |
|---|---|---|
| `decompile.py` | Decompile hàm, kèm callee ngắn / string tham chiếu | `run.ps1 decompile 1e92100 --callees --strings` |
| `disasm.py` | Disasm kèm byte thô (dựng AOB, chọn độ dài patch) | `run.ps1 disasm 25e0e0:30` hoặc `... --func` |
| `xrefs.py` | Nơi gọi và tham chiếu dữ liệu, nhiều tầng | `run.ps1 xrefs 25e0e0 --depth=2 --decompile` |
| `strings.py` | Từ string tìm ra hàm (ASCII lẫn UTF-16) | `run.ps1 strings "Warping to" --decompile` |
| `aob.py` | Một mẫu AOB có duy nhất không, ở những bản exe nào | `run.ps1 aob -Version all "48 8B 05 ?? ?? ?? ?? 48 85 C0"` |
| `vtable.py` | Slot của vtable theo tên class RTTI hoặc địa chỉ | `run.ps1 vtable CSChrBehaviorModule --slots=40` |
| `callpath.py` | Có đường gọi trực tiếp từ A tới B không | `run.ps1 callpath <đích> <gốc>... --depth=4` |
| `fieldaccess.py` | Mọi lệnh truy cập `[reg+offset]` (~1 phút) | `run.ps1 fieldaccess 17c8 --float` |
| `switches.py` | Switch lớn trong cây gọi (bộ phân loại theo ID) | `run.ps1 switches <gốc> --depth=3 --min-cases=15` |
| `export_all.py` | Xuất cả exe ra text để grep (rất nặng, vài giờ) | `run.ps1 export_all --asm` |

Tham số có khoảng trắng (mẫu AOB, chuỗi tìm) đặt trong nháy kép. Tham số mỗi
công cụ ghi ở docstring đầu file.

`aob.py` in dòng `RESULT UNIQUE|MULTI(n)|NONE ...` cho từng mẫu, nên
`-Version all` cho ra bảng so sánh ngay trên console. Đây là bước kiểm tra
trước khi đưa mẫu vào `common::memscan::find_pattern_in_module`: mẫu phải
`UNIQUE` ở mọi bản exe mod hỗ trợ. Ví dụ, 1 mẫu thử trong hàm `0x25E0E0`
(2.7.x) khớp đúng 1 chỗ ở cả 3 bản exe nhưng ở RVA `25E121` trên 2.6.2.0 so với
`25E101` trên 2.7.0.0/2.7.1.0 - RVA lệch giữa các bản nên mod chỉ dùng AOB,
không hard-code RVA.

## Viết thêm script

Thêm file `.py` vào thư mục này với khung:

```python
import os, sys, idc
sys.path.insert(0, os.path.dirname(os.path.abspath(idc.ARGV[0])))
import common

common.init()                       # auto_wait + Hex-Rays + thư mục output
pos, flags = common.parse_args()    # tham số vị trí + --cờ / --khóa=giá-trị
...                                 # common.decompile / callees / callers / open_out
common.finish()
```

Lưu ý: giữ nguyên `import common` (không đổi thành `scripts.ida.common`). Chỉ thư mục
chứa script nằm trong `sys.path` của idat, và công cụ di chuyển file của IDE có thể
tự viết lại import khi chuyển thư mục.

Quy ước: script nhận tham số dòng lệnh (không sửa hằng số trong code), ghi
output vào `common.OUT_DIR`, không đổi database, và có docstring ghi cách dùng.
Probe một lần chỉ phục vụ 1 câu hỏi thì cứ viết ở scratchpad - nhưng nếu thấy
mình viết lại lần thứ 2, hãy đưa nó vào đây dưới dạng có tham số.

## Nguồn gốc

Các công cụ này được rút ra từ ~110 script probe dùng một lần của các phiên
nghiên cứu 2026-09-16 đến 2026-10-06 (warp/GameMan, spirit summon, FasterRevival,
phantom/animation, crash dump). Probe gốc vốn chỉ nằm trong scratchpad tạm của
từng phiên nên không được lưu; bản khôi phục từ transcript nằm ở
`tmp/ida-scripts-recovered/` (không vào git, xoá được bất kỳ lúc nào), kèm
`index.csv` ghi phiên và ngày. Giai đoạn trước dùng Ghidra: 6 script tổng quát còn giữ ở `scripts/ghidra/`
chỉ để tham khảo, các script Ghidra một lần đã xoá hẳn.

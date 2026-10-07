"""Helpers dùng chung cho các script IDAPython trong research/ida/.

Các script chạy headless qua `run.ps1` (idat -A -S"<script> <args>"). Mỗi script
`import common` rồi gọi `common.init()` đầu tiên, `common.finish()` cuối cùng.

- Tham số dòng lệnh: `common.ARGS` (đã bỏ tên script), tách cờ bằng `parse_args`.
- Output: `common.OUT_DIR` (run.ps1 đặt biến môi trường ER_OUT; mặc định là
  thư mục `out/` cạnh file .i64). Script chỉ đọc database, không đổi tên/type.
- Địa chỉ nhập dạng RVA (`1e92100`, `0x1e92100`), VA (`0x141e92100`) hoặc tên
  symbol; xem `parse_ea`.
"""
import os
import sys

import ida_auto
import ida_funcs
import ida_hexrays
import idaapi
import idautils
import idc

BADADDR = idaapi.BADADDR
BASE = idaapi.get_imagebase()
ARGS = list(idc.ARGV[1:])
OUT_DIR = os.environ.get("ER_OUT") or os.path.join(
    os.path.dirname(idc.get_idb_path()), "out"
)

_HEXRAYS = False


def init():
    """Chờ auto-analysis xong, bật Hex-Rays, tạo thư mục output."""
    global _HEXRAYS
    ida_auto.auto_wait()
    _HEXRAYS = bool(ida_hexrays.init_hexrays_plugin())
    os.makedirs(OUT_DIR, exist_ok=True)


def finish(code=0):
    idc.qexit(code)


def die(msg):
    print("ERROR: " + msg)
    idc.qexit(1)


def parse_args(args=None):
    """Tách `--cờ` / `--khóa=giá-trị` khỏi tham số vị trí.

    Trả về (positional, flags); `--x` -> flags['x'] = True, `--x=3` -> '3'.
    """
    pos, flags = [], {}
    for a in args if args is not None else ARGS:
        if a.startswith("--"):
            k, _, v = a[2:].partition("=")
            flags[k] = v if _ else True
        else:
            pos.append(a)
    return pos, flags


def parse_ea(tok):
    """RVA / VA / tên symbol -> địa chỉ ảo. Giá trị < image base coi là RVA."""
    t = tok.strip()
    if t.lower().startswith("0x"):
        t = t[2:]
    try:
        v = int(t, 16)
        if not t.lower().startswith(("sub_", "loc_")):
            return v + BASE if v < BASE else v
    except ValueError:
        pass
    ea = idc.get_name_ea_simple(tok)
    if ea == BADADDR:
        die("không hiểu địa chỉ/tên: %r" % tok)
    return ea


def rva(ea):
    return ea - BASE


def fn_name(ea):
    name = idc.get_func_name(ea)
    return name or "sub_%X" % ea


def func_start(ea):
    f = ida_funcs.get_func(ea)
    return f.start_ea if f else None


def decompile(ea):
    """Pseudocode (str) hoặc dòng chú thích lỗi; không bao giờ ném exception."""
    if not _HEXRAYS:
        return "// Hex-Rays không khả dụng"
    try:
        return str(ida_hexrays.decompile(ea))
    except Exception as e:  # DecompilationFailure, hoặc thiếu func
        return "// decompile thất bại @%X: %s" % (ea, e)


def callees(func_ea):
    """Tập địa chỉ đầu hàm được gọi trực tiếp từ hàm tại func_ea."""
    f = ida_funcs.get_func(func_ea)
    res = set()
    if not f:
        return res
    for h in idautils.FuncItems(f.start_ea):
        for x in idautils.CodeRefsFrom(h, False):
            g = ida_funcs.get_func(x)
            if g and g.start_ea == x and x != f.start_ea:
                res.add(x)
    return res


def callers(ea):
    """Danh sách (nơi gọi, đầu hàm chứa hoặc None) tới ea qua code xref."""
    out = []
    for x in idautils.CodeRefsTo(ea, False):
        out.append((x, func_start(x)))
    return out


def out_path(name):
    return os.path.join(OUT_DIR, name)


def open_out(name):
    return open(out_path(name), "w", encoding="utf-8", newline="\n")

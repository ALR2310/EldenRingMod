"""Tìm nơi gọi / tham chiếu dữ liệu tới 1 hay nhiều địa chỉ, nhiều tầng.

Dùng:
    run.ps1 xrefs <ea>... [--depth=N] [--decompile]

    --depth=N     đi ngược N tầng caller (mặc định 1)
    --decompile   ghi decompile của từng hàm caller vào xrefs/sub_<RVA>.c

Ghi xrefs.txt: mỗi dòng `d<tầng> <đích> <- <nơi gọi> (hàm ...)` cho code ref,
và `<-data` cho tham chiếu dữ liệu (vtable, bảng con trỏ). Địa chỉ in dạng RVA.
"""
import os
import sys

import idc

sys.path.insert(0, os.path.dirname(os.path.abspath(idc.ARGV[0])))


import idautils

import common


def main():
    pos, flags = common.parse_args()
    if not pos:
        common.die("thiếu địa chỉ; xem docstring")
    depth = int(flags.get("depth", 1))
    want_c = "decompile" in flags
    sub = os.path.join(common.OUT_DIR, "xrefs")
    if want_c:
        os.makedirs(sub, exist_ok=True)
    out = common.open_out("xrefs.txt")
    seen = set()
    level = [common.parse_ea(t) for t in pos]
    for d in range(depth):
        nxt = []
        for tgt in level:
            for frm, fs in common.callers(tgt):
                out.write("d%d %X <- %X (func %s %s)\n" % (
                    d, common.rva(tgt), common.rva(frm),
                    "%X" % common.rva(fs) if fs else "-", common.fn_name(fs) if fs else ""))
                if fs and fs not in seen:
                    seen.add(fs)
                    nxt.append(fs)
                    if want_c:
                        with open(os.path.join(sub, "sub_%X.c" % common.rva(fs)), "w", encoding="utf-8") as fh:
                            fh.write(common.decompile(fs))
            for x in idautils.DataRefsTo(tgt):
                out.write("d%d %X <-data %X (%s)\n" % (d, common.rva(tgt), common.rva(x), idc.get_name(x) or ""))
        level = nxt
    out.close()
    print("%d caller functions -> %s" % (len(seen), common.out_path("xrefs.txt")))


common.init()
main()
common.finish()

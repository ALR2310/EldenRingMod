"""Tìm string trong exe và hàm tham chiếu tới nó (điểm xuất phát tốt nhất khi
chưa biết hàm nằm đâu).

Dùng:
    run.ps1 strings <chuỗi con>... [--decompile] [--regex] [--case]

    --regex      coi mỗi tham số là regex thay vì chuỗi con
    --case       phân biệt hoa thường (mặc định: không phân biệt)
    --decompile  ghi decompile của hàm tham chiếu vào strings/sub_<RVA>.c

Quét cả string ASCII lẫn UTF-16. Ghi strings.txt: `RVA "text" <- hàm...`.
"""
import os
import sys

import idc

sys.path.insert(0, os.path.dirname(os.path.abspath(idc.ARGV[0])))

import re

import ida_nalt
import idautils

import common


def main():
    pos, flags = common.parse_args()
    if not pos:
        common.die("thiếu chuỗi cần tìm; xem docstring")
    pats = [re.compile(p if "regex" in flags else re.escape(p), 0 if "case" in flags else re.I) for p in pos]
    sub = os.path.join(common.OUT_DIR, "strings")
    if "decompile" in flags:
        os.makedirs(sub, exist_ok=True)

    sl = idautils.Strings(default_setup=False)
    sl.setup(strtypes=[ida_nalt.STRTYPE_C, ida_nalt.STRTYPE_C_16], minlen=3, only_7bit=False)
    sl.refresh()

    out = common.open_out("strings.txt")
    funcs = {}
    hits = 0
    for s in sl:
        text = str(s)
        if not any(p.search(text) for p in pats):
            continue
        hits += 1
        refs = []
        for x in idautils.DataRefsTo(s.ea):
            fs = common.func_start(x)
            refs.append("%X(%s)" % (common.rva(x), common.fn_name(fs) if fs else "-"))
            if fs:
                funcs[fs] = text
        out.write("%X %r <- %s\n" % (common.rva(s.ea), text[:140], " ".join(refs[:8]) or "(không có xref)"))
    if "decompile" in flags:
        for fs, text in funcs.items():
            with open(os.path.join(sub, "sub_%X.c" % common.rva(fs)), "w", encoding="utf-8") as fh:
                fh.write("// string: %r\n%s\n" % (text[:100], common.decompile(fs)))
    out.close()
    print("%d string khớp, %d hàm -> %s" % (hits, len(funcs), common.out_path("strings.txt")))


common.init()
main()
common.finish()

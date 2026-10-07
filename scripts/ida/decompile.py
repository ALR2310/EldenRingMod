"""Decompile 1 hay nhiều hàm theo RVA/VA/tên, ghi ra decompile.txt.

Dùng:
    run.ps1 decompile <ea>... [--callees] [--strings] [--max-lines=N]

    --callees     decompile thêm các hàm được gọi trực tiếp (chỉ hàm ngắn,
                  dưới --max-lines dòng, mặc định 80)
    --strings     liệt kê string literal mà hàm tham chiếu
    --max-lines=N cắt pseudocode dài hơn N dòng (mặc định: không cắt)
Mỗi <ea> có thể là địa chỉ bất kỳ trong hàm, ví dụ nơi crash từ crash dump.
"""
import os
import sys

import idc

sys.path.insert(0, os.path.dirname(os.path.abspath(idc.ARGV[0])))

import idautils

import common


def strings_in(func_ea):
    seen = []
    for h in idautils.FuncItems(func_ea):
        for x in idautils.DataRefsFrom(h):
            for kind, tag in ((idc.STRTYPE_C, "str"), (idc.STRTYPE_C_16, "wstr")):
                s = idc.get_strlit_contents(x, -1, kind)
                if s and len(s) > 3:
                    txt = s.decode("utf-8", "replace") if isinstance(s, bytes) else s
                    seen.append("  %s@%X: %s" % (tag, common.rva(h), txt[:100]))
                    break
    return seen


def main():
    pos, flags = common.parse_args()
    if not pos:
        common.die("thiếu địa chỉ; xem docstring")
    limit = int(flags["max-lines"]) if "max-lines" in flags else None
    callee_limit = int(flags["max-lines"]) if "max-lines" in flags else 80
    done = set()
    out = common.open_out("decompile.txt")

    def dump(ea, title):
        fs = common.func_start(ea)
        if fs is None:
            out.write("// ===== %s %X: không nằm trong hàm nào =====\n\n" % (title, ea))
            return None
        if fs in done:
            return fs
        done.add(fs)
        text = common.decompile(fs)
        if limit:
            text = "\n".join(text.splitlines()[:limit])
        out.write("// ===== %s %s  RVA %#x  VA %#x =====\n" % (title, common.fn_name(fs), common.rva(fs), fs))
        if "strings" in flags:
            out.write("// strings referenced:\n" + "\n".join(strings_in(fs) or ["  (none)"]) + "\n")
        out.write(text + "\n\n")
        return fs

    for tok in pos:
        ea = common.parse_ea(tok)
        fs = dump(ea, tok)
        if fs is not None and "callees" in flags:
            for c in sorted(common.callees(fs)):
                if common.decompile(c).count("\n") < callee_limit:
                    dump(c, "callee of %X" % common.rva(fs))
    out.close()
    print("decompiled %d functions -> %s" % (len(done), common.out_path("decompile.txt")))


common.init()
main()
common.finish()

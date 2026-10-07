"""Tìm các `switch` lớn (bảng nhảy) trong cây gọi của 1 tập hàm gốc - thường là
bộ phân loại theo ID (TAE event, ESD command, loại item...).

Dùng:
    run.ps1 switches <gốc>... [--depth=N] [--min-cases=N]

    --depth=N       độ sâu theo callee (mặc định 3)
    --min-cases=N   chỉ báo switch có ít nhất N case (mặc định 15)

Ghi switches.txt: `<gốc> d<tầng> func RVA switch@RVA cases=N low=L size=S`.
"""
import os
import sys

import idc

sys.path.insert(0, os.path.dirname(os.path.abspath(idc.ARGV[0])))

import ida_funcs
import ida_nalt
import idautils

import common


def switches(f):
    for h in idautils.FuncItems(f.start_ea):
        si = ida_nalt.get_switch_info(h)
        if si:
            yield h, si.get_jtable_size(), si.lowcase


def main():
    pos, flags = common.parse_args()
    if not pos:
        common.die("thiếu hàm gốc; xem docstring")
    depth = int(flags.get("depth", 3))
    min_cases = int(flags.get("min-cases", 15))
    out = common.open_out("switches.txt")
    for tok in pos:
        root = common.func_start(common.parse_ea(tok)) or common.parse_ea(tok)
        seen = set()
        stack = [(root, 0)]
        while stack:
            a, d = stack.pop()
            if a in seen:
                continue
            seen.add(a)
            f = ida_funcs.get_func(a)
            if not f:
                continue
            for h, n, low in switches(f):
                if n >= min_cases:
                    out.write("%s d%d func %X switch@%X cases=%d low=%d size=%X\n" % (
                        tok, d, common.rva(f.start_ea), common.rva(h), n, low, f.size()))
            if d < depth:
                for c in common.callees(f.start_ea):
                    stack.append((c, d + 1))
        out.write("%s: đã duyệt %d hàm\n" % (tok, len(seen)))
    out.close()
    print("-> " + common.out_path("switches.txt"))


common.init()
main()
common.finish()

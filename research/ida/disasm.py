"""Disassemble quanh 1 địa chỉ, kèm byte thô (để dựng AOB / chọn độ dài patch).

Dùng:
    run.ps1 disasm <ea>[:count] ...        (count mặc định 40 lệnh)
    run.ps1 disasm <ea> --func              (cả hàm chứa <ea>, bỏ qua count)

Mỗi dòng: `RVA: lệnh | byte hex`. Ghi ra disasm.txt.
"""
import os
import sys

import idc

sys.path.insert(0, os.path.dirname(os.path.abspath(idc.ARGV[0])))

import ida_bytes
import ida_funcs
import idautils

import common


def line(ea):
    size = idc.get_item_size(ea)
    raw = ida_bytes.get_bytes(ea, size) or b""
    return "%X: %-58s | %s" % (common.rva(ea), idc.GetDisasm(ea), raw.hex(" "))


def main():
    pos, flags = common.parse_args()
    if not pos:
        common.die("thiếu địa chỉ; xem docstring")
    out = common.open_out("disasm.txt")
    for tok in pos:
        spec, _, cnt = tok.partition(":")
        ea = common.parse_ea(spec)
        out.write("---- %s (%s) ----\n" % (tok, common.fn_name(ea)))
        if "func" in flags:
            f = ida_funcs.get_func(ea)
            if not f:
                out.write("(không nằm trong hàm)\n")
                continue
            for h in idautils.Heads(f.start_ea, f.end_ea):
                out.write(line(h) + "\n")
        else:
            cur = ea
            for _ in range(int(cnt) if cnt else 40):
                out.write(line(cur) + "\n")
                cur = idc.next_head(cur)
    out.close()
    print("-> " + common.out_path("disasm.txt"))


common.init()
main()
common.finish()

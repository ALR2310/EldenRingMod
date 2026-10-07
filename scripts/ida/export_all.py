"""Xuất toàn bộ exe ra file text để grep: pseudocode (hoặc asm nếu không có
Hex-Rays) + danh sách string. NẶNG: decompile hàng trăm nghìn hàm, vài giờ và
nhiều GB - chỉ dùng khi thật sự cần tìm kiếm văn bản trên cả binary.

Dùng:
    run.ps1 export_all [--asm]

    --asm   bỏ Hex-Rays, xuất disassembly (nhanh hơn nhiều)

Ghi export.c (hoặc export.asm) và export.strings.txt vào thư mục output.
Chạy cho DLL nhỏ (mod khác, ersc) thì dùng idat trực tiếp với script này.
"""
import os
import sys

import idc

sys.path.insert(0, os.path.dirname(os.path.abspath(idc.ARGV[0])))

import ida_funcs
import ida_hexrays
import idautils

import common


def main():
    _, flags = common.parse_args()
    have = (not flags.get("asm")) and bool(ida_hexrays.init_hexrays_plugin())
    ext = ".c" if have else ".asm"
    out = open(common.out_path("export" + ext), "w", encoding="utf-8", errors="replace")
    n = ok = 0
    for ea in idautils.Functions():
        f = ida_funcs.get_func(ea)
        if not f:
            continue
        n += 1
        name = idc.get_func_name(ea)
        if have:
            try:
                out.write("// ===== %s @ %#x =====\n%s\n\n" % (name, ea, ida_hexrays.decompile(ea)))
                ok += 1
                continue
            except Exception as e:
                out.write("// ===== %s @ %#x : decompile thất bại %s =====\n" % (name, ea, e))
        for h in idautils.Heads(f.start_ea, f.end_ea):
            out.write("%x  %s\n" % (h, idc.generate_disasm_line(h, 0)))
        out.write("\n")
    out.close()
    with open(common.out_path("export.strings.txt"), "w", encoding="utf-8", errors="replace") as s:
        for st in idautils.Strings():
            s.write("%x  %s\n" % (st.ea, str(st)))
    print("FUNCS %d DECOMPILED %d" % (n, ok))


common.init()
main()
common.finish()

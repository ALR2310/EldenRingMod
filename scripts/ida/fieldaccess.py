"""Tìm mọi lệnh truy cập [reg+offset] với offset cho trước - tức mọi hàm đọc/ghi
1 field của struct (tốc độ, HP, cờ...) khi biết offset từ crash dump hay Cheat
Engine.

Dùng:
    run.ps1 fieldaccess <offset hex>... [--float] [--limit=N]

    --float      chỉ lệnh SSE float (movss/mulss/comiss/...), bớt nhiễu khi
                 offset nhỏ và phổ biến
    --limit=N    in tối đa N lệnh (mặc định 500)

Quét toàn bộ hàm nên lâu (vài phút). Ghi fieldaccess.txt, mỗi dòng:
`+offset  func=RVA  ea=RVA  tên hàm | lệnh`, sắp theo offset rồi địa chỉ.
"""
import os
import sys

import idc

sys.path.insert(0, os.path.dirname(os.path.abspath(idc.ARGV[0])))

import ida_funcs
import ida_ua
import idautils

import common

FLOAT_MN = {"movss", "mulss", "divss", "addss", "subss", "comiss", "ucomiss", "maxss", "minss",
            "cvtss2sd", "cvtsi2ss", "movd", "movaps", "movups", "shufps"}


def main():
    pos, flags = common.parse_args()
    if not pos:
        common.die("thiếu offset; xem docstring")
    targets = {int(p, 16) for p in pos}
    limit = int(flags.get("limit", 500))
    only_float = "float" in flags
    insn = ida_ua.insn_t()
    hits = []
    for fea in idautils.Functions():
        for ea in idautils.FuncItems(fea):
            if ida_ua.decode_insn(insn, ea) == 0:
                continue
            for op in insn.ops:
                if op.type == ida_ua.o_void:
                    break
                if op.type == ida_ua.o_displ and op.addr in targets:
                    if only_float and insn.get_canon_mnem() not in FLOAT_MN:
                        break
                    hits.append((op.addr, fea, ea, idc.GetDisasm(ea)))
                    break
    hits.sort()
    out = common.open_out("fieldaccess.txt")
    for off, fs, ea, dis in hits[:limit]:
        out.write("+%X  func=%X  ea=%X  %s | %s\n" % (off, common.rva(fs), common.rva(ea), common.fn_name(fs), dis))
    if len(hits) > limit:
        out.write("... còn %d lệnh nữa (tăng --limit)\n" % (len(hits) - limit))
    out.close()
    print("%d lệnh khớp -> %s" % (len(hits), common.out_path("fieldaccess.txt")))


common.init()
main()
common.finish()

"""Liệt kê vtable của 1 class (theo tên RTTI) hoặc tại 1 địa chỉ, kèm slot.

Dùng:
    run.ps1 vtable <tên class con | địa chỉ>... [--slots=N] [--decompile[=i,j,..]]
                                              [--rtti-scan]

    --slots=N       số slot tối đa mỗi vtable (mặc định 40; dừng ở slot không
                    trỏ vào hàm)
    --decompile     decompile mọi slot (hoặc chỉ các chỉ số i,j,..) vào
                    vtable/<vtable>_slot<i>.c
    --rtti-scan     bỏ qua tên symbol, quét RTTI type descriptor trong
                    .data/.rdata (dùng khi database chưa đặt tên `??_7...`)

Tham số bắt đầu bằng 0x hoặc toàn chữ số hex (>= 6 ký tự) coi là địa chỉ,
còn lại là chuỗi con của tên class (vd. `CSChrBehaviorModule`).
Ghi vtable.txt: `slot i  RVA  tên hàm`.
"""
import os
import sys

import idc

sys.path.insert(0, os.path.dirname(os.path.abspath(idc.ARGV[0])))

import re

import ida_bytes
import ida_funcs
import ida_segment
import idautils

import common


def is_addr(tok):
    t = tok.lower()
    return t.startswith("0x") or (len(t) >= 6 and re.fullmatch(r"[0-9a-f]+", t) is not None)


def slots_of(vt, limit):
    res = []
    for i in range(limit):
        p = ida_bytes.get_qword(vt + i * 8)
        if not ida_funcs.get_func(p):
            break
        res.append(p)
    return res


def vtables_by_name(sub):
    for ea, name in idautils.Names():
        if name.startswith("??_7") and sub.lower() in name.lower():
            yield ea, idc.demangle_name(name, idc.get_inf_attr(idc.INF_SHORT_DN)) or name


def vtables_by_rtti(sub):
    """Quét '.?AV<...sub...>' -> COMPLETE_OBJECT_LOCATOR -> vtable (như RTTI của MSVC x64)."""
    rx = re.compile(rb"\.\?A[VU][A-Za-z0-9_@?$]*" + re.escape(sub.encode()) + rb"[A-Za-z0-9_@?$]*", re.I)
    segs = [(s, idc.get_segm_end(s)) for s in idautils.Segments() if idc.get_segm_name(s) in (".data", ".rdata")]
    rdata = {s: ida_bytes.get_bytes(s, e - s) or b"" for s, e in segs if idc.get_segm_name(s) == ".rdata"}

    def find(value, size):
        needle = value.to_bytes(size, "little")
        for s, buf in rdata.items():
            i = buf.find(needle)
            while i != -1:
                yield s + i
                i = buf.find(needle, i + 1)

    for s, e in segs:
        buf = ida_bytes.get_bytes(s, e - s) or b""
        for m in rx.finditer(buf):
            td = s + m.start() - 0x10
            for x in find(td - common.BASE, 4):
                col = x - 12
                if ida_bytes.get_dword(col) != 1:
                    continue
                for ref in find(col, 8):
                    yield ref + 8, m.group(0).decode()


def main():
    pos, flags = common.parse_args()
    if not pos:
        common.die("thiếu tên class/địa chỉ; xem docstring")
    limit = int(flags.get("slots", 40))
    want = flags.get("decompile")
    only = None if want in (None, True) else {int(x) for x in str(want).split(",")}
    sub = os.path.join(common.OUT_DIR, "vtable")
    if want:
        os.makedirs(sub, exist_ok=True)
    out = common.open_out("vtable.txt")
    n = 0
    for tok in pos:
        if is_addr(tok):
            found = [(common.parse_ea(tok), tok)]
        else:
            found = [] if "rtti-scan" in flags else list(vtables_by_name(tok))
            if not found:
                found = list(vtables_by_rtti(tok))
        if not found:
            out.write("== %s: không thấy vtable\n" % tok)
        for vt, name in found:
            n += 1
            out.write("== %s  vtable RVA %#x\n" % (name, common.rva(vt)))
            for i, f in enumerate(slots_of(vt, limit)):
                out.write("slot %-3d %X  %s\n" % (i, common.rva(f), common.fn_name(f)))
                if want and (only is None or i in only):
                    safe = re.sub(r"[^A-Za-z0-9_]", "_", name)[:40]
                    with open(os.path.join(sub, "%s_slot%d.c" % (safe, i)), "w", encoding="utf-8") as fh:
                        fh.write(common.decompile(f))
    out.close()
    print("%d vtable -> %s" % (n, common.out_path("vtable.txt")))


common.init()
main()
common.finish()

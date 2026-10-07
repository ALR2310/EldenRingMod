"""Đếm số vị trí khớp 1 hay nhiều mẫu AOB trong exe (kiểm tra tính duy nhất).

Dùng:
    run.ps1 aob "48 8B 05 ?? ?? ?? ?? 48 85 C0" ["mẫu 2" ...] [--text] [--all=N]
    run.ps1 aob -Version all "mẫu..."      # so sánh giữa mọi bản exe

    --text     chỉ tìm trong section chạy code (.text); mặc định tìm cả image
    --all=N    liệt kê tối đa N vị trí khớp (mặc định 8)

`?` và `??` là wildcard 1 byte. Mỗi mẫu in 1 dòng `RESULT` (run.ps1 gom lại
khi chạy -Version all): `RESULT UNIQUE ...` nếu đúng 1 vị trí, `RESULT MULTI(n)`
nếu nhiều hơn, `RESULT NONE` nếu không có. Vị trí in dạng RVA kèm tên hàm.
Mẫu lấy từ `common::memscan::find_pattern_in_module` của mod phải UNIQUE ở mọi
bản exe mod hỗ trợ.
"""
import os
import sys

import idc

sys.path.insert(0, os.path.dirname(os.path.abspath(idc.ARGV[0])))

import ida_bytes
import ida_ida
import idautils

import common


def ranges(text_only):
    if not text_only:
        return [(ida_ida.inf_get_min_ea(), ida_ida.inf_get_max_ea())]
    return [(s, idc.get_segm_end(s)) for s in idautils.Segments() if idc.get_segm_name(s) == ".text"]


def find_all(pattern, text_only, limit):
    patt = ida_bytes.compiled_binpat_vec_t()
    ok = ida_bytes.parse_binpat_str(patt, common.BASE, pattern.replace("??", "?"), 16)
    # IDA 9.x: True / chuỗi rỗng khi thành công, False / chuỗi lỗi khi thất bại.
    if ok is False or (isinstance(ok, str) and ok):
        raise ValueError("mẫu không hợp lệ: %r (%s)" % (pattern, ok))
    flags = ida_bytes.BIN_SEARCH_FORWARD | ida_bytes.BIN_SEARCH_NOBREAK | ida_bytes.BIN_SEARCH_NOSHOW
    hits = []
    total = 0
    for lo, hi in ranges(text_only):
        ea = lo
        while ea < hi:
            r = ida_bytes.bin_search(ea, hi, patt, flags)
            ea = r[0] if isinstance(r, tuple) else r
            if ea == common.BADADDR or ea >= hi:
                break
            total += 1
            if len(hits) < limit:
                hits.append(ea)
            ea += 1
    return total, hits


def main():
    pos, flags = common.parse_args()
    if not pos:
        common.die("thiếu mẫu AOB; xem docstring")
    limit = int(flags.get("all", 8))
    out = common.open_out("aob.txt")
    for pat in pos:
        try:
            total, hits = find_all(pat, "text" in flags, limit)
        except ValueError as e:
            out.write("RESULT ERROR %s\n" % e)
            continue
        verdict = "UNIQUE" if total == 1 else ("NONE" if total == 0 else "MULTI(%d)" % total)
        where = ", ".join("%X %s" % (common.rva(h), common.fn_name(h)) for h in hits)
        out.write("RESULT %s  [%s]  %s\n" % (verdict, pat, where))
    out.close()
    print(open(common.out_path("aob.txt"), encoding="utf-8").read())


common.init()
main()
common.finish()

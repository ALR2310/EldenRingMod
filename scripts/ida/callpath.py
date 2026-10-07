"""Tìm đường gọi (qua call trực tiếp) từ 1 tập hàm gốc tới 1 hàm đích.

Dùng:
    run.ps1 callpath <đích> <gốc>... [--depth=N] [--decompile]

    --depth=N     độ sâu BFS tối đa (mặc định 4)
    --decompile   decompile thêm các hàm ngay trước đích trên đường tìm được

Trả lời "hàm X có gọi tới Y không, qua đâu" khi xref ngược quá rộng. Gọi gián
tiếp qua vtable không được theo - dùng vtable.py/xrefs.py cho phần đó.
Ghi callpath.txt: `path: gốc <- ... <- hàm gọi đích -> đích` (RVA).
"""
import os
import sys

import idc

sys.path.insert(0, os.path.dirname(os.path.abspath(idc.ARGV[0])))

from collections import deque

import common


def main():
    pos, flags = common.parse_args()
    if len(pos) < 2:
        common.die("cần ít nhất <đích> và 1 <gốc>; xem docstring")
    depth = int(flags.get("depth", 4))
    target = common.func_start(common.parse_ea(pos[0])) or common.parse_ea(pos[0])
    roots = [common.func_start(common.parse_ea(t)) or common.parse_ea(t) for t in pos[1:]]
    parent = {r: None for r in roots}
    queue = deque((r, 0) for r in roots)
    found = []
    while queue:
        f, d = queue.popleft()
        if d >= depth:
            continue
        for c in common.callees(f):
            if c == target:
                found.append(f)
            if c not in parent:
                parent[c] = f
                queue.append((c, d + 1))
    out = common.open_out("callpath.txt")
    if not found:
        out.write("không có đường gọi trực tiếp trong %d tầng\n" % depth)
    for f in found:
        chain, x = [], f
        while x is not None:
            chain.append("%X" % common.rva(x))
            x = parent[x]
        out.write("path: %s -> %X\n" % (" <- ".join(chain), common.rva(target)))
        if "decompile" in flags:
            out.write("\n===== %X =====\n%s\n\n" % (common.rva(f), common.decompile(f)))
    out.close()
    print("%d đường -> %s" % (len(found), common.out_path("callpath.txt")))


common.init()
main()
common.finish()

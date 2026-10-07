# What Seamless Co-op patches in the game - ELDEN RING 2.7.1.0 + Seamless 2.0.1

Reference for making mods compatible with Seamless Co-op (`ersc.dll`, SHA256
`FCD11A18...`, IDA db in `dumps/ersc/fcd11a18/`). `ersc.dll` itself is
protected (VM/packer section, imports resolved at run time), so instead of
reading it, the running game was compared with its exe on disk.

## Files

| File | What |
|---|---|
| `diff_offline.txt` | Every byte range where the running `eldenring.exe` (code sections + `.rdata`, after relocations) differs from the file on disk - **without** Seamless. |
| `diff_seamless.txt` | Same, **with** Seamless 2.0.1 loaded. |
| `seamless_only.json` | Ranges only in a Seamless run = Seamless's patches (363 of them), `[rva, {len, sec, mem, disk}]`. Built from an earlier pair of runs (offline 293 / Seamless 654 ranges). |
| `seamless_only_ida.txt` | Each of those mapped in IDA (`dumps/eldenring/2.7.1.0/eldenring.exe.i64`): containing function, the instruction, and for a redirected `call`/`jmp` the original target. `NEAR` = close to the phantom-colour code that was being chased. |

The two `diff_*.txt` here are a later pair of runs (offline 441 / Seamless 794
ranges) than the one `seamless_only.json` came from - both offline runs carry
the same noise, so either pair works.

What's in both runs (not Seamless): Arxan's run-time code changes and the
patches of the mods that were loaded (SpiritMultiplier, CameraFix,
WindowResize). Subtract the offline run to get Seamless only - match by RVA
with a little slack (`abs(rva - offline_rva) < 16` was used).

## Found with it

- **Phantom colour** (SpiritMultiplier `GhostColor`, 2026-10-01): Seamless
  hooks the start of `sub_1403F0C20` (PhantomParam of a ChrIns) - spirits
  lose their ghost tint. Also hooks `sub_1403F1C90` (get team). Not fixed,
  see TODO.md.
- **Animation speed** (SpeedMultiplier, 2026-10-03): `0x140417EA1` - the
  rel32 of `sub_140417EA0: jmp <animation_speed getter>`, the game's only
  read of `CSChrBehaviorModule.animation_speed`. Fixed in
  `crates/speedmultiplier/src/seamless.rs`.

## Kinds of patches seen

- `E9 xx xx xx xx` over a function start - inline hook.
- 4 changed bytes after an `E8`/`E9` - a `call`/`jmp` redirected (rel32).
- `90 90 ...` - instructions NOPed.
- `EB` over a `7x` - a conditional jump made unconditional.

Seamless finds its sites by byte signatures and aborts the game ("No such
pattern") if one doesn't match - a mod that patches near them must only
rewrite a `call`/`jmp`'s rel32 (see `common::codepatch::redirect_rel32`).

## How it was made

`GhostProbe` in SpiritMultiplier (debug-only, never released): once in the
world it reads `eldenring.exe` from disk, lays its sections out like memory,
applies the relocations, and compares every executable section and `.rdata`
with memory, merging differences closer than 8 bytes. The code is in
`git stash` "seamless ghost probe" (`crates/spiritmultiplier/src/ghost_probe.rs`,
`code_diff()`). Redo it for a new game or Seamless version: the RVAs here
only hold for 2.7.1.0 + Seamless 2.0.1.

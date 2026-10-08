# SpiritThink.ini: chỉnh AI spirit khi đang chơi (thử nghiệm)

**Status 2026-10-08: đã viết, đang test in-game. Chưa có trong ini mặc định và chưa nói trong CHANGELOG (công cụ để nghiên cứu độ hung hăng của spirit).**

## Cách dùng

File `SpiritThink.ini` cạnh DLL, chỉ đọc nếu tồn tại. Mỗi dòng `Tên=giá trị`; tên như trong ngoặc của Smithbox (`TeamAttackEffectivity`, `isGuard_Act`) hoặc tên Rust (`team_attack_effectivity`), không phân biệt hoa thường và dấu `_`. Lưu file lúc đang chơi: tối đa 1 giây sau mod áp lại (không cần F5), rồi triệu hồi lại spirit. Bỏ dòng hoặc xoá file thì về giá trị gốc của game.

## Cách hoạt động

- Hàng đích: mọi `npcThinkParamId` / `npcThinkParamId_ridden` của `BuddyParam`, chọn theo index (không qua bảng tra runtime).
- Lần đầu chụp nguyên 228 byte của mỗi hàng, mỗi lần áp bắt đầu từ bản chụp rồi ghi các field trong file (cùng mẫu baseline với `ghost_color.rs`).
- Bảng field (`src/think_fields.rs`) sinh từ `NPC_THINK_PARAM_ST` của fromsoftware-rs (85 field số, không có cờ bit); test `offsets_match_struct` đối chiếu offset với setter của thư viện.

## Dữ liệu để thử (từ Age of Spirit, regulation.bin của họ so với gốc, dòng Lone Wolf)

| Field | Age of Spirit | Gốc |
|---|---|---|
| TeamAttackEffectivity | 100 | 34 (spirit đầu bảng: 50) |
| BattleStartDist | 15 | 2 |
| eye_dist / searchEye_dist | 15 | 2 / 0 |
| nose_dist | 5 | 0 |
| searchTarget_LV1/2_forgetTime | 5.0 | 1 |

Tooltip Smithbox của TeamAttackEffectivity nói "tăng thì ít người tấn công cùng lúc", ngược với mô tả của tác giả Age of Spirit; chưa biết bên nào đúng, cần test.

# InfiniteAilment: tìm đúng các dòng `SpEffectParam` của Poison và Scarlet Rot

**Status 2026-09-22: ĐÃ XÁC NHẬN trong game** ("phù hợp, không chỉnh sửa gì
thêm"). Code: `mods/infiniteailment/src/status_effect.rs`. Bài học chính: lọc
trực tiếp trên `SpEffectParam` **đang chạy trong game**, không suy đoán từ
CSV export tĩnh (suy đoán từ CSV đã sai ít nhất 3 lần ở mod này).

## Bài toán

Yêu cầu ban đầu của người dùng: "tăng thời gian Poison/Scarlet Rot từ 90s lên
vô hạn". Cần biết field nào trong `SpEffectParam` quyết định thời lượng của
hiệu ứng damage-over-time (DoT) do **người chơi** gây ra, và sửa đúng những
dòng đó, không đụng dòng của quái, hồ độc hay boss.

## Hướng đã thử và bỏ: sửa 1 dòng chung `505`

Phân tích CSV `SpEffectParam` (export bằng SmithBox, bản lọc "Scarlet Rot" và
"Poison") cho ra kết luận sau, **sai ở chỗ then chốt**:

- Con số 90s người dùng thấy ở các dòng `106xxx`/`107xxx` ("Poison/Scarlet Rot
  - Low/Medium/High - Addition/Innate +0..+25") không phải thời lượng DoT mà
  là thời lượng của 1 buff riêng (tăng lượng build-up mỗi hit, vd. từ
  talisman).
- Effect DoT thật nằm ở 1 dòng chung ID `505` ("Poison/Scarlet Rot (Cycled)",
  `EffectEndurance=900`); cả họ `106xxx` lẫn `107xxx` có
  `cycleOccurrenceSpEffectId=505`.
- `-1` là quy ước "vĩnh viễn" của `EffectEndurance` (dòng `90010`, "NPC:
  Disable Scarlet Rot", ship sẵn `-1`).
- `effectTarget*` chỉ là cờ "loại thực thể được phép nhận effect", không cố
  định phe: áp lên bất kỳ ai có thanh build-up đầy.

Kế hoạch lúc đó: `set_effect_endurance(-1.0)` cho đúng dòng `505`, một lần lúc
khởi động. **Chưa từng test trong game.**

Bỏ hướng này (2026-09-22) vì khi lần theo chuỗi để tìm cột damage
(`behaviorId=2105` → `BehaviorParam_PC` (refType=Bullet, refId=1005) →
`Bullet` 1005 → `atkId_Bullet=2` → `AtkParam` ID `2` "Blood Loss - Bullet"),
dòng `AtkParam` cuối chuỗi **dùng chung với hiệu ứng Bleed**. Điều đó làm lung
lay toàn bộ suy luận trước (chỉ dựa trên đọc tĩnh CSV), nên người dùng quyết
định không ghi đè field này nữa. `status_effect.rs` tạm chỉ đọc và log
`EffectEndurance`, `MotionInterval`, `BehaviorId` của dòng `505` để đối chiếu
trước/sau khi tự gây ailment trong game.

## Hướng đúng: lọc trực tiếp trên dữ liệu đang chạy

Duyệt `SpEffectParam` thật trong game, thêm từng điều kiện field, mỗi lần in
số dòng khớp và toàn bộ field liên quan để đối chiếu bằng mắt với SmithBox, cho
tới khi khoanh đúng vùng. Chữ ký cuối cùng:

| Điều kiện | Scarlet Rot | Poison |
|---|---|---|
| `SpCategory` | `10005` | `10004` |
| `StateInfo` | `5` | `2` |
| `ChangeHpRate != 0` | có | có |
| `EffectEndurance == 90` | có | có |

- **`StateInfo` khác nhau giữa 2 ailment**, không dùng chung được. Giả định
  ban đầu "`5` cho cả hai" khiến Poison lọc ra **0 dòng**; người dùng tự tra
  SmithBox mới ra `2`. Vì vậy `Ailment` giữ `state_info` riêng cho từng loại.
- `ChangeHpRate != 0` chỉ giữ dòng thật sự gây mất máu theo thời gian (loại
  các dòng chỉ tăng build-up).
- **`EffectEndurance == 90` vừa là thời lượng vanilla, vừa là điều kiện loại
  bỏ** những nhóm trùng 3 điều kiện trên nhưng không phải "người chơi tự gây
  ra":
  - `180`: hồ Rot/hồ Poison môi trường (`TargetEnemy=false`) và "NPC: Scarlet
    Rot"/"NPC: Poison" (quái tự gây);
  - `300`: hiệu ứng rot riêng của Malenia (boss);
  - `30`: "NPC: Fast Poison" và vài dòng vô danh không nằm trong
    `CycleOccurrenceSpEffectId` (one-shot, không thuộc chu kỳ DoT).

Kết quả: **175 dòng Scarlet Rot + 203 dòng Poison**, đối chiếu tay với SmithBox
từng ID (chỉ còn 1 dòng Poison ID `3750` chưa ai đặt tên; không phải lỗi lọc).

## Field được ghi

| Field | Ý nghĩa | Vanilla Scarlet Rot | Vanilla Poison |
|---|---|---|---|
| `EffectEndurance` | Thời lượng (giây), `-1` = vô hạn | 90 | 90 |
| `ChangeHpRate` | % máu tối đa mất mỗi tick | `0.18` (một số dòng `0.33`) | `0.07` |
| `ChangeHpPoint` | Sát thương cố định cộng thêm mỗi tick | `15` | `7` |

Giả định ban đầu "`FixedDamage` vanilla = 0" là **sai**; giá trị thật ở bảng
trên do người dùng tự đọc trong SmithBox. Các giá trị này là mặc định trong
`InfiniteAilment.ini`.

## Vì sao cache danh sách ID sau lần quét đầu

Điều kiện lọc có `EffectEndurance == 90`, nên sau khi ghi đè lần đầu, quét lại
sẽ không còn khớp chính các dòng vừa sửa: reload trông như "không ăn" dù lần
đầu đã ăn. `Ailment::matched_row_ids` (`Mutex<Option<Vec<u32>>>` riêng cho mỗi
ailment) lưu ID tìm được ở lần quét đầu và dùng lại cho mọi lần reload sau.
Mod ghi **giá trị tuyệt đối** (không nhân hệ số), nên reload không cộng dồn và
không cần snapshot gốc. Cùng loại bug đã gặp ở `drop_rate`/`enemy_scaling`.

## Giới hạn

- Mod dùng `repo.rows::<SpEffectParam>()` của fromsoftware-rs (lần quét đầu).
  Hàm này có thể panic với regulation bị lệch header/runtime (xem
  `mods/dropmultiplier/docs/convergence_panic.md`); người báo lỗi xác nhận
  InfiniteAilment chạy tốt trên Convergence nên chưa chuyển sang
  `common::params::for_each_row_mut`.
- Chữ ký lọc suy ra từ vanilla; regulation overhaul đổi `EffectEndurance` gốc
  (khác 90) sẽ không khớp dòng nào.

# InfiniteAilment

Mod cho Elden Ring: cho phép chỉnh thời lượng (`Duration`) và sát thương
(`PercentDamage`/`FixedDamage`) của hiệu ứng damage-over-time Poison và
Scarlet Rot do người chơi gây ra, qua `InfiniteAilment.ini`. **Đã xác
nhận hoạt động đúng trong game** (2026-09-22) - xem mục cuối cùng bên
dưới để biết cơ chế/field cuối cùng thực sự dùng; các mục phía trên ghi
lại quá trình điều tra dẫn tới đó (gồm cả hướng sai đã thử và bỏ).

## Tạo mod (2026-09-21)

Xuất phát từ nhu cầu ban đầu của người dùng: "tăng thời gian Poison/Scarlet
Rot từ 90s lên vô hạn". Qua điều tra thực tế trên `SpEffectParam.csv`
(export bằng SmithBox, cả bản lọc "Scarlet Rot" lẫn bản lọc "Poison"),
xác định lại đúng vấn đề:

- Con số **90s** người dùng thấy ban đầu (ở các dòng
  `106xxx`/`107xxx`, dạng "Poison/Scarlet Rot - Low/Medium/High -
  Addition/Innate +0..+25") **không phải** thời lượng của bản thân
  DoT - đó là thời lượng của 1 buff riêng (tăng lượng điểm build-up
  gây ra mỗi hit, ví dụ từ talisman "Ailment Talisman").
- Effect DoT thật sự (tick sát thương định kỳ sau khi thanh build-up
  đầy) nằm ở **1 dòng chung duy nhất, ID `505`** ("Poison/Scarlet Rot
  (Cycled)") - `EffectEndurance=900`. Cả họ `106xxx` (Poison) lẫn
  `107xxx` (Scarlet Rot) đều có field `cycleOccurrenceSpEffectId=505`
  trỏ về đúng dòng này, nghĩa là **2 ailment dùng chung 1 effect**,
  không tách được ở tầng param này trừ khi tự nhân bản dòng 505 thành
  2 ID riêng và trỏ lại từng họ - không cần thiết vì người dùng xác
  nhận muốn cả 2 đều vô hạn.
- Xác nhận `-1` là quy ước "vĩnh viễn/không hết hạn" của chính field
  `EffectEndurance` trong bộ dữ liệu này (dòng `90010`, "NPC: Disable
  Scarlet Rot", ship sẵn `EffectEndurance=-1`) - không phải suy đoán
  từ tài liệu ngoài.
- `effectTarget*` trong `SpEffectParam` chỉ là cờ "loại thực thể nào
  được phép nhận effect", không cố định phe - effect áp lên bất kỳ ai
  (người chơi hay quái) có thanh build-up đầy, đối xứng 2 chiều.

Field cần sửa: `SP_EFFECT_PARAM_ST::effect_endurance` (kiểu `f32`, đã có
sẵn getter/setter trong `eldenring` crate qua `SoloParamRepository`), áp
`-1.0` cho đúng 1 row ID `505`, đúng 1 lần lúc khởi động (`status_effect.rs`)
- không có lý do hot-reload vì đây là 1 giá trị tĩnh cố định, giống cách
`sometweaks::misc::unlock_enchantments` chỉ áp 1 lần.

Tách thành mod riêng (không gộp vào `sometweaks`) theo lựa chọn của người
dùng, cùng kiểu cấu trúc với `dropmultiplier`/`weightmultiplier`.

**Chưa test trong game** - engine thực tế dùng `EffectEndurance` thế nào
(là cap an toàn hay là thời lượng cảm nhận trực tiếp) chỉ được suy ra từ
phân tích CSV tĩnh, không phải quan sát runtime. Cần người dùng tự thử
trong game (gây Poison/Scarlet Rot, xem DoT có còn tự tắt sau ~15 phút
hay không) trước khi coi tính năng này là đã xác nhận hoạt động đúng.

## Bỏ việc ghi row 505, chuyển sang chỉ đọc/log để chẩn đoán (2026-09-22)

Trong lúc lần theo chuỗi field để tìm ra cột damage thật (`behaviorId=2105`
→ `BehaviorParam_PC` (refType=Bullet, refId=1005) → `Bullet` 1005 →
`atkId_Bullet=2` → `AtkParam` ID `2` **"Blood Loss - Bullet"**), phát
hiện dòng `AtkParam` phía cuối chuỗi này **dùng chung với hiệu ứng Bleed**
(tên dòng ghi rõ "Blood Loss", không phải tên riêng cho Poison/Scarlet
Rot). Việc này gieo nghi ngờ ngược lại cho chính bước đầu (sửa
`EffectEndurance` ở row `505`): toàn bộ suy luận trước giờ chỉ dựa vào đọc
tĩnh CSV, chưa từng quan sát hành vi thật trong game, nên người dùng quyết
định **không tin tưởng đủ để ghi đè** field này nữa - dừng thay đổi, gỡ bỏ
logic ghi (`row.set_effect_endurance(...)`, cấu hình `Duration` trong
`InfiniteAilment.ini`).

`status_effect.rs` giờ chỉ **đọc và log** giá trị hiện tại của row `505`
(`EffectEndurance`, `MotionInterval`, `BehaviorId`) lúc khởi động và mỗi
khi bấm `ReloadKey` - không ghi gì vào game. Mục đích: cho phép so sánh
giá trị trước/sau khi tự kích hoạt Poison/Scarlet Rot thật trong game,
kiểm chứng đúng field/đúng hành vi trước khi quay lại việc sửa (nếu có).

## Tìm lại đúng row set bằng cách lọc trực tiếp trong code (không phải CSV tĩnh), rồi ghi đè - đã xác nhận hoạt động (2026-09-22)

Thay vì tiếp tục suy đoán từ CSV export (đã sai 1 lần với row `505`),
chuyển hẳn sang **lọc trực tiếp trên dữ liệu `SpEffectParam` thật đang
chạy trong game** (`repo.rows::<SpEffectParam>()`), thêm dần từng điều
kiện field, mỗi lần in ra số dòng khớp + toàn bộ giá trị field liên quan
để mắt thường đối chiếu với SmithBox, cho tới khi khoanh đúng vùng:

- `SpCategory` = `10005` (Scarlet Rot) hoặc `10004` (Poison) - category
  riêng của từng ailment.
- `StateInfo` = `5` (Scarlet Rot) hoặc `2` (Poison) - **khác nhau giữa 2
  ailment**, không dùng chung 1 giá trị được (giả định ban đầu `5` cho cả
  2 khiến Poison lọc ra 0 dòng, phải sửa lại sau khi người dùng tự tra
  SmithBox).
- `ChangeHpRate != 0` - chỉ giữ dòng thực sự gây mất máu theo thời gian.
- `EffectEndurance == 90` - chính là thời lượng vanilla, đồng thời cũng
  là điều kiện loại bỏ các nhóm không liên quan tới "người chơi tự gây
  ra" tình cờ trùng 3 điều kiện trên: hồ Rot/hồ Poison môi trường
  (`EffectEndurance=180`, `TargetEnemy=false`), hiệu ứng riêng của
  Malenia (`300`), "NPC: Scarlet Rot"/"NPC: Poison"/"NPC: Fast Poison"
  (`180`/`30`, quái tự gây chứ không phải người chơi), và vài dòng vô
  danh không lặp vào `CycleOccurrenceSpEffectId` (`30`, one-shot).

Field thật sự sửa: `EffectEndurance` (thời lượng), `ChangeHpRate` (%
sát thương/tick, vanilla `0.18`/`0.33` tùy dòng cho Scarlet Rot,
`0.07` cho Poison), `ChangeHpPoint` (sát thương cố định cộng thêm,
vanilla `15` cho Scarlet Rot, `7` cho Poison - lưu ý: giả định ban đầu
`FixedDamage` vanilla `= 0` là **sai**, đã sửa lại sau khi người dùng tự
đọc giá trị thật trong SmithBox).

Danh sách ID khớp được **cache lại sau lần quét đầu tiên**
(`Ailment::matched_row_ids`, 1 `Mutex<Option<Vec<u32>>>` riêng cho mỗi
ailment) thay vì quét lại theo field mỗi lần `ReloadKey` - vì chính điều
kiện lọc có chứa `EffectEndurance == 90`, nên sau khi ghi đè lần đầu, quét
lại sẽ không còn khớp chính các dòng vừa sửa nữa (giống bug từng gặp ở
`drop_rate`/`enemy_scaling`, đã biết trước và tránh được ngay từ đầu).

Kết quả: 175 dòng Scarlet Rot + 203 dòng Poison khớp đúng (đối chiếu tay
với SmithBox từng ID một, chỉ còn duy nhất 1 dòng Poison ID `3750` chưa
ai đặt tên - không phải lỗi của quá trình lọc). Cấu hình cuối trong
`InfiniteAilment.ini`: `ScarletRot.Duration/PercentDamage/FixedDamage` và
`Poison.Duration/PercentDamage/FixedDamage` (namespace phẳng, không phân
biệt theo `[Section]`, chỉ prefix tên key). **Người dùng đã tự test trong
game và xác nhận hoạt động đúng** ("phù hợp, không chỉnh sửa gì thêm").

## `LogFile=false` giờ không tạo file log nữa - sửa trong `common::logger` (2026-09-23)

Người dùng phát hiện (qua PassiveRunes): `LogFile=false` nhưng file `.log`
vẫn được tạo. Đúng là trước đó `logger::init` được gọi vô điều kiện trong
`lib.rs` - file luôn tạo, `LogFile` chỉ gate log chi tiết - trái với tên
key và với cách RiseArcher/RuneMultiplier vốn làm. Người dùng xác nhận hành
vi đúng: `LogFile=true` mới tạo file; muốn có log sẵn thì deploy với mặc
định `LogFile=true` (mod này mặc định `true`).

Sửa tập trung trong `shared/src/logger.rs`: `init` chỉ ghi nhớ đường dẫn,
file được tạo (truncate) ở dòng log đầu tiên khi `LogFile=true`, `LogFile`
đọc lại mỗi lần ghi - bật/tắt bằng `ReloadKey` có hiệu lực ngay. Comment
"log is always on" trong `lib.rs` đã bỏ; mô tả key trong ini đổi thành
"Write InfiniteAilment.log next to the DLL (for troubleshooting). Off = no log
file". Các mục cũ hơn trong README nói "file log luôn được tạo bất kể
`LogFile`" giờ đã lỗi thời.


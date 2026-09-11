---
name: deploy-nexus-mod
description: Build, package, and publish a new mod file version + changelog to Nexus Mods for one of this workspace's mods. Reconciles the changelog draft in DESCRIPTION.bbcode against the real changelog on Nexus, confirms version + changelog with the user, builds and zips the release, then confirms once more before actually uploading. Use when asked to publish/deploy/release a mod to Nexus, or push a new version.
---

# Publish a mod to Nexus Mods

4 bước, đúng thứ tự, **2 điểm dừng bắt buộc** (cuối bước 2, cuối bước 4)
trước khi đi tiếp - đây là hành động công khai, khó đảo ngược. Cả 2 điểm
dừng này **phải dùng tool `AskUserQuestion`**, không được chỉ hỏi bằng văn
bản thường rồi tự suy đoán tin nhắn tiếp theo của người dùng là đồng ý -
từng bị bỏ qua thật (hỏi chay bằng text, không có hộp thoại xác nhận nào
hiện ra) nên nhắc lại rõ ở đây.

## Mod ID mapping + trạng thái deploy: `manifest.json`

`manifest.json` ở gốc repo là nguồn thật cho cả 2 việc: mod nào đã có trang
Nexus, và **lần deploy gần nhất của mỗi mod dừng ở commit nào** - đây là
input quan trọng cho Giai đoạn 2 (dò xem có tính năng nào chưa vào
changelog). Cấu trúc:

```json
[
  {
    "id": "10548",              // game_scoped_id thật trên Nexus (số, dạng string)
    "internalId": "18610093304116", // Nexus internal mod id (cache sẵn từ
                                 // GET /v3/games/eldenring/mods/{game_scoped_id}
                                 // - đỡ phải gọi lại API này mỗi lần deploy,
                                 // nhưng vẫn nên double-check nếu nghi lỗi thời)
    "name": "Auto Regen",       // TÊN HIỂN THỊ TRÊN NEXUS - CHỈ để hiển thị
                                 // cho người dùng đọc, KHÔNG được suy ra tên
                                 // thư mục crate từ field này (từng sai thật:
                                 // mod 10549 hiển thị "Reduction Weight" trên
                                 // Nexus nhưng nằm ở crates/weightmultiplier)
    "crate": "autoregen",       // tên thư mục thật dưới crates/ - dùng field
                                 // này để tìm file, KHÔNG suy đoán từ "name"
    "deploys": [                // LỊCH SỬ deploy - luôn APPEND, không ghi đè.
      {
        "version": "2.4.0",     // version publish thật (đã xác nhận qua API)
        "commit": "<full sha>"  // commit HEAD dùng để build bản đó
      }
    ]
  }
]
```

Phần tử **cuối cùng** trong `deploys` = trạng thái deploy mới nhất đã biết
- đây là giá trị dùng để đối chiếu ở Bước 2. Các phần tử ban đầu (tính đến
lúc file này được tạo) là **suy luận best-effort**, đối chiếu changelog
thật trên Nexus với git log local (không có neo nào đáng tin trước đó) -
có thể sai, đặc biệt entry nào version ghi `"unknown"` (VD `risearcher` -
API Nexus không trả về dữ liệu changelog nào cho mod này) thì coi là
**chưa xác minh**, hỏi lại người dùng thay vì tin tuyệt đối.

`sometweaks` **chưa có trên Nexus** nên không có entry - không đưa vào
danh sách chọn ở Bước 1, không tự đoán ID nếu người dùng nhắc tới nó.

`game_domain` luôn là `eldenring`. API key ở `.secrets/nexus_api_key.txt`
(đã gitignore). Nếu `manifest.json` có vẻ lỗi thời (VD version không khớp
`CLAUDE.md` ở gốc repo, mục "Đối chiếu changelog thật trên Nexus Mods...")
- ưu tiên đọc lại `manifest.json` làm chuẩn vì nó được chính skill này cập
nhật liên tục, còn `CLAUDE.md` chỉ là bảng tham khảo tĩnh.

## Bước 1 - Hỏi mod cần deploy

Nếu người dùng chưa nói rõ mod nào khi gọi skill này, hỏi (VD qua
`AskUserQuestion`) - liệt kê `name` (tên hiển thị Nexus) của từng entry
trong `manifest.json` cho người dùng chọn. Sau khi chọn xong, lấy `crate`
của đúng entry đó - mọi bước sau (đọc file, build...) dùng `<crate>` này,
không suy đoán từ `name`.

## Bước 2 - Đối chiếu changelog local ↔ Nexus, chốt version + changelog

1. Đọc `crates/<crate>/DESCRIPTION.bbcode`, tìm phần sau
   `[size=4][b]Changelog[/b][/size]`. Parse **mọi** block theo mẫu:
   ```
   [b]<version>[/b]
   [list]
   [*]<dòng 1>[/*]
   [*]<dòng 2>[/*]
   ...
   [/list]
   ```
   Block **đầu tiên** (trên cùng) = ứng viên version + changelog sắp
   publish.

2. Gọi API v1 lấy changelog THẬT (luôn gọi mới, không tin dữ liệu cũ trong
   context từ trước đó trong hội thoại):
   ```bash
   API_KEY=$(cat .secrets/nexus_api_key.txt)
   curl -sS -H "apikey: $API_KEY" \
     "https://api.nexusmods.com/v1/games/eldenring/mods/<game_scoped_id>/changelogs.json"
   ```
   **Cố tình dùng v1, không phải v3** cho bước đọc này - đã kiểm tra
   `.docs/nexus-openapi.yaml` (spec v3), **không có endpoint đọc changelog
   nào trong v3 cả** (chỉ có `POST /mods/{id}/changelogs` để ghi, xem bước
   13). Đừng tự ý đổi sang v3 chỉ vì nó "mới hơn" - v3 mới chưa đầy đủ
   tính năng cho việc này.

3. Đối chiếu 2 phía, phát hiện gap:
   - Nexus có version nào mà local (`DESCRIPTION.bbcode`) đang thiếu → đây
     là gap (từng gặp thật, VD thiếu `2.4.0`). **Không tự sửa file** - báo
     rõ ràng cho người dùng thấy gap này (version nào, nội dung Nexus ghi
     gì) để họ quyết định sửa `DESCRIPTION.bbcode` thế nào.
   - Nếu API trả về thiếu vài version cũ hơn nữa so với thực tế trang web
     (đã từng xảy ra, xem `reference_nexus_api` trong memory) - không phải
     lỗi của bạn, chỉ cần nêu ra nếu thấy nghi vấn, không tự xoá gì.
   - **Version ứng viên (bước 1) trùng với version mới nhất đã publish
     thật** (nghĩa là local chưa có bản nháp changelog nào mới hơn) →
     **KHÔNG được tự kết luận "không có gì mới" rồi dừng câm lặng** - đây
     là dấu hiệu người dùng có thể đã code xong tính năng nhưng quên viết
     changelog. Đọc `commit` của phần tử **cuối cùng** trong mảng
     `deploys` của đúng mod này trong `manifest.json` (commit HEAD tại lần
     publish gần nhất được biết, chính xác hơn nhiều so với suy đoán qua
     timestamp file), rồi liệt kê mọi commit đổi code/ini của crate này kể
     từ đó:
     ```bash
     git log --oneline <commit>..HEAD -- crates/<crate>/src crates/<crate>/*.ini
     ```
     Nếu mảng `deploys` rỗng, hoặc phần tử cuối có `version: "unknown"`
     (chưa xác minh được, xem mục "Mod ID mapping" ở trên) - báo cho người
     dùng biết dữ liệu neo đang không chắc chắn trước khi dùng nó để dò
     gap, để họ tự cân nhắc thêm.

     Nếu có commit nào đổi hành vi thật (không phải chỉ sửa README/comment)
     xuất hiện sau mốc trên → nêu rõ danh sách commit đó cho người dùng,
     hỏi thẳng: "Không thấy bản nháp changelog mới, nhưng các commit này có
     vẻ chưa được ghi vào changelog - có đúng không, và nếu đúng thì version
     mới + nội dung changelog nên là gì?" Chờ người dùng cung cấp version +
     nội dung, rồi **tự thêm block mới** đó vào đầu phần `[Changelog]` trong
     `DESCRIPTION.bbcode` (dùng Edit tool, theo đúng văn phong 1 dòng/tính
     năng - xem `feedback_changelog_style` trong memory) trước khi coi đó
     là ứng viên version ở bước 4.

     Chỉ khi không có commit code nào đáng kể sau mốc trên, hoặc người dùng
     xác nhận rõ ràng "đúng là chưa có gì mới" - mới được kết luận không có
     gì để publish và dừng skill tại đây.

4. **Không có gap** (hoặc gap đã được người dùng xử lý xong): trình bày rõ
   ràng version + toàn bộ nội dung changelog (nguyên văn từng dòng) sắp
   dùng để publish, rồi **BẮT BUỘC gọi tool `AskUserQuestion`** (không phải
   chỉ hỏi bằng văn bản thường trong câu trả lời) với câu hỏi dạng "Version
   và changelog này đã đúng chưa?" + 2 lựa chọn (VD "Đúng, tiếp tục" /
   "Chưa đúng, dừng lại"). Đây là 1 trong 2 điểm dừng bắt buộc của cả skill
   (xem đầu file) - lý do phải dùng đúng tool này (không hỏi chay bằng
   text): nếu chỉ viết câu hỏi trong text trả lời rồi tự đi tiếp ở lượt sau
   khi thấy người dùng gõ gì đó, rất dễ vô tình chạy tiếp Bước 3 dựa trên 1
   tin nhắn không phải là xác nhận rõ ràng (từng xảy ra thật). Chỉ đi tiếp
   sang bước 3 khi `AskUserQuestion` trả lại lựa chọn xác nhận.

## Bước 3 - Tự build + đóng gói

`build-mod.ps1` cần tên PascalCase (`-Mod AutoRegen`), khác với `crate`
(lowercase, `autoregen`) dùng ở Bước 2. Lấy đúng tên này từ
`crates/<crate>/Cargo.toml`, mục `[lib] name` - **không** tự viết hoa chữ
cái đầu crate slug (không phải lúc nào cũng khớp, VD tên thư mục và tên
`[lib]` có thể khác nhau về cách viết hoa/cách ly từ).

Sau khi version đã chốt ở bước 2, ghi lại commit HEAD hiện tại **trước khi
build** - đây sẽ là giá trị ghi vào `manifest.json` ở cuối Bước 4, để
tránh lệch nếu có commit khác lọt vào giữa lúc skill đang chạy:

```bash
build_commit=$(git rev-parse HEAD)
```

Rồi tự chạy (không cần hỏi lại):

```powershell
pwsh -File scripts/build-mod.ps1 -Mod <Mod> -Zip -Version <version đã chốt>
```

Kết quả mong đợi: `build/<Mod>-<version>.zip`. Nếu script báo lỗi (build
fail, thiếu ini...) - dừng lại, báo nguyên văn lỗi cho người dùng, không tự
sửa code để "cho qua".

## Bước 4 - Xác nhận lần cuối rồi publish

5. Lấy mod ID nội bộ Nexus (khác `game_scoped_id`) từ `internalId` đã cache
   sẵn trong `manifest.json`. Chỉ gọi lại API để xác nhận nếu nghi ngờ đã
   lỗi thời (hiếm khi đổi):
   ```bash
   curl -sS -H "apikey: $API_KEY" \
     "https://api.nexusmods.com/v3/games/eldenring/mods/<game_scoped_id>"
   ```
   Lấy `.data.id` - nếu khác với `internalId` trong manifest, cập nhật lại
   luôn cho đúng.

6. Liệt kê mod file (update group) hiện có, chọn đúng cái cần thêm version:
   ```bash
   curl -sS -H "apikey: $API_KEY" "https://api.nexusmods.com/v3/mods/<id>/files"
   ```
   Nếu có nhiều hơn 1 file khả dĩ (hiếm với các mod đơn giản trong repo
   này) - hỏi người dùng chọn, không tự đoán.

7. Tính metadata file zip vừa build ở bước 3:
   ```bash
   size_bytes=$(stat -c%s "<zip_path>" 2>/dev/null || stat -f%z "<zip_path>")
   md5_hex=$(md5sum "<zip_path>" | cut -d' ' -f1)
   ```

8. **Dừng lại, trình bày tóm tắt đầy đủ lần cuối trong text**, gồm:
   - Mod + game_scoped_id + Nexus internal id.
   - Mod file (update group) sẽ được thêm version vào (tên + id).
   - Version + toàn bộ changelog (đã chốt ở bước 2, nhắc lại cho chắc) -
     dùng ở cả 2 nơi: `description` của file (bước 12) và endpoint
     changelog riêng (bước 13).
   - Tên file zip vừa build, kích thước, md5.

   Sau đó **BẮT BUỘC gọi tool `AskUserQuestion`** (không phải chỉ hỏi bằng
   văn bản thường) với câu hỏi "Xác nhận publish?" + 2 lựa chọn (VD "Xác
   nhận, publish ngay" / "Dừng lại, chưa publish"). Đây là điểm dừng bắt
   buộc thứ 2 của cả skill (xem đầu file). Chỉ đi tiếp sang bước 9 khi
   `AskUserQuestion` trả lại lựa chọn xác nhận trong chính lượt chạy này -
   một sự đồng ý chung chung trước đó trong cuộc trò chuyện (VD "cứ làm
   đi") không tính, và tuyệt đối không tự suy ra "chắc là đồng ý" từ 1 tin
   nhắn không phải câu trả lời của `AskUserQuestion`.

9. Tạo upload session:
   ```bash
   curl -sS -X POST -H "apikey: $API_KEY" -H "Content-Type: application/json" \
     -d "{\"size_bytes\": $size_bytes, \"filename\": \"<zip_filename>\", \"md5\": \"$md5_hex\"}" \
     "https://api.nexusmods.com/v3/uploads"
   ```
   Lấy `data.id` (upload_id) và `data.presigned_url`.

10. Upload file lên `presigned_url` (PUT, bắt buộc `Content-Disposition`
    khớp đúng filename đã gửi ở bước 9, `Content-MD5` là base64 của cùng
    md5 đó (không phải hex), **và bắt buộc `Content-Type:
    application/octet-stream`**):
    ```bash
    md5_b64=$(openssl dgst -md5 -binary "<zip_path>" | base64)
    curl -sS -X PUT "<presigned_url>" \
      -H "Content-Disposition: attachment; filename=\"<zip_filename>\"" \
      -H "Content-MD5: $md5_b64" \
      -H "Content-Type: application/octet-stream" \
      --data-binary @"<zip_path>"
    ```
    **Quan trọng - lỗi thật đã gặp (2026-09-11)**: `presigned_url` trả về
    ở bước 9 có `X-Amz-SignedHeaders` luôn gồm cả `content-type` (kiểm tra
    query string của URL để xác nhận), nhưng **tài liệu API không hề nhắc
    tới việc này** - chỉ nói rõ `Content-Disposition`/`Content-MD5` là bắt
    buộc. Thiếu header `Content-Type` (hoặc gửi sai giá trị) khiến `curl`
    tự thêm `Content-Type: application/x-www-form-urlencoded` mặc định khi
    dùng `--data-binary`, không khớp chữ ký → lỗi `SignatureDoesNotMatch`
    từ Cloudflare R2 (nơi Nexus lưu file), **trông giống lỗi phía server
    nhưng thực chất là thiếu header ở phía client**. Đã verify thật: thêm
    đúng `Content-Type: application/octet-stream` → PUT trả `200` ngay.
    Nếu vẫn gặp `SignatureDoesNotMatch` dù đã có đủ 3 header này - lúc đó
    mới nên nghi ngờ là lỗi tạm thời phía Nexus/Cloudflare, không phải lặp
    lại lỗi thiếu header này nữa.

11. Finalise rồi poll tới khi sẵn sàng:
    ```bash
    curl -sS -X POST -H "apikey: $API_KEY" \
      "https://api.nexusmods.com/v3/uploads/<upload_id>/finalise"
    # poll:
    curl -sS -H "apikey: $API_KEY" "https://api.nexusmods.com/v3/uploads/<upload_id>"
    # lặp lại vài giây/lần tới khi data.state == "available"
    ```

12. Gắn upload thành version mới của đúng mod file đã chọn ở bước 6 (dùng
    `createModFileVersion`, **không** dùng `POST /mod-files` - endpoint đó
    tạo file hoàn toàn mới, sai mục đích):
    Đưa luôn nội dung changelog (đã chốt ở bước 2) vào `description` - để
    người xem "Files" tab thấy ngay đổi gì mà không cần lật sang mục
    Changelog riêng:
    ```bash
    curl -sS -X POST -H "apikey: $API_KEY" -H "Content-Type: application/json" \
      -d "{\"upload_id\": \"<upload_id>\", \"name\": \"<Mod>.zip\", \"version\": \"<version>\", \"file_category\": \"main\", \"update_mod_version\": true, \"description\": \"<changelog text, mỗi dòng 1 bullet trần (KHÔNG tự thêm dấu \\\"- \\\" ở đầu) - giống hệt nội dung gửi ở bước 13>\"}" \
      "https://api.nexusmods.com/v3/mod-files/<mod_file_id>/versions"
    ```

13. Đẩy changelog (cộng dồn - chỉ gọi đúng 1 lần cho version này, gọi lại
    sẽ bị lặp dòng vì API là additive):
    ```bash
    curl -sS -X POST -H "apikey: $API_KEY" -H "Content-Type: application/json" \
      -d "{\"version\": \"<version>\", \"changelog\": \"<changelog text, mỗi bullet 1 dòng, phân cách bằng newline (\\\\n)>\"}" \
      "https://api.nexusmods.com/v3/mods/<id>/changelogs"
    ```
    **Không tự thêm dấu `-` (hay bất kỳ ký tự bullet nào) ở đầu mỗi dòng**
    - lỗi thật đã gặp (2026-09-11): Nexus tự hiển thị mỗi dòng thành 1 mục
    danh sách riêng trên trang mod rồi, tự thêm `- ` vào text khiến hiện ra
    2 dấu gạch đầu dòng chồng lên nhau (`- - Added ...`). Mỗi dòng trong
    `changelog`/`description` chỉ nên là đúng nguyên văn bullet gốc (VD
    lấy thẳng nội dung bên trong `[*]...[/*]` của `DESCRIPTION.bbcode`,
    không thêm gì phía trước).

14. Sau khi bước 12-13 thành công (không lỗi), **append** 1 phần tử mới
    vào cuối mảng `deploys` của đúng mod này trong `manifest.json`:
    `{"version": "<version vừa publish>", "commit": "$build_commit"}`
    (`$build_commit` đã ghi lại ở Bước 3 - không dùng `git rev-parse HEAD`
    lại lần nữa ở đây, có thể đã trôi nếu có commit mới trong lúc chờ
    upload/poll). **Không xoá/ghi đè các phần tử cũ** - đây là lịch sử
    cộng dồn. Đây là bước không cần hỏi xác nhận thêm - việc ghi lại trạng
    thái deploy là hệ quả tự nhiên của việc vừa publish thành công, không
    phải 1 thay đổi rủi ro riêng.

15. Báo lại kết quả cho người dùng: version mới đã lên chưa (link mod
    page), `manifest.json` đã cập nhật xong, và nhắc rằng **có thể vẫn cần
    vào tay trang Nexus** để kiểm tra file mới có tự động là "primary
    download" chưa hay cần tự đặt - skill không tự xác nhận được việc này
    qua API.

## Ghi chú an toàn

- Không bao giờ chạy bước 9 trở đi nếu chưa hoàn thành đúng bước 8 (xác
  nhận tường minh trong lượt chạy hiện tại).
- Không tự bịa version hay nội dung changelog - luôn lấy nguyên văn từ
  `DESCRIPTION.bbcode`, không suy đoán. **Không** lấy version từ
  `Cargo.toml` của crate - version đó không đồng bộ với version publish
  trên Nexus trong workspace này (VD `autoregen` Cargo.toml đang `2.0.0`
  trong khi Nexus đã publish tới `2.4.0`).
- Nếu bất kỳ bước nào trả lỗi (401/403/422...) - dừng lại, đọc
  `ProblemDetails.detail` trong response, báo người dùng nguyên văn lỗi
  thay vì tự thử lại nhiều lần.

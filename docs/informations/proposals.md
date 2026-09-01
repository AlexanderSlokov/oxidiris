# Oxidiris — Đề xuất Thiết kế & Kiến trúc

> **Trạng thái:** Bản nháp v2 (đang thảo luận)
> **Phạm vi:** Định vị sản phẩm, kiến trúc crate, tiêu chuẩn thiết kế, engine parser, hạ tầng dự án.

---

## 0. Định vị sản phẩm

Oxidiris là công cụ TUI áp dụng kỹ thuật **RSVP** (Rapid Serial Visual Presentation) kết hợp
**ORP** (Optimal Recognition Point), cho phép đọc tài liệu kỹ thuật mà mắt gần như không phải
di chuyển.

### 0.1. Đối tượng người dùng

| Nhóm | Nhu cầu đặc thù |
|---|---|
| Lập trình viên | Đọc nhanh README, changelog, RFC, docs.rs mà không rời terminal |
| Nghiên cứu sinh / học thuật | Quét (skim) paper arXiv/IEEE dạng PDF 2 cột để sàng lọc trước khi đọc kỹ |
| Người mắc chứng khó đọc (dyslexia) | RSVP loại bỏ nhu cầu bám dòng (line tracking) — một trong những rào cản lớn nhất của nhóm này |
| Người có hạn chế vận động mắt | Giảm thiểu saccade, giảm mỏi mắt khi đọc dài |

Việc xác định rõ nhóm thứ 3 và thứ 4 là lý do Oxidiris được xếp vào nhóm **Assistive Technology**,
kéo theo các ràng buộc bắt buộc ở Chương 3.4 (không phải khuyến nghị "nice-to-have").

### 0.2. Giới hạn khoa học cần thành thật thừa nhận

RSVP **không phải** thuốc tiên. Nghiên cứu về đọc cho thấy khoảng 10–15% chuyển động mắt khi đọc
tự nhiên là **regression** (liếc ngược lại từ đã đọc) — cơ chế sửa lỗi hiểu của não bộ. RSVP triệt
tiêu hoàn toàn regression, nên **tốc độ tăng thường đánh đổi bằng khả năng hiểu và ghi nhớ**,
đặc biệt với văn bản dày đặc khái niệm như paper khoa học.

Hệ quả cho thiết kế — Oxidiris phải:

1. Định vị mình là công cụ **skim/triage** (sàng lọc, nắm ý chính), không phải công cụ thay thế
   việc đọc kỹ.
2. Coi các tính năng **Backstep**, **Review Mode**, và **Bảng Toàn Văn** là *tính năng bù trừ
   cho điểm yếu cố hữu*, không phải tính năng phụ. Chúng phải có mặt ngay từ v0.1.
3. Không quảng cáo con số WPM như chỉ số thành tích.

---

## 1. Crate: Binary hay Library?

Mô hình chuẩn của hệ sinh thái Rust là kết hợp cả hai qua **Cargo Workspace**:

* **Library (`oxidiris-core`)**: Toàn bộ logic xử lý văn bản — thuật toán RSVP, chia từ,
  tính điểm ORP, canh lề, cấu hình WPM, parse Markdown/LaTeX.
* **CLI/TUI Binary (`oxidiris`)**: Công cụ terminal hoàn chỉnh, cài qua `cargo install oxidiris`
  hoặc Homebrew.

**Lợi ích:** Khi tách logic core ra thư viện riêng, có thể tái sử dụng để nhúng lên Web (wasm),
làm GUI app (Tauri), plugin cho Obsidian/VS Code mà không cần viết lại thuật toán.

### 1.1. Cấu trúc thư mục đề xuất

> ⚠️ **Việc cần làm sớm.** Hiện tại repo đang là single-crate (`oxidiris/src/main.rs`).
> Chi phí tách workspace ở thời điểm này gần bằng 0; để đến Giai đoạn 2 (build wasm) mới tách
> sẽ tốn kém hơn nhiều vì lúc đó `core` đã lỡ phụ thuộc vào `ratatui`/`crossterm`.

```text
oxidiris/                  # workspace root
├── Cargo.toml             # [workspace] members = ["crates/*"]
├── crates/
│   ├── oxidiris-core/     # lib: KHÔNG phụ thuộc TUI, wasm-compatible
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── token.rs        # struct Token, TokenKind
│   │   │   ├── orp.rs          # thuật toán ORP
│   │   │   ├── pacing.rs       # tính duration cho từng token
│   │   │   ├── segment.rs      # tokenizer đa ngôn ngữ
│   │   │   └── parser/
│   │   │       ├── markdown.rs
│   │   │       ├── plaintext.rs
│   │   │       └── ...
│   │   └── tests/
│   └── oxidiris/          # bin: TUI
│       └── src/
│           ├── main.rs
│           ├── cli.rs          # clap
│           ├── app.rs          # state machine
│           ├── event.rs        # event loop
│           ├── theme.rs
│           └── ui/
├── docs/
└── testdata/              # corpus mẫu cho parser test
```

**Ràng buộc kiến trúc bắt buộc:** `oxidiris-core` **không được** phụ thuộc vào `ratatui`,
`crossterm`, hay bất kỳ crate I/O terminal nào. Nên thêm CI check `cargo build -p oxidiris-core
--target wasm32-unknown-unknown` ngay từ đầu để ràng buộc này không bị vi phạm âm thầm.

### 1.2. Ghi chú về giấy phép

Repo hiện dùng **GPL-3.0**. Cần lưu ý mâu thuẫn tiềm tàng với mục tiêu ở trên: GPL-3 sẽ ngăn
người khác nhúng `oxidiris-core` vào plugin/sản phẩm closed-source, làm hẹp đáng kể khả năng
lan tỏa của thư viện.

**Đề xuất:** Dual-license `oxidiris-core` theo **MIT OR Apache-2.0** (chuẩn de-facto của hệ sinh
thái Rust), giữ **GPL-3.0** cho binary `oxidiris`. Đây là mô hình phổ biến và hợp pháp.

`Cargo.toml` hiện cũng đang thiếu các field bắt buộc để publish lên crates.io:
`license`, `repository`, `readme`, `keywords`, `categories`, `rust-version`.

---

## 2. Roadmap

### Giai đoạn 1 — Core & CLI TUI

Xây dựng workspace với `ratatui`. Hỗ trợ đọc file trực tiếp từ máy:

```bash
oxidiris paper.md --wpm 400
```

Chia nhỏ thành các mốc:

| Mốc | Nội dung | Tiêu chí hoàn thành |
|---|---|---|
| **v0.1** | Tokenizer + ORP + pacing + TUI 1 panel (`focus` mode), chỉ `.txt` và `.md` | Đọc được README của chính dự án, ORP không nhảy cột |
| **v0.2** | Split-view (Bảng Toàn Văn), Outline/TOC, Backstep, progress bar | Đọc được paper `.md` dài, tua lại chính xác |
| **v0.3** | Theme, config file, auto-save vị trí, search | Dùng hằng ngày được |
| **v0.4** | LaTeX/Typst parser | Đọc được source `.tex` từ arXiv |
| **v0.5** | PDF (de-columnizing) | Đọc được paper 2 cột IEEE |
| **v1.0** | EPUB/HTML, đóng gói đa nền tảng, tài liệu đầy đủ | Publish crates.io + Homebrew |

### Giai đoạn 2 — Web Demo / Interactive Landing Page

Dùng `wasm-pack` xuất `oxidiris-core` sang web để tạo landing page cho người dùng test thử tốc độ
đọc RSVP ngay trên trình duyệt.

> Đây chính là phần thưởng của quyết định tách crate ở Chương 1 — nếu `core` sạch, giai đoạn này
> gần như chỉ là viết lớp UI.

### Giai đoạn 3 — Hệ sinh thái mở rộng

Extension trình duyệt, plugin VS Code/Neovim để đọc nhanh tài liệu mà không cần rời màn hình code.

---

## 3. Tiêu chuẩn thiết kế

Oxidiris thuộc nhóm Assistive Tech. Đối với dạng công cụ triệt tiêu chuyển động mắt này, quy chuẩn
thiết kế phải tuân theo nghiên cứu về công thái học thị giác (Visual Ergonomics) và tiêu chuẩn
tiếp cận W3C WCAG.

---

### 3.1. Quy chuẩn về Điểm Nhận Diện Tối Ưu (ORP)

Đây là quy chuẩn cốt lõi nhất của RSVP để mắt không phải dịch chuyển dù chỉ 1 milimet.

* **Quy tắc 1/3:** Khi một từ hiển thị, điểm tiêu điểm (một chữ cái duy nhất) nằm ở vị trí khoảng
  **30–40% chiều dài từ** (tính từ trái sang), không phải chính giữa.
* **Visual Anchor:** Chữ cái tại ORP phải được làm nổi bật **đồng thời bằng màu sắc *và* dấu hiệu
  hình học** (ví dụ: màu nhấn + cặp mũi tên `▼ ▲` ở trên/dưới). Xem lý do bắt buộc ở Chương 3.4.

#### 3.1.1. "Chiều dài từ" được đo bằng gì? — Ba cái bẫy

Đây là chi tiết quyết định thành bại. Nếu định nghĩa sai, chữ sẽ **nhảy ngang** mỗi lần đổi từ,
phá hủy đúng cái "Spatial Consistency" mà Chương 3.4 yêu cầu.

**Bẫy 1 — Grapheme cluster ≠ `char` ≠ byte.**
Tiếng Việt "ế" có thể là 1 code point (dạng NFC) hoặc 3 code point (dạng NFD: `e` + `◌̂` + `◌́`).
Văn bản thực tế trộn lẫn cả hai. Nếu tính ORP theo `char::count()`, cùng một từ sẽ ra chỉ số khác
nhau tùy nguồn file.

→ **Bắt buộc chuẩn hóa NFC (`unicode-normalization`) ngay tại khâu tokenize**, sau đó luôn đếm
theo **grapheme cluster** (`unicode-segmentation`), không bao giờ đếm theo `char` hay byte.

**Bẫy 2 — Chiều rộng hiển thị (display width).**
Ký tự CJK và phần lớn emoji chiếm **2 cột** terminal (UAX #11 East Asian Width). Từ "日本語" có
3 grapheme nhưng chiếm 6 cột. Căn ORP theo số grapheme sẽ khiến chữ lệch tâm.

→ **Vị trí neo trên màn hình phải tính bằng cột hiển thị (`unicode-width`), không phải số ký tự.**

**Bẫy 3 — Ký tự zero-width và tổ hợp.**
Dấu thanh rời, ZWJ trong emoji ghép, ký tự điều khiển bidi — đều có width 0 hoặc bất định.

→ Lọc bỏ ký tự điều khiển ở khâu tokenize; với emoji ghép, coi cả cụm ZWJ là 1 grapheme.

#### 3.1.2. Bảng tra ORP (giá trị mặc định)

Chỉ số ORP tính theo **grapheme cluster**, đánh số từ 0:

| Độ dài từ (grapheme) | Chỉ số ORP | % thực tế |
|---|---|---|
| 1 | 0 | 0% |
| 2 – 5 | 1 | 20–50% |
| 6 – 9 | 2 | 22–33% |
| 10 – 13 | 3 | 23–30% |
| ≥ 14 | 4 (trần) | ≤ 29% |

Áp trần ở chỉ số 4 vì với từ rất dài (thuật ngữ hóa học, tên hàm), đẩy tiêu điểm quá xa sang phải
sẽ làm phần đuôi từ tràn ra ngoài vùng thị giác trung tâm (foveal vision ~2°).

> **Cần kiểm chứng:** Bảng này là điểm khởi đầu theo quy ước chung của các công cụ RSVP hiện có.
> Nên có test case cụ thể cho tiếng Việt và CJK trước khi chốt.

#### 3.1.3. Thuật toán canh cột

```text
Khung hiển thị rộng W cột. Cột neo A = W / 2 (cố định tuyệt đối).

Với mỗi từ:
  1. Chuẩn hóa NFC.
  2. Tách thành Vec<grapheme>.
  3. Tra bảng → chỉ số ORP i.
  4. left_width  = tổng display-width của grapheme[0..i]
  5. padding_trái = A - left_width
  6. Render tại cột padding_trái.

=> Grapheme[i] luôn bắt đầu đúng tại cột A, bất kể độ dài từ.
```

**Trường hợp biên cần xử lý:**

| Tình huống | Xử lý |
|---|---|
| Từ dài hơn cả khung (URL, hash git) | Cắt bớt có `…`, hoặc hạ WPM và hiển thị nguyên khối |
| `padding_trái` âm (từ quá dài về bên trái) | Ghim về 0, chấp nhận lệch — vẫn tốt hơn là tràn khung |
| Từ 1 ký tự ("a", "I") | ORP = 0, neo tại A |
| Chuỗi số ("3.14159") | Coi là 1 token, không tách ở dấu chấm |

---

### 3.2. Quy chuẩn về Nhịp độ Đọc (Pacing & Contextual Delays)

Không được hiển thị mọi từ với thời gian bằng nhau, vì não bộ cần thời gian xử lý khác nhau cho
từng loại từ.

* **Dừng theo dấu câu (Punctuation Delays):** Gặp `.` `?` `!` → dừng lâu hơn **2–2.5×**; gặp
  `,` `;` `:` → **1.5×**.
* **Độ dài từ (Word Length Penalties):** Từ càng dài, thời gian hiển thị tăng tỷ lệ thuận theo
  số grapheme.
* **Kiểm soát WPM linh hoạt:** Người dùng tăng/giảm tốc độ tức thì bằng phím mũi tên, không cần
  mở cài đặt.

#### 3.2.1. Công thức tính thời lượng

```text
base_ms = 60_000 / wpm

duration = base_ms
         × length_factor      // 1.0 + max(0, len_grapheme - 6) × 0.05, trần 2.0
         × punctuation_factor // xem bảng dưới
         × kind_factor        // token loại đặc biệt (code, công thức...)
         + structural_pause   // cộng thêm, không nhân
```

| Ngữ cảnh | `punctuation_factor` |
|---|---|
| Từ thường | 1.0 |
| Kết thúc bằng `,` `;` `:` | 1.5 |
| Kết thúc bằng `.` `?` `!` | 2.25 |
| Kết thúc bằng `)` `"` `'` sau dấu câu | cộng dồn |

| Ngữ cảnh cấu trúc | `structural_pause` |
|---|---|
| Hết đoạn văn (paragraph) | +250 ms |
| Trước một Heading | +400 ms |
| Mỗi mục trong list | +150 ms |
| Vào/ra một code block | +300 ms |

#### 3.2.2. Bẫy: viết tắt bị nhầm là hết câu

`.` không phải lúc nào cũng là dấu chấm câu. Trong văn bản kỹ thuật và học thuật, chuỗi
`Fig. 3`, `et al.`, `i.e.`, `e.g.`, `vs.`, `No. 5`, `v1.2.3`, `Dr.`, `std::fmt` sẽ tạo ra
những khoảng dừng 2.25× hoàn toàn vô lý, làm nhịp đọc giật cục.

→ Cần **danh sách viết tắt** (abbreviation list) + heuristic: nếu token sau dấu chấm bắt đầu bằng
chữ thường hoặc chữ số, đó **không phải** hết câu.

#### 3.2.3. Ramp-up khi Resume *(mục mới)*

Bắn thẳng vào 450 WPM ngay sau khi nhấn Play là trải nghiệm gây sốc và thường làm mất chữ đầu tiên.

→ Khi resume: **tua lùi 2 từ**, khởi động ở ~60% WPM đích và tăng tuyến tính về 100% trong khoảng
5 từ. Tương tự khi bắt đầu đọc file lần đầu.

#### 3.2.4. WPM hiệu dụng (Effective WPM) *(mục mới)*

Do các hệ số nhân ở trên, tốc độ *thực tế* luôn thấp hơn WPM cài đặt (thường 15–25%). Hiển thị cả
hai con số để người dùng không bị nhầm lẫn:

```text
speed: 450 WPM  (eff. 361)
```

#### 3.2.5. Chống trôi nhịp (Clock Drift) — xem Chương 4.2

Đây là vấn đề hiện thực, không phải thiết kế, nhưng ảnh hưởng trực tiếp tới cảm giác nhịp điệu.

---

### 3.3. Quy chuẩn Hiển thị Bối cảnh (Spatial & Contextual Navigation)

Điểm yếu lớn nhất của RSVP là người đọc dễ bị "ngợp" hoặc mất dấu nếu lỡ lơ là 1 giây. Đây cũng là
nơi bù trừ cho giới hạn khoa học đã nêu ở Chương 0.2.

* **Thanh tiến trình (Progress Bar):** Hiển thị % trực quan ở góc dưới TUI.
* **Cơ chế Backstep:** `Space` tạm dừng, `H`/`←` tua lại 5 từ. **Bắt buộc phải có** cho tài liệu
  học thuật phức tạp.
* **Chế độ xem trước cấu trúc (Outline View):** Sidebar hiển thị cây Heading (H1, H2, H3) của file
  để định vị bối cảnh trước khi bắt đầu đọc.

#### Bổ sung *(các mục mới)*

* **Tìm kiếm (`/`)** — kiểu `less`/Vim. Có Outline nhưng không có cách nhảy tới một cụm từ cụ thể
  là một lỗ hổng điều hướng lớn.
* **Nhảy vị trí** — `g` về đầu, `G` về cuối, `<số>%` nhảy theo phần trăm.
* **Bookmark** — đánh dấu vị trí trong paper dài (`m` để đặt, `'` để nhảy tới). Hữu ích hơn cả
  auto-save khi làm việc với tài liệu 30 trang.
* **Review Mode** — phím hiện lại **nguyên đoạn văn vừa đọc** ở dạng full-text tĩnh. Đây là đối
  trọng trực tiếp với việc RSVP triệt tiêu regression: người đọc lấy lại được khả năng "liếc
  ngược" theo cách có kiểm soát.
* **Định nghĩa "Đoạn văn"** cho phím `[` / `]` phải được xác định riêng cho từng parser — ranh giới
  paragraph trong PDF hay LaTeX không hiển nhiên như trong Markdown (xem Chương 6).

---

### 3.4. Quy chuẩn TUI và Khả năng Tiếp Cận (W3C WCAG trong Terminal)

* **Độ tương phản cao (Contrast Ratio):** Tối thiểu **7:1** theo WCAG AAA (SC 1.4.6). Cho phép đổi
  theme để phù hợp với người mù màu hoặc nhạy cảm ánh sáng.
* **Keyboard-driven:** Mọi tính năng điều khiển được 100% bằng phím.
* **Duy trì vị trí cố định (Spatial Consistency):** Khung RSVP cố định tuyệt đối, không co giãn hay
  nhảy vị trí khi độ dài từ thay đổi. (Cách hiện thực: Chương 3.1.3.)

#### 3.4.1. Không được dùng màu làm kênh thông tin duy nhất *(sửa đổi quan trọng)*

**WCAG SC 1.4.1 (Use of Color):** màu không được là phương tiện *duy nhất* truyền tải thông tin.

Bản đề xuất trước viết ORP được đánh dấu "bằng màu sắc **hoặc** ký tự đặc biệt". Điều này vi phạm
tiêu chuẩn: với người mù màu deuteranopia, chữ cái ORP màu đỏ trên nền tối gần như biến mất hoàn
toàn — và ORP biến mất thì toàn bộ công cụ mất tác dụng.

→ Sửa thành **"và"**: ORP phải được đánh dấu bằng **màu nhấn + dấu hiệu hình học** (cặp mũi tên
`▼ ▲`, hoặc gạch chân/đậm). Mockup ở Chương 5 vốn đã làm đúng điều này.

#### 3.4.2. Ngưỡng nhấp nháy — WCAG SC 2.3.1 *(mục mới)*

Chữ thay đổi ở tốc độ cao là một dạng kích thích nhấp nháy. WCAG 2.3.1 (Three Flashes) tồn tại để
bảo vệ người nhạy cảm ánh sáng và người có nguy cơ động kinh.

→ Đặt **WPM mặc định ở mức an toàn (300)**, hiển thị cảnh báo một lần khi người dùng vượt ngưỡng
cao (ví dụ 700 WPM), và ghi rõ trong README. Không đặt WPM cao làm mặc định vì lý do marketing.

#### 3.4.3. Trình đọc màn hình (Screen Reader) *(mục mới)*

TUI chạy chữ và screen reader xung đột trực tiếp: screen reader không thể theo kịp, và nội dung
thay đổi liên tục sẽ khiến nó đọc lặp hoặc im lặng.

→ Cung cấp cờ **`--dump`**: xuất token stream ra stdout dạng plain text đã được làm sạch
(bỏ markup, giữ cấu trúc đoạn). Đây vừa là công cụ debug parser, vừa là **đường thoát a11y thật sự**
cho người dùng screen reader.

#### 3.4.4. Khả năng của Terminal *(mục mới)*

Không thể giả định terminal nào cũng như nhau:

* Tôn trọng biến môi trường **`NO_COLOR`**.
* Phát hiện năng lực màu qua `COLORTERM` / `TERM`: truecolor → 256 màu → 16 màu. Theme
  "solarized" phải có bản dự phòng cho terminal 16 màu.
* Phát hiện hỗ trợ Unicode; nếu terminal không vẽ được `▼ ▲` thì fallback sang `v ^`.
* **Kích thước tối thiểu:** định nghĩa ngưỡng (ví dụ 80×24). Dưới ngưỡng → tự chuyển sang `focus`
  mode 1 panel; dưới nữa → hiện thông báo yêu cầu phóng to thay vì vẽ giao diện vỡ.
* Xử lý sự kiện **resize**: layout phải tái tính, và cột neo ORP phải được tính lại.

---

## 4. Kiến trúc kỹ thuật *(chương mới)*

Chương này trả lời câu hỏi "làm thế nào", bổ khuyết cho các chương thiết kế ở trên.

### 4.1. Luồng dữ liệu

```text
File / stdin / URL
      │
      ▼
[ Đọc & phát hiện encoding ]   ← encoding_rs (BOM, UTF-16, Latin-1)
      │
      ▼
[ Parser theo định dạng ]      ← pulldown-cmark / lopdf / epub ...
      │  (AST → flatten)
      ▼
[ Document ]                    { Vec<Block>, Vec<Heading>, metadata }
      │
      ▼
[ Segmenter ]                  ← unicode-segmentation + normalization
      │
      ▼
[ Vec<Token> ]                  { text, orp_index, display_width, orp_offset,
      │                           weight, pause_ms, kind, block_id, byte_span }
      ▼
[ Player (state machine) ]      con trỏ + đồng hồ + WPM
      │
      ▼
[ Renderer TUI ]               ← ratatui
```

`Token` giữ `byte_span` trỏ ngược về văn bản gốc — đây là thứ cho phép Bảng Toàn Văn highlight
đúng từ đang đọc và cho phép Review Mode tái dựng nguyên đoạn.

> **Đính chính (ADR 001).** Bản đề xuất trước ghi `Token` có field `duration_ms`. Khi hiện thực
> OXD-010/OXD-018 mới thấy đây là thiết kế sai: WPM thay đổi *trong lúc đọc*, nên duration cố định
> buộc phải re-pace toàn bộ tài liệu mỗi lần bấm phím. Token giờ giữ `weight` (hệ số không phụ
> thuộc WPM) và `pause_ms` (khoảng nghỉ cấu trúc, không co giãn theo tốc độ); duration được suy ra
> lúc hiển thị qua `Token::duration_ms(wpm)`. Chi tiết và các phương án đã loại:
> [`docs/decisions/token-timing.md`](../decisions/token-timing.md).
>
> `Token` cũng có thêm `orp_offset` (độ rộng cột của phần trước ORP) để renderer không phải tính
> lại mỗi frame.

### 4.2. Vòng lặp sự kiện và đồng hồ

**Vấn đề:** cách hiện thực ngây thơ `sleep(60_000 / wpm)` sẽ tích lũy sai số — mỗi lần lặp cộng
thêm thời gian render và thời gian OS scheduler trả về muộn. Sau 20 phút đọc, sai lệch có thể lên
tới hàng chục giây, và quan trọng hơn: **nhịp bị giật không đều**, phá hỏng cảm giác đọc.

**Giải pháp — lập lịch theo deadline tuyệt đối:**

```rust
// Sai: next_word_at = Instant::now() + duration;   // trôi dần
// Đúng:
next_word_at += duration;                            // neo vào mốc trước đó
let timeout = next_word_at.saturating_duration_since(Instant::now());
```

**Kiến trúc vòng lặp:** một thread đọc input `crossterm` gửi qua `mpsc::channel`; luồng chính
`recv_timeout(timeout)` — hết timeout thì tiến token, có sự kiện thì xử lý phím rồi tính lại
deadline. Cách này giữ input phản hồi tức thì mà không cần `tokio`.

> Với phạm vi hiện tại, **thread + channel là đủ**. Chỉ cần cân nhắc `tokio` nếu sau này thêm tải
> file qua mạng (URL/arXiv).

**Render:** chỉ vẽ lại khi state đổi (token mới, phím, resize) — không vẽ ở tần số cố định, tránh
đốt CPU vô ích trên máy laptop.

### 4.3. Encoding và file lớn

* **Encoding:** không giả định UTF-8. Phát hiện BOM, hỗ trợ UTF-16 và Latin-1 (RFC/manpage cũ) qua
  `encoding_rs`. File hỏng phải báo lỗi rõ ràng, không panic.
* **File lớn:** EPUB/PDF vài nghìn trang mà parse toàn bộ vào `Vec<Token>` ngay khi mở sẽ cho thời
  gian khởi động tệ và ngốn RAM.
  → **Parse theo chương/khối, lazy**: dựng ngay cây Heading (rẻ) để hiện Outline, chỉ tokenize
  block khi sắp đọc tới. Giữ cửa sổ trượt các block quanh vị trí hiện tại.
* **Cache:** lưu kết quả parse của file lớn vào `~/.cache/oxidiris/`, invalidate theo `mtime` + kích
  thước file.

### 4.4. Xử lý lỗi

Dùng `anyhow` ở tầng binary, `thiserror` ở tầng core. Các trường hợp phải xử lý tử tế (không panic):
file không tồn tại, không có quyền đọc, định dạng không hỗ trợ, PDF được mã hóa, file rỗng,
terminal không phải TTY (khi đó nên tự chuyển sang `--dump`).

---

## 5. Đề xuất giao diện TUI

```text
┌─────── FOCUS HERE ─────────┐┌────────────── README.md ──────────────┐
│             ▼              ││ Oxidiris là một công cụ viết bằng     │
│         Oxidiris           ││ Rust hỗ trợ đọc nhanh tài liệu dạng   │
│             ▲              ││ TUI bằng kỹ thuật RSVP.               │
│                            ││                                       │
├────────────────────────────┤│ ## Tính năng chính                    │
│ speed: 450 WPM (eff. 361)  ││ - Tốc độ cực hạn (Blazing fast)       │
│ word: 45/999 (42%)         ││ - >> Không cần di chuyển mắt <<       │
│ [Space] stop  [?] help     ││ - Hỗ trợ định dạng Markdown           │
└────────────────────────────┘└───────────────────────────────────────┘

# Chữ 'i' trong "Oxidiris" là ORP: màu nhấn + kẹp giữa ▼ ▲
# Bảng trái : Đọc siêu tốc, cột neo đứng yên tuyệt đối
# Bảng phải : Hiển thị như nano, tự cuộn, highlight từ đang đọc
```

**Ghi chú hiện thực:**

* Cột neo ORP nằm giữa panel trái và **không đổi** khi từ dài ngắn khác nhau (Chương 3.1.3).
* Panel phải highlight token hiện tại dựa trên `byte_span` — đây là đầu mối bối cảnh quan trọng
  giúp người đọc không bị lạc.
* Dòng trạng thái hiển thị `word: 45/999` chính xác hơn `Line: 45/999`, vì đơn vị con trỏ RSVP là
  từ chứ không phải dòng.
* Khi terminal < 80 cột: bỏ panel phải, tự chuyển `focus` mode (Chương 3.4.4).

---

## 6. Hệ thống Lệnh đầu vào (CLI Flags & Options)

Dùng `clap` (derive API) để bắt tham số.

| Flag | Mô tả |
|---|---|
| `oxidiris <file>` | Mở file bài báo hoặc README |
| `oxidiris -` | **(mới)** Đọc từ stdin: `cat paper.md \| oxidiris -` |
| `-w, --wpm <n>` | Tốc độ đọc khởi đầu. Mặc định: **300** |
| `-m, --mode <tui\|focus>` | `tui`: chia đôi màn hình. `focus`: chỉ khung RSVP giữa màn hình |
| `--pacing <natural\|linear>` | **(sửa)** Thay cho `--no-delay`. `linear` = tốc độ đều tuyệt đối |
| `--theme <dark\|light\|solarized>` | **(sửa lỗi cú pháp)** trước đây ghi thiếu một dấu gạch |
| `--dump` | **(mới)** Xuất plain text ra stdout, không mở TUI. Dùng cho screen reader / pipe |
| `--chunk <n>` | **(mới)** Số từ hiển thị mỗi lần. Mặc định 1 |
| `--start <n%\|word:n>` | **(mới)** Bắt đầu từ vị trí chỉ định |
| `--no-resume` | **(mới)** Bỏ qua vị trí đã lưu, đọc lại từ đầu |
| `--config <path>` | **(mới)** Chỉ định file cấu hình khác |

### 6.1. File cấu hình *(mục mới)*

CLI flags không đủ — người dùng không muốn gõ lại `--wpm 420 --theme solarized` mỗi lần.

Đường dẫn: `~/.config/oxidiris/config.toml` (theo XDG Base Directory, dùng crate `directories`).

```toml
wpm = 420
theme = "solarized"
mode = "tui"
pacing = "natural"
chunk = 1

[keybindings]          # cho phép người dùng ánh xạ lại phím
pause = ["Space"]
faster = ["k", "Up", "+"]
slower = ["j", "Down", "-"]
```

**Thứ tự ưu tiên:** CLI flag > biến môi trường > config file > mặc định.

---

## 7. Hệ thống Phím tắt (TUI Hotkeys)

Bố trí quanh cụm phím điều hướng quen thuộc (Vim-style HJKL hoặc phím mũi tên) để không phải rời
tay khỏi vị trí gõ.

### 7.1. Phím tắt là *modal* — điểm cần làm rõ

Bản đề xuất trước có xung đột: `J`/`K` được gán cho WPM, nhưng `Tab` chuyển focus sang Bảng Toàn
Văn — lúc đó `J`/`K` phải là cuộn văn bản. Cần khai báo rõ hệ phím thay đổi theo panel đang focus.

**Ba chế độ:** `Reader` (mặc định) · `Browser` (focus panel toàn văn) · `Outline` (focus sidebar TOC)

### 7.2. Chế độ Reader

**🎛️ Điều khiển Luồng đọc**

| Phím | Hành động |
|---|---|
| `Space` | Tạm dừng / Tiếp tục (Play/Pause) — phím quan trọng nhất |
| `R` | Đọc lại từ đầu tài liệu |

**⏱️ Điều chỉnh Tốc độ**

| Phím | Hành động |
|---|---|
| `K` / `↑` | Tăng 25 WPM |
| `J` / `↓` | Giảm 25 WPM |
| `+` / `-` | **(mới)** Tinh chỉnh ±5 WPM |

**🗺️ Điều hướng**

| Phím | Hành động |
|---|---|
| `H` / `←` | Tua lại 5 từ (Backstep) |
| `L` / `→` | Tua nhanh 5 từ |
| `[` / `]` | Nhảy Đoạn văn trước / sau |
| `g` / `G` | **(mới)** Về đầu / cuối tài liệu |
| `<n>%` | **(mới)** Nhảy tới phần trăm vị trí |
| `/` | **(mới)** Tìm kiếm; `n` / `N` để tới kết quả kế tiếp / trước đó |
| `m` / `'` | **(mới)** Đặt bookmark / nhảy tới bookmark |

**👁️ Bối cảnh**

| Phím | Hành động |
|---|---|
| `Tab` | Chuyển focus sang Bảng Toàn Văn (→ chế độ Browser) |
| `o` | **(mới)** Mở/đóng sidebar Outline (→ chế độ Outline) |
| `v` | **(mới)** Review Mode — hiện nguyên đoạn vừa đọc dạng tĩnh |

**🚪 Hệ thống**

| Phím | Hành động |
|---|---|
| `?` | Bảng pop-up hướng dẫn phím tắt |
| `Esc` | Thoát chế độ phụ, quay về Reader |
| `Q` / `q` | Thoát chương trình |

> **Lưu ý:** Bản trước gán `Esc` cho cả "thoát chương trình". Nên tách: `Esc` = thoát chế độ phụ,
> `q` = thoát hẳn. Đây là quy ước người dùng TUI mong đợi.

### 7.3. Chế độ Browser / Outline

| Phím | Hành động |
|---|---|
| `J` / `K` | Cuộn xuống / lên (**không** đổi WPM ở chế độ này) |
| `Enter` | Nhảy con trỏ RSVP tới vị trí đang chọn, quay về Reader |
| `Tab` / `Esc` | Quay về Reader |

### 7.4. Auto-Save vị trí

Khi người dùng nhấn `Q` để thoát một bài báo dài 30 trang, Oxidiris tự động lưu vị trí hiện tại vào
`~/.cache/oxidiris/history.json`. Lần sau mở lại file đó, TUI hỏi: *"Tiếp tục đọc từ đoạn cũ?"*

**Chi tiết cần chốt:**

* Khóa nhận dạng file: **hash nội dung**, không phải đường dẫn — để file được đổi tên/di chuyển vẫn
  nhận ra, và file bị sửa thì vị trí cũ bị vô hiệu đúng cách.
* Lưu kèm bookmark của file đó.
* Giới hạn số bản ghi (ví dụ 500 file gần nhất) để cache không phình vô hạn.
* Cờ `--no-resume` để bỏ qua.

---

## 8. Engine bóc tách văn bản (Parser)

Oxidiris không thể xử lý văn bản như chuỗi thô. Nó cần bộ parser phân loại nội dung thành token đọc
được và các phần tử cần bỏ qua/hiển thị đặc biệt (công thức toán, code block, bảng biểu).

### 8.1. Nhóm Markdown & Documentation (`.md`, `.markdown`, `.mdx`, `.org`)

* **Headings (`#`, `##`, `###`)**: bóc tách để tạo cây mục lục (TOC) và điều hướng đoạn.
* **Code Blocks** (inline `` `code` `` & fenced ```` ```lang ````): RSVP không đọc code từng từ
  được. Parser nhận diện block code để tự động hạ WPM, tạm dừng, hoặc hiển thị nguyên khối.
* **Links & Images**: chỉ lấy phần text hiển thị, loại bỏ URL để không làm gián đoạn nhịp đọc.
* **Lists & Blockquotes**: bóc tách dấu đầu dòng để tạo khoảng nghỉ.
* **(mới) Bảng biểu**: bảng Markdown đọc tuần tự sẽ vô nghĩa. Nên hiển thị nguyên khối trong panel
  phải và bỏ qua ở luồng RSVP, hoặc đọc theo hàng kèm tên cột.

### 8.2. Nhóm Học thuật (`.tex`, `.latex`, `.typ`, `.pdf`)

**LaTeX / Typst:**

* **Math Formulas** (`$...$`, `$$...$$`, `\begin{equation}`): chuyển cú pháp toán thành dạng đọc
  được, hoặc hiển thị khối riêng thay vì chạy từng ký tự như `\alpha`, `\frac`.
* **Citations & References** (`\cite{...}`, `\ref{...}`): bỏ qua mã định danh nội bộ, thay bằng số
  hoặc cụm ngắn để không làm rác nhãn thị giác.

**PDF:** định dạng phổ biến nhất của paper khoa học nhưng khó xử lý nhất — bố cục 2 cột,
header/footer, footnote, số trang. Parser phải bóc luồng văn bản theo đúng **reading order** của
cột thay vì quét ngang toàn trang.

> **Cảnh báo phạm vi:** de-columnizing PDF là bài toán khó và dễ ngốn hết thời gian dự án. Nên xếp
> vào v0.5 và cân nhắc gọi công cụ ngoài (`pdftotext -layout` của Poppler) làm bước đệm trước khi
> tự viết.

### 8.3. Nhóm Sách điện tử (`.epub`)

EPUB là các file XHTML đóng gói ZIP. Parser bóc sạch thẻ HTML thừa (`<div>`, `<p>`, `<span>`, CSS)
và chỉ giữ luồng văn bản thuần kèm cấu trúc chương hồi.

### 8.4. Nhóm Web & HTML (`.html`, `.htm`, Web Articles)

Bóc tách qua thuật toán **Readability**: gạt bỏ navbar, sidebar quảng cáo, footer; chỉ trích xuất
vùng `<article>` hoặc `<main>`.

### 8.5. Nhóm Văn bản thô (`.txt`, `.rst`, `.asciidoc`)

Manpage, RFC, file `.txt` thuần. Xử lý ngắt dòng thông minh (soft wrap), nhận diện đoạn văn dựa
trên khoảng trắng kép.

> **Lưu ý riêng cho RFC/manpage:** các file này thường ngắt dòng cứng ở cột 72 và dùng khoảng trắng
> để canh lề. Cần nối lại dòng (unwrap) trước khi tokenize, nếu không mỗi dòng sẽ bị hiểu nhầm là
> một đoạn.

### 8.6. Tokenizer đa ngôn ngữ *(mục mới — quan trọng)*

Bảng ưu tiên bên dưới ghi "`unicode-segmentation` — hỗ trợ cả tiếng Việt có dấu, CJK". Thực tế
phức tạp hơn:

**Tiếng Việt** là ngôn ngữ đa âm tiết viết rời. "nghiên cứu" là **một** từ nhưng **hai** token khi
tách theo khoảng trắng. Đọc RSVP từng âm tiết rời rạc khó hiểu hơn hẳn so với tiếng Anh, vì mỗi âm
tiết đơn lẻ thường không mang nghĩa trọn vẹn.

→ Cân nhắc mặc định **`--chunk 2`** cho văn bản tiếng Việt, hoặc tích hợp từ điển ghép từ đơn giản.
Cần thử nghiệm thực tế trước khi chốt.

**CJK** không dùng khoảng trắng. `unicode-segmentation` sẽ trả về từng ký tự riêng lẻ, tạo ra một
luồng RSVP vô dụng.

→ Hoặc gom theo cụm 2–4 ký tự bằng heuristic, hoặc **tuyên bố thẳng là chưa hỗ trợ ở v1.0**. Điều
tệ nhất là ngầm hỗ trợ nửa vời.

**RTL (Ả Rập, Do Thái):** hướng đọc ngược khiến khái niệm "30% từ trái sang" đảo chiều, và thuật
toán bidi phức tạp. → Ngoài phạm vi v1.0, ghi rõ trong tài liệu.

**Chunking mode (`--chunk n`):** hiển thị 2–3 từ mỗi lần thay vì 1. Đây là tính năng chuẩn của hầu
hết công cụ RSVP và đặc biệt cần cho tiếng Việt. Khi `chunk > 1`, ORP tính trên **toàn cụm** như
một đơn vị.

### 8.7. Bảng tổng hợp độ ưu tiên triển khai

| Nhóm định dạng | Ưu tiên | Crate Rust đề xuất | Lưu ý xử lý đặc thù |
|---|---|---|---|
| Markdown / MDX | 1 (Core) | `pulldown-cmark` hoặc `comrak` | Flatten AST thành luồng token, giữ heading metadata |
| Plain Text / TXT | 1 (Core) | stdlib + `unicode-segmentation` + `unicode-normalization` + `unicode-width` | Chuẩn NFC, đếm grapheme, canh theo cột hiển thị |
| LaTeX / Typst | 2 | `typst-syntax`, hoặc parser riêng cho LaTeX | Nhận diện môi trường toán (`align`, `equation`) để không phân rã thành từ vô nghĩa |
| PDF (paper 2 cột) | 3 | `lopdf` / `pdf-extract`, hoặc gọi `pdftotext -layout` | Xử lý phân cột để không đọc lẫn cột trái sang cột phải |
| EPUB / HTML | 4 | `epub` crate / `readability` | Lột bỏ thẻ HTML, chỉ lấy semantic text stream |

**Crate hạ tầng bổ sung:** `clap` (CLI) · `crossterm` (backend terminal) · `encoding_rs` (encoding)
· `serde` + `toml` + `serde_json` (config/cache) · `directories` (đường dẫn XDG) ·
`anyhow` + `thiserror` (lỗi).

---

## 9. Chất lượng & Hạ tầng dự án *(chương mới)*

Phần này trước đây chưa được đề cập, nhưng quyết định dự án có publish được hay không.

### 9.1. Chiến lược kiểm thử

| Loại | Công cụ | Đối tượng |
|---|---|---|
| Unit test | stdlib | **Thuật toán ORP** (ưu tiên số 1 — dễ sai, dễ test), pacing, tokenizer |
| Golden/corpus test | `testdata/` + `insta` | Parser: file mẫu `.md`/`.tex`/`.txt` → token stream kỳ vọng |
| Snapshot TUI | `ratatui::backend::TestBackend` + `insta` | Layout không vỡ khi resize, khi từ dài/ngắn |
| Property test | `proptest` | Bất biến: *cột neo ORP luôn cố định với mọi chuỗi Unicode đầu vào* |
| Fuzz | `cargo-fuzz` | Parser không panic với input rác |

**Bộ test bắt buộc cho ORP** phải bao gồm: từ 1 ký tự, từ 20 ký tự, tiếng Việt NFC, tiếng Việt NFD,
CJK full-width, emoji ghép ZWJ, chuỗi có ký tự điều khiển.

### 9.2. Benchmark

`criterion` cho tokenizer và parser. Ngưỡng mục tiêu: mở file Markdown 1 MB dưới 100 ms.

### 9.3. CI/CD

GitHub Actions:

* `cargo fmt --check`
* `cargo clippy -- -D warnings`
* `cargo test` trên **Linux, macOS, Windows** (crossterm trên Windows Terminal có khác biệt về
  key event — đặc biệt là sự kiện key release)
* `cargo build -p oxidiris-core --target wasm32-unknown-unknown` (giữ ràng buộc Chương 1.1)
* `cargo deny check` (license + advisory)
* Release binary đa nền tảng qua `cargo-dist`

### 9.4. MSRV

`edition = "2024"` yêu cầu Rust **≥ 1.85**. Cần khai báo `rust-version` trong `Cargo.toml`, ghi rõ
trong README, và pin một job CI ở đúng phiên bản MSRV để không vô tình phá vỡ.

### 9.5. Đóng gói & Phân phối

crates.io · Homebrew tap · AUR · Nix · `cargo-binstall`.

### 9.6. Tài liệu

* **README**: hiện chỉ có một dòng (và dư một dấu `"` ở cuối). Cần bổ sung — **demo GIF/ảnh động**
  (dùng `vhs` hoặc `asciinema`; với công cụ RSVP thì demo động là thứ thuyết phục nhất), badge CI,
  hướng dẫn cài đặt, bảng phím tắt, ghi chú a11y, và tuyên bố trung thực ở Chương 0.2.
* `CONTRIBUTING.md`, `CHANGELOG.md` (theo Keep a Changelog), man page, `--help` có ví dụ.
* Rustdoc cho `oxidiris-core` publish lên docs.rs.

### 9.7. Quyền riêng tư

Oxidiris đọc tài liệu cá nhân của người dùng. Cần tuyên bố rõ trong README: **không telemetry,
không gửi dữ liệu ra ngoài**; cache và history chỉ nằm local trong `~/.cache/oxidiris/`.

---

## 10. Ba việc nên làm trước tiên

1. **Tách workspace** thành `oxidiris-core` + `oxidiris` (Chương 1.1) — chi phí gần bằng 0 ngay bây
   giờ, đắt về sau.
2. **Chốt spec ORP** dưới dạng bảng tra + bộ test case cụ thể (Chương 3.1) — tính theo *cột hiển
   thị*, có ca kiểm thử tiếng Việt và CJK.
3. **Dựng khung event loop + timing** theo mô hình deadline (Chương 4.2), trước khi viết bất kỳ
   widget nào.

---

## Phụ lục A — Tài liệu tham khảo

[1] [https://wcag.ie](https://wcag.ie/assistive-tech-accessibility/)
[2] [https://www.w3.org](https://www.w3.org/TR/WCAG20/)
[3] [https://www.ijdesign.org](https://www.ijdesign.org/index.php/IJDesign/article/view/36/8)
[4] [https://findanexpert.unimelb.edu.au](https://findanexpert.unimelb.edu.au/scholarlywork/2110825-exploring-design-parameters-for-rsvp-reading-of-mobile-notifications)
[5] [https://a1slides.com](https://a1slides.com/accessible-powerpoint-presentations-wcag-guide/)
[6] [https://dl.acm.org](https://dl.acm.org/doi/10.1145/3726986.3727002)
[7] [https://www.w3.org](https://www.w3.org/WAI/people-use-web/tools-techniques/presentation/)
[8] [https://www.digitalaccesstraining.com](https://www.digitalaccesstraining.com/pages/articles?p=five-common-assistive-technologies-and-how-to-design-for-them)
[9] [https://www.w3.org](https://www.w3.org/TR/WCAG22/)
[10] [https://www.w3.org](https://www.w3.org/WAI/WCAG21/Understanding/visual-presentation.html)
[11] [https://lobehub.com](https://lobehub.com/skills/neversight-learn-skills.dev-tui-design)
[12] [https://ixdf.org](https://ixdf.org/literature/topics/assistive-technology)

**Về chuyển động mắt và đọc**

* Rayner, K. (1998). *Eye movements in reading and information processing: 20 years of research.*
  Psychological Bulletin, 124(3). — Công trình nền tảng về saccade, fixation và regression.
* Rayner, K., Schotter, E. R., Masson, M. E. J., Potter, M. C., & Treiman, R. (2016).
  *So Much to Read, So Little Time: How Do We Read, and Can Speed Reading Help?*
  Psychological Science in the Public Interest, 17(1). — **Nguồn quan trọng nhất cho Chương 0.2**:
  phân tích phê phán về RSVP và đánh đổi tốc độ–khả năng hiểu.

**Tiêu chuẩn tiếp cận**

* W3C — *Web Content Accessibility Guidelines (WCAG) 2.2*.
  Đặc biệt: SC 1.4.1 (Use of Color), SC 1.4.6 (Contrast Enhanced, AAA), SC 2.3.1 (Three Flashes).

**Tiêu chuẩn Unicode**

* UAX #29 — *Unicode Text Segmentation* (ranh giới grapheme và từ).
* UAX #11 — *East Asian Width* (chiều rộng cột hiển thị).
* UAX #15 — *Unicode Normalization Forms* (NFC/NFD).

**Tham chiếu sản phẩm**

* Spritz — công cụ thương mại phổ biến hóa khái niệm ORP và "quy tắc 1/3".

*(Cần bổ sung: nguồn cụ thể cho các con số delay dấu câu 2–2.5× và bảng tra ORP ở Chương 3.1.2.)*
# Cam kết quyền riêng tư và Bảo mật (Privacy & Security Commitment)

> **Tóm tắt cốt lõi**: Rustype là phần mềm mã nguồn mở độc lập, **hoàn toàn ngoại tuyến (100% Offline)**, **không thu thập bất kỳ dữ liệu nào (Zero Telemetry)** và **tuyệt đối không ghi nhật ký phím gõ (Zero Keylogging)**.

Bộ gõ tiếng Việt là ứng dụng nhạy cảm bậc nhất trên hệ điều hành vì nó nhận từng phím bấm của người dùng. Rustype được thiết kế ngay từ dòng mã đầu tiên với triết lý bảo vệ tối đa quyền riêng tư của bạn.

---

## 1. Không kết nối mạng (100% Offline)
* **Không mã mạng**: Mã nguồn của Rustype (`rustype-core` và `rustype-ibus`) được viết bằng Rust thuần, không liên kết (link) với bất kỳ thư viện mạng, socket TCP/UDP, HTTP client, hay dịch vụ phân tích dữ liệu nào.
* **Không thể truy cập Internet**: Bản thân engine không có quyền hay cơ chế để gửi bất kỳ gói tin nào ra bên ngoài máy tính của bạn. Mọi quá trình xử lý phím bấm đều diễn ra cục bộ trong bộ nhớ RAM của thiết bị.

## 2. Tuyệt đối không ghi nhật ký phím gõ (Zero Keylogging)
* **Bộ đệm tạm thời**: Các phím người dùng gõ chỉ được lưu trong một vùng đệm tạm thời (buffer) của tiến trình trong quá trình hình thành một từ tiếng Việt (ví dụ: gõ `v`, `i`, `e`, `t`, `j`).
* **Hủy ngay sau khi hoàn thành**: Ngay khi từ được commit vào ứng dụng đích (hoặc khi người dùng nhấn phím cách, Enter, đổi con trỏ chuột, đổi cửa sổ), toàn bộ bộ đệm trong RAM sẽ bị xóa sạch (`clear()`).
* **Không ghi file log phím**: Rustype không bao giờ tạo bất kỳ tệp nhật ký nào ghi lại nội dung văn bản bạn đã gõ.

## 3. Bảo vệ an toàn trong ô Mật khẩu
* Khi bạn nhập mật khẩu trên trình duyệt, màn hình khóa hay các ứng dụng quản lý mật khẩu, chuẩn IBus sẽ gửi cờ thuộc tính `IBUS_INPUT_PURPOSE_PASSWORD`.
* Khi phát hiện cờ này, Rustype tự động kích hoạt chế độ **Bypass hoàn toàn**:
  - Không đọc, không phân tích, không giữ lại phím.
  - Phím được chuyển thẳng nguyên bản tới ứng dụng đích như khi không bật bộ gõ.

## 4. Chế độ gỡ lỗi an toàn (Safe Debug Mode)
* Khi bạn bật cờ `RUSTYPE_DEBUG=1` để chẩn đoán sự cố:
  - Rustype **chỉ ghi mã quét phần cứng (hardware keycode/keyval dạng hex)** và trạng thái chuyển đổi của máy trạng thái.
  - **Tuyệt đối không in ký tự hay nội dung văn bản** mà bạn gõ ra terminal hay file log.

## 5. Mã nguồn mở và Có thể kiểm chứng (Auditable)
* Toàn bộ mã nguồn của Rustype được công khai minh bạch dưới giấy phép tự do MIT.
* Bất kỳ ai cũng có thể tải mã nguồn về, tự kiểm tra mã nguồn (audit) và tự biên dịch (`cargo build --release`) mà không phụ thuộc vào bất kỳ binary đóng gói sẵn nào từ bên thứ ba.

---

*Rustype — An tâm gõ tiếng Việt trên Linux.*

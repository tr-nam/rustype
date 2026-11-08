# Cam kết quyền riêng tư và Bảo mật (Privacy & Security Commitment)

> **Tóm tắt cốt lõi**: Rustype là phần mềm mã nguồn mở. **Engine gõ không có mã mạng**, **không thu thập dữ liệu (không telemetry)** và **không ghi nhật ký phím gõ**. Ứng dụng cài đặt chỉ có một yêu cầu mạng, và chỉ khi bạn bấm "Kiểm tra cập nhật" (xem mục 1).

Bộ gõ tiếng Việt là ứng dụng nhạy cảm bậc nhất trên hệ điều hành vì nó nhận từng phím bấm của người dùng. Rustype được thiết kế với mục tiêu bảo vệ quyền riêng tư của bạn ở mức cao nhất.

---

## 1. Không kết nối mạng khi gõ
* **Không mã mạng**: Mã nguồn của Rustype (`rustype-core`, `rustype-linux` và các crate khác) không liên kết với thư viện mạng, HTTP client hay dịch vụ phân tích dữ liệu nào. `rustype-linux` chỉ dùng thư viện chuẩn của Rust và mở Unix socket cục bộ cho D-Bus.
* **Không gửi dữ liệu đi**: engine không có mã gửi gói tin ra ngoài máy của bạn. Việc xử lý phím diễn ra cục bộ trong bộ nhớ của tiến trình.
* **Ngoại lệ duy nhất: kiểm tra cập nhật**: Ứng dụng cài đặt `rustype-settings` gửi một yêu cầu HTTPS GET tới `https://api.github.com/repos/tr-nam/rustype/releases/latest`, và chỉ khi bạn bấm nút "Kiểm tra cập nhật". Yêu cầu chỉ mang header `User-Agent: RustypeSettings/<phiên bản>` cùng các header HTTP thông thường. Không có nội dung cấu hình, từ gõ hay định danh cài đặt nào được gửi đi. Phản hồi là thông tin phiên bản mới nhất để hiển thị. Như mọi kết nối HTTPS, GitHub nhận được địa chỉ IP của máy bạn.

## 2. Không ghi nhật ký phím gõ
* **Bộ đệm tạm thời**: Các phím người dùng gõ chỉ được lưu trong một vùng đệm tạm thời của tiến trình trong lúc hình thành một từ tiếng Việt (ví dụ: gõ `v`, `i`, `e`, `t`, `j`).
* **Hủy ngay sau khi hoàn thành**: Khi từ được commit vào ứng dụng đích (phím cách, Enter), hoặc khi GNOME Shell báo Reset hay FocusOut (đổi con trỏ chuột, đổi cửa sổ), engine xóa sạch bộ đệm trong bộ nhớ.
* **Engine Windows**: `rustype-windows` có low-level mouse hook. Nhấn nút chuột, đổi cửa sổ hoặc gặp phím không tạo ký tự (mũi tên, Home, End, Tab) đều xóa bộ đệm trong bộ nhớ.
* **Không ghi file log phím**: Rustype không tạo tệp nhật ký nào ghi lại nội dung văn bản bạn đã gõ. Khi chạy, `rustype-linux` chỉ ghi file địa chỉ và socket của bus.

## 3. Bảo vệ an toàn trong ô Mật khẩu
* Khi bạn nhập mật khẩu trên trình duyệt, màn hình khóa hay các ứng dụng quản lý mật khẩu, chuẩn IBus sẽ gửi cờ thuộc tính `IBUS_INPUT_PURPOSE_PASSWORD` (hoặc PIN).
* Khi phát hiện cờ này, Rustype tự động kích hoạt chế độ **bỏ qua hoàn toàn**:
  - Không đọc, không phân tích, không giữ lại phím.
  - Phím được chuyển thẳng nguyên bản tới ứng dụng đích như khi không bật bộ gõ.

## 4. Chế độ gỡ lỗi an toàn
* Khi bạn bật cờ `RUSTYPE_DEBUG=1` để chẩn đoán sự cố:
  - Rustype **chỉ ghi mã phím phần cứng (keycode/keyval dạng hex)** và trạng thái của máy trạng thái.
  - **Không in ký tự hay nội dung văn bản** mà bạn gõ ra terminal hay file log.

## 5. Mã nguồn mở và có thể kiểm chứng
* Toàn bộ mã nguồn của Rustype được công khai dưới giấy phép MIT.
* Bất kỳ ai cũng có thể tải mã nguồn về, tự kiểm tra và tự biên dịch (`cargo build --release`) mà không phụ thuộc vào bất kỳ binary đóng gói sẵn nào từ bên thứ ba.

## 6. Ô thanh địa chỉ (URL)
* Khi ô đang focus có mục đích URL, ứng dụng gửi cho engine văn bản xung quanh con trỏ trong ô đó (Surrounding Text) qua IBus. Engine so phần đuôi văn bản này với phần đã nhớ để phát hiện lệnh xóa bị Chrome bỏ qua. Việc so sánh chỉ diễn ra trong bộ nhớ.
* Engine không lưu, không ghi file và không in nội dung này ra log. Log gỡ lỗi chỉ in độ dài và vị trí.
* Sau lần đầu vào một ô URL, GNOME có thể gửi cả Surrounding Text của các ô khác, vì mọi ứng dụng dùng chung một kết nối IBus. Engine bỏ ngay các báo cáo không phải của ô URL khi nhận, không so sánh và không dùng tới.

---

*Rustype: an tâm gõ tiếng Việt.*

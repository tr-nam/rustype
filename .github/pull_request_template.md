## Mô tả thay đổi
<!-- Tóm tắt ngắn gọn nội dung PR, giải thích lý do và cách thức thay đổi -->

## Liên quan đến Issue
<!-- Điền mã số issue nếu có, ví dụ: Closes #12 -->

## Loại thay đổi
- [ ] 🐛 Sửa lỗi (Bug fix)
- [ ] ✨ Tính năng mới (New feature)
- [ ] ⚡ Tối ưu hiệu năng (Performance improvement)
- [ ] 📝 Cập nhật tài liệu (Documentation)
- [ ] 🧪 Bổ sung kiểm thử (Tests / Golden cases)

## Danh sách kiểm tra chất lượng (Checklist)
- [ ] Mã nguồn đã được định dạng chuẩn: `cargo fmt --all --check`
- [ ] Clippy không có bất kỳ cảnh báo nào: `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Toàn bộ unit tests và golden tests đều vượt qua: `cargo test --workspace`
- [ ] Không thêm phụ thuộc thư viện runtime bên ngoài vào `rustype-core`
- [ ] Đã kiểm tra thực tế trên ứng dụng đích (Terminal / Trình duyệt / Editor)

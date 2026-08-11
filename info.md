Để phân phối và vận hành hàng nghìn extension (tiện ích mở rộng) một cách mượt mà, ứng dụng gốc Tachiyomi (và các bản fork hiện tại như Mihon) chia sẻ một bộ công cụ cốt lõi bao gồm các thư viện mạng, thư viện xử lý dữ liệu và các giao diện lập trình (Interfaces/Classes) định sẵn.
Bất kỳ extension nào của Tachiyomi cũng đều được thừa hưởng và sử dụng chung các thành phần cốt lõi sau từ ứng dụng gốc:
------------------------------
## 1. Các thư viện bên thứ ba (Third-party Libraries)
Để giữ cho dung lượng file APK của extension siêu nhẹ (chỉ vài trăm KB), ứng dụng gốc tích hợp sẵn và chia sẻ các thư viện nặng ký sau qua cơ chế compileOnly:

* OkHttp (okhttp3.*): Thư viện xử lý mạng. Tất cả các extension đều dùng chung một bộ quản lý cookie, bộ đếm thời gian (timeouts), và các trình chặn (Interceptors) như CloudflareInterceptor do ứng dụng gốc cấu hình để vượt tường lửa.
* Jsoup (org.jsoup.*): Thư viện cào dữ liệu và phân tích cú pháp HTML/XML từ các trang web truyện tranh.
* Gson (com.google.gson.*) hoặc KotlinX Serialization: Dùng để phân tích cú pháp (parse) dữ liệu dạng JSON khi extension gọi API của các nguồn truyện.

## 2. Các Interface và Class cốt lõi của hệ thống nguồn (Source API)
Tachiyomi định nghĩa sẵn các "khung xương" (Interfaces) trong mã nguồn của ứng dụng gốc. Extension chỉ việc "điền vào chỗ trống" bằng cách kế thừa (extend) hoặc thực thi (implement) các lớp này:

* Source: Giao diện cơ bản nhất định nghĩa tên extension, ngôn ngữ, và ID của nguồn truyện.
* HttpSource: Lớp trừu tượng (Abstract Class) mà hầu hết các extension đều kế thừa. Nó chia sẻ sẵn các hàm xử lý HTTP Request và HTTP Response.
* CatalogueSource: Giao diện quản lý việc duyệt danh mục truyện (gồm danh sách truyện mới cập nhật, truyện phổ biến, và chức năng tìm kiếm).

## 3. Các Phương thức và Hàm chia sẻ quan trọng (Core Methods)
Thông qua lớp HttpSource và các lớp tiện ích (Utils), Tachiyomi chia sẻ các phương thức bắt buộc phải có để đọc truyện trực tuyến:

* Nhóm phương thức lấy danh sách truyện (Catalog):
* fetchPopularManga(page) / popularMangaRequest(page): Gọi mạng và trả về danh sách truyện hot.
   * fetchSearchManga(page, query, filters): Xử lý tìm kiếm truyện theo từ khóa và bộ lọc.
   * fetchLatestUpdates(page): Lấy danh sách các truyện mới cập nhật chương (chapter).
* Nhóm phương thức lấy chi tiết truyện & chương:
* fetchMangaDetails(manga): Lấy thông tin mô tả, tác giả, trạng thái, ảnh bìa của truyện.
   * fetchChapterList(manga): Lấy toàn bộ danh sách các chương của bộ truyện đó.
* Nhóm phương thức lấy nội dung (Trang ảnh):
* fetchPageList(chapter): Phương thức quan trọng nhất để lấy ra danh sách các URL hình ảnh của một chương truyện, phục vụ cho trình đọc (Reader) của ứng dụng gốc.
* Nhóm phương thức bổ trợ (Helper Methods):
* client: Trả về đối tượng OkHttpClient của ứng dụng gốc để extension thực hiện các cuộc gọi mạng độc lập.
   * headersBuilder(): Phương thức chia sẻ cấu hình User-Agent hoặc các thông tin bảo mật tiêu chuẩn để giả lập trình duyệt, tránh bị chặn.

------------------------------
## Quy trình phối hợp thực tế
Khi bạn bấm vào một chương truyện trên ứng dụng:

   1. Ứng dụng gốc gọi phương thức fetchPageList(chapter) của Extension.
   2. Extension sử dụng client (OkHttp chung) để tải trang HTML về.
   3. Extension dùng Jsoup (được chia sẻ từ app gốc) để bóc tách các thẻ <img> nhằm lấy URL ảnh.
   4. Extension đóng gói danh sách URL này vào một đối tượng dữ liệu (List<Page>) rồi trả về cho ứng dụng gốc.
   5. Ứng dụng gốc nhận danh sách URL và tự thực hiện việc tải, hiển thị ảnh lên màn hình cho bạn đọc.

Nếu bạn đang tìm hiểu cấu trúc này để tự viết một Extension cho Tachiyomi/Mihon, hoặc muốn tự xây dựng một kiến trúc App-Plugin tương tự, hãy cho tôi biết:

* Bạn cần tôi hướng dẫn cách khai báo các thư viện này trong file build.gradle của extension?
* Hay bạn muốn xem một đoạn mã nguồn mẫu kết hợp giữa HttpSource, OkHttp và Jsoup theo chuẩn Tachiyomi?



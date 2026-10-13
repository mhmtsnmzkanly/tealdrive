# Çekirdek geliştirme: yeni aşamalar

Kullanıcının son kapsamı: hosting yönetimi eklenmez. Mevcut veri ve aktif servis korunur; değişiklikler ayrı commit ve kabul testleriyle yayınlanır.

## Birinci aşama

- [ ] UI/oturum: cookie bridge başarısızlığı, yeniden bağlantı, anlaşılır hata, aktarım ilerleme/iptal.
- [ ] Büyük dosya aktarımı: sınırlandırılmış bellek, parçalı helper yazımı, kota/iptal/temizlik; devam desteği açıkça belgelenir.
- [ ] Sanal genel panel admin: servis UID ayrı kalır; korumalı yetki kapsamı hesabın Linux/virtual türünden ayrılır. Sanal home ve başlangıç web parolası. Eski admin geçiş doğrulanmadan kaldırılmaz.
- [ ] Backend TLS: güvenilir sertifika yenileme ve proxy trust güncelleme; HTTP fallback yok.
- [ ] Ek uygulama şifrelemesi: tehdit modeli, anahtar güveni, sürümlü handshake, standart protokol/kütüphane, downgrade/replay testleri. TLS yerine geçmez; XSS veya endpoint compromise koruması iddiası yok.

## İkinci aşama

- [ ] Root Wizard ortak imaj init/status/check/expand/sync; aktif imajı ezme ve otomatik shrink yok.
- [ ] Net dağıtılabilir havuz ile gerçek ext4 kapasitesi/metadata işletim payı ayrılır. Fiziksel alan önceden ayrılır, kapasite doğrulanır.
- [ ] Atomik tahsis; kullanım ile tahsis ayrılır. Admin kendi dosyaları da sanal havuzdan düşer.
- [ ] Sistem DB/güvenlik/control audit bütçesi kullanıcı havuzundan ayrı, bounded. Kullanıcı geçmişi/trash kendi kotasından.
- [ ] Mount ve crash recovery; dış servislerin aynı dosya alanını erişim izinleriyle kullanması belgelenir, hosting provision yapılmaz.

## Kararlar ve ilerleme

- Başlangıçta kök diskte 96 GiB boş alan var. Debug sembolleri ve incremental build kapatılarak derleme alanı azaltılır. Her büyük test/release öncesinde df kontrol edilir; kullanıcı dosyaları, audit ve aktif binaries silinmez.
- SDK cookie bind veya session confirmation hatasında doğrulanmış WS kapatılmalıdır; aksi halde ikinci AuthLogin dosya komutu dispatcherına gider ve NotImplemented döner. Bu kod kusuru doğrulandı; cookie kaybının tarayıcıdaki özgül nedeni henüz doğrulanmadı.
- Büyük aktarım yalnız limit yükseltme değildir: mevcut commit tüm dosyayı Vec içine okuyup helper IPC üzerinden gönderiyor. Güven sınırları korunarak protokol genişletilmelidir.
- Ek şifreleme yaklaşımı tasarımla seçilecek; rastgele sunucu anahtarını güvenilir kabul eden özel şema uygulanmayacak.

### Oturum köprüsü düzeltmesi

SDK bind HTTP reddi, ağ hatası ve bind başarılı/session401 durumlarında kimliği temizler ve WS kapatır. Tekrar giriş yeni handshake gerektirir. Üç regression senaryosu mevcut SDK testine eklendi. `cargo test --locked --test sdk_client` passed (incremental/debug sembolleri kapalı). Canlı release henüz değiştirilmedi. Test sonrasında kök disk boş alanı95GiB.

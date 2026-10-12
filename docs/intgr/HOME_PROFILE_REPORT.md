# Otomatik home geliştirme raporu

## Tamamlanan bölüm

Root config'inde açık `home_template` seçimiyle hesap home'u otomatik kullanılır. OS/NSS doğrulaması helper account gateway'de kalır. Server istemcinin gönderdiği home/UID'yi kullanmaz. Template yalnız web-account modunda ve `roots=[]` ile geçerlidir; eski statik profili sessiz genişletmek reddedilir.

HTTP login/session/download/preview, WebSocket dosya dispatch ve privileged helper aynı resolver'ı kullanır. Helper bağımsız Root template'ini materialize edip gelen snapshot'ı tavanla sınırlar. Read-only/upload/hidden/webroot/quota ve Root/access/sensitive kuralları korunur. Sanal allocation eksikse0; Root tavanı varsa daha düşük değer uygulanır.

Sistem dizinleri/application state erişim kökü olamaz. Doğrulanmış sanal hesap için `/home/tealdrive/virtual-homes/<hesap>` istisnadır; OS kimliği oluşturma bu aşamada eklenmez. `.ssh` ortak görünmezlik kontrolüne tabidir. `.env` legacy deny/warn pattern'lerinden otomatik çıkarılmaz; yeni config örneğinde Root açık seçimle izin verir.

## Test kanıtı

- 550 unit passed,1 opt-in PAM ignored; normal integration tests passed.
- Altı yeni home testi: identity-derived path/constraints, legacy migration reddi, sistem/inaktif hesap reddi, virtual budget, otomatik mode validation/serde/PATCH ve cross-account snapshot reddi.
- Gerçek Chromium/public SDK/HTTP/setuid helper/üç geçici Linux identity ile dynamic home okuma, .env okuma, .ssh metadata ve read NOT_FOUND, olmayan dosyayla aynı kod, HTTP download ve cross-home reddi geçti.
- Admin onay/suspend/enable/PAM registration/UI kabulü otomatik home modunda tekrar geçti; eski cookie/grant/socket davranışı korundu.
- Root config API üzerinden değişmedi, audit actor/target içeriyor/credential içermiyor, geçici identities/grants/home temizlendi.
- Clippy -D warnings,fmt,diff,Rust1.85.0 all-target check geçti.
- Aday legacy config ile normal UI SDK auth smoke geçti.

Native ilk denemede .env legacy warn_read pattern'i nedeniyle erişim reddedildi; fixture açık migration ile iki pattern listesini düzenledi. Production kuralları gevşetilmedi.

## Sunucu durumu

Aday 127.0.0.1:13101 güncel server/helper binary ile yeniden başlatıldı; mevcut statik config korunur. Native acceptance server13102 kapatıldı. Önceki aday binary'ler private `/root/tealdrive-runtime-backups/home-stage-20261012` dizininde rollback için saklanır; bu dizin yayımlanmaz. Caddy tdrive.novastrum.dev eski backend'de kalır. Gerçek kullanıcı dosyaları/config/OS parolası/admin rolü değiştirilmedi.

## Henüz tamamlanmayan işler

Birinci ana maddenin kullanıcıya özel policy yönetimi kaldı: Root/admin/personal politika okuma/değiştirme komutları, protected kalıcılık, generation/revoke ve SDK/UI bağlantısı. Root-owned mevcut `access_policy.users` yapılandırması çalışmaya devam eder, fakat kişisel/admin değişimi için web mutation API'si henüz yoktur. Wizard migration ve fresh-install ayar üretimi de sonraki aşamadır.

Sanal UID/GID provisioning/native quota; Wizard; SDK/UI tamamlaması; direct TLS/custom UI; kullanıcı audit/undo; kalıcı güvenlik sayaçları/GC; temiz kurulum ve canlı geçiş henüz tamamlanmadı. Bu rapor projenin bütünü için hazır kabulü değildir.

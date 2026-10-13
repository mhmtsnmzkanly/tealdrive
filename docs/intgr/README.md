# TealDrive geliştirme ve kullanım belgeleri

2026-10-13: planlanan çekirdek uygulandı ve bu sunucuda yeni sürüme geçildi. Güncel teslim ve sınırlar [FINAL_REPORT.md](FINAL_REPORT.md), kabul kanıtları [TEST_RESULTS.md](TEST_RESULTS.md), işlerin kapanışı [PLAN.md](PLAN.md) içindedir. Önceki hedef şemalar [history](history/) altında korunur; eski örnekleri çalışan parser'a vermeyin.

Yeni yetkilendirilmiş çekirdek geliştirme: [NEXT_PHASES.md](NEXT_PHASES.md). Buradaki yeni işler önceki teslimden ayrıdır; kabul edilmeden canlı özellik sayılmaz.

| Belge | Konu |
|---|---|
| [HELPER.md](HELPER.md) / [SECURITY.md](SECURITY.md) | Root supervisor, User worker, kimlik/politika ve kernel sınırları |
| [SERVER.md](SERVER.md) / [AUTH.md](AUTH.md) | HTTP/WS, PAM enrollment, web parola, session ve gateway |
| [CONFIGURATION.md](CONFIGURATION.md) | Çalışan Root/overlay ayar ayrımı ve örnek |
| [TRANSPORT_SECURITY.md](TRANSPORT_SECURITY.md) | Yeni TLS yenileme davranışı, proxy trust ve ek şifreleme tasarım sınırları |
| [WIZARD.md](WIZARD.md) | Kurulum/güncelleme/onarım, hesap yönetimi ve ayrı ctl |
| [STORAGE.md](STORAGE.md) / [QUOTA.md](QUOTA.md) | /home/tealdrive düzeni, SQLite metadata, native kota |
| [SDK.md](SDK.md) / [STATE.md](STATE.md) | Public auth/files/admin API, managed/client state, events |
| [sdk-api.js](../../sdk-api.js) | Fonksiyon kullanım/JSDoc örnekleri; SDK implementasyonu değildir |
| [client.d.ts](../../src/ui/sdk/client.d.ts) | Metot/parametre/sonuç/event tiplerinin çalışan referansı |
| [UI.md](UI.md) | Gömülü TR/EN arayüz, chmod ve geçmiş/undo |
| [AUDIT.md](AUDIT.md) / [UNDO.md](UNDO.md) / [LIFECYCLE.md](LIFECYCLE.md) | Root korumalı geçmiş, geri alma, saklama/temizlik |
| [IMPLEMENTATION_DECISIONS.md](IMPLEMENTATION_DECISIONS.md) | Uygulama sırasında alınan gerekçeli kararlar |
| [CONTEXT.md](CONTEXT.md) | Mevcut sunucu ve geri dönüş bilgileri; sır içermez |
| [tools/dev](../../tools/dev/README.md) / [tools/ops](../../tools/ops/README.md) | Kabul araçları ve güvenli operasyon yardımcıları |

Dört binary vardır: server, işlem başına setuid helper, setuid olmayan Wizard ve ayrı ctl. SDK/UI bunlara gömülür; ilave daemon veya kullanıcı başına helper binary yoktur. Caddy zorunlu değildir; direct TLS ve güvenilir yerel TLS proxy desteklenir.

Root kritik tavanı ve ilk admini belirler. Sistem hesabı NSS UID/GID/home kullanır. Sanal hesap ayrı locked/nologin Linux UID/GID kullanır; sanal admin yalnız kendi tenant'ını yönetir. Panel yöneticisi Root olmaz. Kişisel politikalar yalnız yetkiyi daraltır.

Kullanıcı dosyaları SQLite'a girmez. Root DB hesap/parola özeti/tahsis/nesil ve korumalı kayıtları tutar. Web parola ve grant/token değerleri browser storage'a veya raporlara yazılmaz. Ek uygulama şifrelemesi, MFA/SSH kapatma, kullanıcı uygulamalarının süreç yönetimi bu teslim kapsamı dışındadır.

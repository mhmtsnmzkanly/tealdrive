# Sunucunun mevcut durumu — 2026-10-13

[Dizin](README.md) · [Bitiş raporu](FINAL_REPORT.md)

| Alan | Mevcut değer |
|---|---|
| URL | https://tdrive.novastrum.dev |
| Backend | 127.0.0.1:13100; TLS; plaintext handshake reddi |
| Servis | tealdrive.service; enabled, User/Group tealdrive |
| Servis NSS home | /home/tealdrive; nologin; UID995/GID986 |
| Working directory | /home/tealdrive/data/runtime |
| Binaryler | /home/tealdrive/bin; dört /usr/local/bin kısayolu |
| Root ayarları | /home/tealdrive/policy/server.toml ve helper.toml |
| Root key/metadata | policy/helper.key ve auth/accounts.db; içerikleri rapora alınmaz |
| Panel çalışma ayarı | /home/tealdrive/config/settings.toml |
| Test hesabı | tealdrive-test; Linux UID/GID1000, DB RoleUser, active |
| Kullanıcı kökü | Gerçek /home/tealdrive-test; legacy dosyalar üzerine yazılmadan kopyalandı |
| Sanal depolama | 1GiB ext4 image; Root pool512MiB; ayrı UID quota; admin otomatik oluşturulmadı |
| Journal/control audit | enabled; retention504h; undo upper24h |
| Lifecycle | trash504h, staging24h, deleted virtual36h; hourly timer enabled |
| Merkezi audit | /home/tealdrive/logs/server/audit.jsonl;10MiB,5archive,504h |
| TLS/proxy | Direct backend TLS + Caddy certificate validation; secure cookies/exact origin; trusted loopback XRealIp |
| Caddy admin | private /run/caddy-admin/admin.sock; service UID erişemez |

Web parola modeli aktiftir. Linux parolası ve mevcut web parolası değiştirilmedi; web parola Linux parolasıyla aynı olmak zorunda değildir. Özel parola dosyaları Root dışında okunamaz ve Git'te değildir. İlk admin Root Wizard ile belirlenir; eski server config'teki admin_users adı DB RoleUser hesabını otomatik admin yapmaz.

`Web test.md`60bayt ve26legacy regular file SHA karşılaştırmasıyla korundu. Legacy /srv/tealdrive-test orijinali geri dönüş için tutulur; canlı panel home'u kullanır. Kullanıcı tarafından değiştirilmiş home'un üstüne eski yedek körlemesine restore edilmez.

Rollback: /root/tealdrive-runtime-backups/final-migration-20261013 Root700; eski config/unit/Caddyfile, kullanıcı dosyaları ve metadata backup, retired executablelar içerir. Aktif eski test/candidate executablelar ve public setuid kopyaları kaldırıldı. Caddy DNS credential veya key değerleri hiçbir belgeye alınmadı.

Caddy yalnız resmî Cloudflare CIDR'lerinden CF-Connecting-IP kabul eder, X-Real-IP'yi üzerine yazar; backend yalnız loopback proxy'ye güvenir. Audit gerçek dış istemciIP31.58.91.190 ve socket peer127.0.0.1 ayrımını gösterdi. Son public Cloudflare HTTPS200 ve gerçek Chromium login/logout geçti; önceki403 son kabulde tekrarlanmadı, edge politikası gelecekte farklı otomatik isteği reddedebilir.

Sistem entegrasyon istisnaları PAM ve systemd dosyalarıdır. Eski deploy/operasyon yedekleri otomatik silinmedi; kullanıcı verisi veya güvenlik kaydı geliştirme çıktısı sayılmadı. Tüm önceki bağlam [history arşivinde](history/CONTEXT-before-final-2026-10-13.md) korunur.

Backend sertifikası policy/origin-cert.pem Root644; private TLS key tls/origin-key.pem Root:tealdrive0640. Caddy yalnız bu sertifikayı trust pool ile doğrular ve SNI tealdrive-origin.local kullanır; skip-verify yoktur. Böylece servis dururken sıradan OS kullanıcısının loopback portu taklit etmesi backend kimliğini sağlayamaz. Sertifika1yıl geçerlidir; süresi dolmadan Root cert/key ve Caddy trust pool birlikte yenilenir.

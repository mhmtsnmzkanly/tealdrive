# Çalışan dosya ve veri düzeni

[Dizin](README.md) · [Kota](QUOTA.md) · [Wizard](WIZARD.md)

```text
/home/tealdrive/                        Root:Root 0755; servis NSS home'u
├── bin/                               Root:Root 0755
│   ├── tealdrive-server               0755, tealdrive UID ile çalışır
│   ├── tealdrive-helper               4755, doğrulama → ayrı User worker
│   ├── tealdrive-wizard               0755, setuid değildir
│   └── tealdrive-ctl                  0755, setuid değildir
├── policy/                            Root:Root 0755
│   ├── server.toml                    Root:Root 0644
│   ├── helper.toml                    Root:Root 0600
│   └── helper.key                     Root:Root 0600
├── auth/                              Root:Root 0700
│   └── accounts.db                    Root:Root 0600; SQLite metadata
├── config/settings.toml               tealdrive:tealdrive 0600; sınırlı overlay
├── data/runtime/                      tealdrive:tealdrive 0700
├── logs/server/                       servis yazabilir; merkezi JSONL
├── logs/security/                     Root:Root 0700; ayrılmış Root alanı
├── storage/                           Root:Root 0700
│   ├── virtual-pool.ext4              Root:Root 0600; native volume image
│   └── update-backup-UUID/            Root-private metadata ve binary yedekleri
├── virtual-homes/                     ayrı nodev/nosuid/noexec/usrquota mount
│   ├── tdv_UID/                       kendi sanal UID/GID, 0700
│   └── .tealdrive-audit/              Root-only parent; UID'ye ücretlenen kayıtlar
├── tls/                               Root:tealdrive 0750; direct TLS seçeneği
└── ui/packages/                       Root onaylı paket kurulursa
```

`accounts.db` kimlik, rol, tahsis, web parola özeti, oturum/enrollment grant bilgileri, kişisel/Root/admin politikaları, undo tercihleri ve korumalı audit/cleanup tokenlarını içerir. SQLite WAL/journal sahipliği de Root'tadır. `users.d/*.toml` çalışma deposu değildir; kullanıcı politikaları protected DB'dedir. Server DB'yi açmaz; helper gateway üzerinden erişir.

```text
/home/alice/                           gerçek NSS home
├── projects/                         normal kullanıcı dosyaları
├── .env                              yeni home profilinde erişilebilir
├── .ssh/                             panelden görünmez/erişilemez
├── .tealdrive-trash/
│   ├── files/OPERATION_UUID           gerçek çöp kutusu içeriği
│   └── info/                         kullanıcı alanındaki yardımcı metadata
└── .tdrv-transaction-UUID[.zip|.tar.gz] geçici işlem çıktısı; hedefin parent'ında
```

Trash/staging normal discovery API'lerinde görünmez. Silme/restore/GC otoritesi Root DB kayıtlarından gelir; kullanıcı home'undaki metadata tek başına güvenilir sayılmaz. Eski kurulumdan gelen Root tokenı olmayan trash/geçici öğeler otomatik taranıp silinmez.

İşletim sistemi entegrasyonu nedeniyle unitler `/etc/systemd/system`, PAM kaydı `/etc/pam.d/tealdrive`, kolaylık symlinkleri `/usr/local/bin` altında kalır. Bunlar ayrı uygulama veri depoları değildir. Wizard kısayolları otomatik kurar ve unrelated dosyanın üzerine yazmaz.

`storage init 1GiB` yalnız durmuş uygulamada, boş mountpoint ve sanal hesap/provision intent yokken kullanılabilir. Image gerçek backing blocks ayırır. Boot mount bağımlılığı ve kernel quota desteği doğrulanır; destek yoksa application enforcement'a sessiz fallback olmaz. Metadata backup kullanıcı içeriklerini/image/runtime overlay'i kapsamaz; bunlar ayrıca yedeklenir.

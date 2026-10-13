# Caddy yönetim API'sini TealDrive servisinden ayırma

Caddy kullanımı zorunlu değildir. Bu belge yalnız Caddy proxy kurulumundaki yönetim arayüzünü kapsar.

Bu sunucuda2026-10-13 tarihinde TealDrive UID995'in localhost2019/config API'sine erişip DNS credential alanı bulunan Caddy config'ini okuyabildiği doğrulandı. Config file zaten Root:caddy640 idi; TCP yönetim API'si bu dosya iznini aşan ayrı bir erişim yoluydu. Config/anahtar değerleri test çıktısına veya repoya alınmadı.

```caddyfile
{
    admin unix//run/caddy-admin/admin.sock
}
```

```ini
# /etc/systemd/system/caddy.service.d/20-private-admin.conf
[Service]
RuntimeDirectory=caddy-admin
RuntimeDirectoryMode=0700
ExecReload=
ExecReload=/usr/bin/caddy reload --config /etc/caddy/Caddyfile --address unix//run/caddy-admin/admin.sock
```

Socket parent Caddy UID:GID0700; Caddyfile Root:caddy640. Root ve Caddy yönetim erişimini korur; TealDrive/diğer UID'ler connect edemez. İlk mevcut TCP'den geçiş reload'u eski admin adresiyle yapılır, sonrakiler Unix adresiyle. Binary yolunu kendi Caddy unit'inize göre ayarlayın. Root config/backup gizli tutulmalıdır. Resmi [admin seçenekleri](https://caddyserver.com/docs/caddyfile/options#admin) ve [API](https://caddyserver.com/docs/api) Unix socket ile filesystem erişim denetimini destekler; admin off seçilirse canlı reload kullanılamaz.

```bash
sudo systemctl daemon-reload
sudo systemctl reload caddy
sudo tools/dev/check.sh proxy-admin
```

Kabul: service TCP2019 connection refused, private Unix socket permission denied, Root Unix/config200; systemctl reload ve doğrudan yerel Caddy origin HTTPS200. Yalnız admin directive değiştiği byte karşılaştırmasıyla doğrulandı; domain/backend yönlendirmesi aynı kaldı. Bu sunucunun Cloudflare edge isteği403 aldı, yerel TLS origin200: dış Cloudflare erişim politikası final ağ kabulünde ayrıca değerlendirilecek; bu test dış erişime200 garantisi değildir. Runtime rollback kopyası Root700 /root/tealdrive-runtime-backups/caddy-admin-stage-20261013 altında, config600.

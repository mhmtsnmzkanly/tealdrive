# TealDrive QUOTA geliştirme sözleşmesi

> **Durum:** Hedef geliştirme; yeni komut/şemalar henüz uygulanmış değildir. Mevcut sürüm belgeleri docs kökünde, örnekleri docs/deployment altındadır.

[Dizin](README.md) · [İş planı](PLAN.md) · [Kararlar](DECISIONS.md)

## Esas veri kaynakları

DB kota tahsisi; helper gerçek kullanım. Rezervasyon kullanım değildir. Cache varsa zaman/yaş ve logical/allocated byte yöntemi görünür. Sonlu limit bilinmiyorsa yüzde null; ölçüm hatası sıfır değildir.

Root havuzu → admin bütçesi → sanal alt hesap tahsisi. Gerçek Linux OS kotası ayrı. Parallel yazma ortak kilit/rezervasyonla denetlenir. Hardlink/sparse muhasebesi ve scan limitleri tanımlanmalı. Ayarlanabilir %80 uyarı örneği; tekrar sorgu spam yapmaz. Logların kullanıcı kotasına eklenmesi ayrı açık karar.


### 3.5 Kota örneği

```text
TealDrive havuzu:             100 GiB (root belirler)
├── Admin A bütçesi:           60 GiB
│   ├── Sanal kullanıcı A1:    20 GiB
│   └── Sanal kullanıcı A2:    40 GiB
└── Admin B bütçesi:           40 GiB
    ├── Sanal kullanıcı B1:    10 GiB
    └── Henüz tahsis edilmemiş:30 GiB
```

Admin yönetim bütçesi fiziksel disk rezervasyonu değildir. Kota tahsisi DB'de, gerçek kullanım helper ölçümündedir; DB kullanım sayacı esas kaynak değildir. Tahsis, ölçülmüş kullanım ve in-flight rezervasyonlar ayrı tutulur; paralel işlemlerde tüm seviyeler birlikte kontrol edilir. Trash/staging sanal kotaya dahildir. Log/auth/cache bu havuzdan ayrı disk bütçesi ister. Yalnızca `tealdrive` UID'sine Linux kotası vermek ayrı UID'lerdeki sanal dosyaları kapsamaz; OS sert sınırı için ayrı filesystem/project quota gerekir. Gerçek Linux kullanıcısında OS kotası okunamıyorsa “bilgi alınamadı” gösterilir.



### 3.6 Helper kullanım ölçümü ve kota uyarısı

```toml
# config/settings.toml içinde önerilen ek bölüm:
[quota_notifications]
warning_percent = 80
```

Eşik 1–100 aralığında doğrulanır; gerçek kota sınırını değiştirmez. Kullanıcı home/trash/staging için helper ölçüm yapar; OS kota bilgisi veya sandbox içinde güvenli dosya taraması desteklenir. Kullanım ölçüsü (mantıksal byte ya da ayrılmış disk alanı) açık tanımlanır; farklı ölçüler aynı yüzde hesabına karıştırılmaz. Sayaç yerine ölçüm sonucu kullanılır; büyük ağaçlarda sınırlı cache varsa son ölçüm zamanı/yaşı görünür.

```javascript
const quota = await api.quota.getOwn();
// Örnek sonuç şekli; ölçüm helper'dan gelir:
// {
//   limitBytes: 10737418240,
//   usedBytes: 8589934592,
//   reservedBytes: 0,
//   usagePercent: 80,
//   warningPercent: 80,
//   warning: true,
//   measurement: 'logical_bytes',
//   measuredAt: '2026-10-11T21:00:00Z',
//   stale: false,
//   source: 'helper'
// }

api.on('quota.warning', ({ usagePercent, warningPercent }) => {
  showQuotaWarning(usagePercent, warningPercent);
});
```

`getOwn` yetkili helper sorgusudur. Panel belirlenen aralıkta veya dosya işlemi sonrasında sorgular; server sonuç değişimini SDK olayına dönüştürebilir, sürekli çalışan helper daemon gerekmez. Eşik ilk aşıldığında bildirim, kullanım barında kalıcı durum; tekrar sorgulama popup spam'i üretmez. Bildirim gizlenmesi kota zorlamasını kapatmaz. Sonlu limit bilinmiyorsa yüzde null, durum kodu gösterilir.

Kota aşmayı önleme UI kontrolünden bağımsızdır: Helper yazma sırasında aynı hesap ve ortak havuz için koordinasyon/kilit/rezervasyon uygular. Rezervasyonlar DB'de tutulsa da ölçülmüş kullanım değildir; çökme sonrası toparlanır. Ölçüm başarısızsa sıfır kullanılmış kabul edilmez. Dış programların yazmaları uygulama kotasıyla bütünüyle durdurulamaz; sert sınır OS kotası/filesystem kapsamındadır. Logların kullanıcı kotasına muhasebe edilmesi ayrı açık karardır; mevcut örnekte log/auth/cache ayrı bütçededir.



### 14. Sanal hesaplar, kota ve kullanıcı yaşam döngüsü — çok büyük

- Her hesaba ayrı nologin Linux UID/GID, kilitli OS parolası, sabit TealDrive hesap ID'si ve `virtual-homes/<id>`.
- Sistem araçlarıyla kontrollü provisioning: Keyfi username/home/shell/UID veya komut parametresi kabul etme; root'un ayırdığı kapsamda oluştur.
- Panel admini yalnızca root'un sanal hesap sağlama yetkisi verdiği durumda oluşturabilir. Root yetkili hesap sağlama işlemi genel helper dosya komutlarına karıştırılmaz.
- Ayrı Linux UID tek başına geniş world-readable host dosyalarını engellemez; helper sandbox sınırları tüm sanal işlemlerde sürer.
- Root, TealDrive sanal depolaması için ortak toplam kota havuzu belirler; her sanal admine bu havuzdan bütçe ayrılır, admin bütçesini sanal alt hesaplarına dağıtır. Admin bütçelerinin toplamı havuzu, alt hesap tahsislerinin toplamı ilgili admin bütçesini aşamaz. Gerçek Linux kullanıcılarının home'ları bu havuza dahil değildir. Bütçe değişimi ve paralel tahsisler atomik denetlenir.
- Sanal dosyalar ayrı UID'lere aittir: Yalnızca `tealdrive` UID'sine verilen Linux kullanıcı kotası ortak havuzu sınırlamaz. İlk katman root kontrollü uygulama kota politikasıdır; OS düzeyinde sert sınır istenirse `virtual-homes/` için desteklenen project quota veya kapasitesi sınırlı ayrı filesystem kullanılır. Host desteği Wizard tarafından kontrol edilir; UID izolasyonu kota için kaldırılmaz.
- Ortak sanal havuz trash/staging dahil kullanıcı verisini kapsar; log/auth/cache ve root kontrollü yönetim verilerine ayrıca disk bütçesi bırakılır. Tahsis edilen kapasite ile gerçek kullanım ayrı gösterilir; kota fiziksel boş alan garantisi değildir.
- Linux kullanıcılarında TealDrive kota ekleme yok; mevcut OS kota bilgisini read-only göster. Bulunamazsa sınırsız diye varsayma, “bilgi alınamıyor/yapılandırılmamış” ayrımını göster.
- Sanal hesaplarda kota uygula; paralel upload/copy/archive, trash, staging ve hatalı işlemler hesaba katılır. OS kota varsa daha sıkı sınır geçerlidir.
- Kota tahsisi DB'de tutulur; gerçek kullanım helper'dan sorulur, DB kullanım sayacı gerçekliğin kaynağı değildir. Kullanım yöntemi (mantıksal byte/ayrılmış disk alanı), ölçüm zamanı ve varsa cache yaşı yanıtta belirtilir; bu iki ölçü birbirine karıştırılmaz.
- Uyarı eşiği ayarlanabilir; ilk örnek %80. Panel kullanım çubuğu ve eşik aşımında bildirim gösterir; tekrar sorguda aynı uyarı spam edilmez, altına düşüp yeniden aşınca tekrar gösterilebilir. Kota/threshold değişimi değerlendirmeyi yeniler.
- Eşzamanlı yazmalar helper koordinasyonunda kilit ve in-flight rezervasyonlarla denetlenir; yalnızca arayüzün ön sorgusuna güvenilmez. Rezervasyon kayıtları ölçülmüş kullanım değildir. Ölçüm alınamazsa sıfır/sınırsız gösterilmez; kota gerektiren yazma güvenli biçimde reddedilir. Kabul: %80 eşiği, kota değişimi, paralel yazma, trash/staging ve başarısız ölçüm.
- Hesap kapatma, veri saklama, oturum iptali ve silme ayrı aşamalar. Kullanıcı silmek varsayılan olarak veriyi hemen silmez.
- Kabul: Sanal hesaplar SSH/PAM web modu üzerinden OS parolasıyla giremez; birbirinin verisine erişemez; root hesaplara dönüşemez; kota paralel işlemlerde aşılmaz.


## SDK gezinti/state güncellemesi

2026-10-12 yeni server state, helper path doğrulaması, üç SDK alanı ve event/kalıcı bildirim sözleşmesi [STATE.md](STATE.md) altında. Bu hedef henüz uygulanmış değildir; ilgili kabul testleri orada tanımlanır.

## Kullanıcı loglarının muhasebesi

Geliştiriciye bırakılan tercihle sanal hesabın kendi-görüntülenebilir merkezi işlem logları kota kullanımına eklenir. Helper home/trash/staging + o hesap ID'sinin güvenilir log boyutunu ölçer; DB ölçülmüş kullanımı temsil etmez. Log/transfer için ayrılmış rezervasyonlar iki kez sayılmaz. Admin bütçe ve ortak havuz bu log byte'larını kapsar; server/security/auth verileri ayrı sistem bütçesidir.

Gerçek Linux kullanıcılarının root-owned merkezi logları OS UID kotasına dahil edilemez varsayılır; ayrı sistem bütçesinde tutulur. UID izolasyonu veya root log koruması bu muhasebe için kaldırılmaz. Log reserve/rotation üst sınırları gelişimde belirlenir; kota dolunca kritik audit gereksinimi korunur.

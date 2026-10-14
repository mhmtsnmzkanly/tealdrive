# SDK state, gezinti ve bildirim sözleşmesi

14 Ekim 2026: çalışan SDK1.1 sözleşmesi. Önceki öneriler ve kullanıcı kararları [tarihsel taslakta](history/STATE-design-20261012.md) korunur. Eski API.user/goPath/currentDir örnekleri güncel metot adları değildir.

[Dizin](README.md) · [SDK](SDK.md) · [Tip referansı](../../src/ui/sdk/client.d.ts)

## Sorumluluklar

`auth` giriş/çıkış/enrollment/parola, `files` bütün kullanıcıların dosya/gezinti/geçmiş/kota işlemleri, `admin` atanmış kapsamda yönetim içindir. SDK kendi isteğe bağlı state katmanını tutar; server/helper erişim, yol, dosya sürümü ve gerçek kullanım otoritesidir. Browser storage hiçbir yetki vermez.

`handshake()` WebSocket ve sürümlü protokolü başlatır; server isterse TLS yanında Noise kanalı kurulur. Dosya işlemleri ayrıca doğrulanmış session gerektirir. Şifreleme kapsamı/pin güveni [TRANSPORT_SECURITY.md](TRANSPORT_SECURITY.md) içindedir.

## Managed state

```javascript
import { createClient } from '/sdk/v1/tealdrive.js';
const api = createClient({ stateMode: 'managed' });
await api.handshake(); // Mevcut HttpOnly session varsa devam eder; yoksa auth.login gerekir.
await api.files.validatePath('/musics');
await api.files.goTo('/musics');
console.log(api.files.getCurrentDirectory()); // Son doğrulanmış SDK konumu; I/O yapmaz.
const page = await api.files.list();
const stop = api.files.observeState(state => render(state));
stop();
```

`/musics` seçilen kullanıcının izinli kökü altında sanal yoldur; host absolute path değildir. State snapshot stale olabilir; gerçek işlem helper tarafından yeniden doğrulanır. Dizin silinir veya erişim değişirse refresh/gezinti en yakın izinli üst dizine döner. Geç gelen eski cevap yeni konumu ezmez.

Managed browser storage yalnız origin/account/generation kapsamlı gezinti tercihini tutar. Dosya içeriği, parola, cookie veya grant saklanmaz. Yeniden bağlantıda hatırlanan yol yeniden doğrulanır; kapanmış session state ile canlandırılmaz. `refreshState()` gerçek konumu yeniden kontrol eder.

## Kendi state katmanını kullanma

```javascript
const api = createClient({ stateMode: 'client', storage: null });
await api.handshake();
const page = await api.files.list({ rootId: 'home', path: 'musics' });
```

UI kendi store/router katmanını kullanabilir; helper kontrollerini atlayamaz. `getState()`/`observeState()` bağlantı ve doğrulanmış state snapshot'larını sunar. Sinyaller DOM veya belirli bir UI framework'üne bağımlı değildir.

## Event ve kalıcı koşul

```javascript
const stopEvent = api.files.on('quota.warning.entered', quota => notify(quota));
const stopCondition = api.files.observeQuotaWarning(warning => renderWarning(warning));
stopEvent();
stopCondition();
```

Event geçişi bildirir; observer mevcut koşulu hemen ve değiştikçe verir. Kota varsayılan80% eşiği Root/admin ayarına tabidir. Alan açılana kadar aktif uyarıyı UI gösterebilir; SDK popup çizmez. Ölçüm hatası kotayı sıfır veya uyarıyı temizlenmiş yapmaz; stale/unknown olarak tutulur. Kota polling observer varken çalışır; unsubscribe/disconnect kaynakları temizler.

## Kabul ve sınırlar

Gezinti restore/epoch, silinen dizinden üst dizine dönüş, kullanıcı bazlı storage, kota event/observer ve disconnect SDK regression testleriyle doğrulanır. Server restart sonrasında upload authority saklanmaz; gezinti tercihi upload ticket değildir. Ayrıntılı test ve canlı sonuçlar [TEST_RESULTS.md](TEST_RESULTS.md) içindedir.

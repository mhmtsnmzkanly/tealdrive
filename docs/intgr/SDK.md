# TealDrive SDK

SDK, dosya yöneticisi arayüzünün **işlem ve bağlantı katmanıdır**. UI geliştiricisi binary protokol, oturum, request ID, upload parçaları, filtre/sıralama veya hata çeviri kodlarını yeniden yazmaz. SDK DOM üretmez; React/Vue/vanilla JS ile kullanılabilir.

Kaynak: `src/ui/sdk/client.js`. Tipler: `src/ui/sdk/client.d.ts`. Server bunları binary'den `/sdk/v1/tealdrive.js` ve `/sdk/v1/tealdrive.d.ts` olarak sunar; custom UI override edemez. API adları `auth / files / admin`, camelCase. Geliştirme devam ediyor; aşağıdaki destek tablosu uygulanmış ile bekleyen altyapıyı ayırır.

## İlk kullanım

```javascript
import { createClient, TealDriveError } from '/sdk/v1/tealdrive.js';
const api = createClient();
await api.handshake(); // WS önce; protokol hazır, dosya yetkisi henüz yok.
await api.auth.login({ username, password }); // aynı WS; HttpOnly cookie bağlama otomatik.
const page = await api.files.list(); // doğrulanmış current directory.
```

HTTPS/WSS sertifika/şifrelemesini tarayıcı yapar; handshake JS kripto değildir. Pre-auth sadece hello/auth mesajlarıdır. Auth başarısızsa dosya/admin erişimi yok. Login HttpOnly cookie için tek kullanımlık HTTP bridge yapar; parola HTTP body'ye gönderilmez. Bridge token storage/URL/log'a yazılmaz.

## Üç alan

| Alan | Görev |
|---|---|
| `auth` | Login, session, logout; web enrollment/password/revoke için capability kontrollü yüzey |
| `files` | Listeleme, gezinti, metin, CRUD/chmod, arşiv, transfer, trash, kota, kendi geçmiş/politika |
| `admin` | Panel ayarları; `systemUsers` ve `virtualUsers` ayrı yönetim alanları |

`handshake`, `disconnect`, `getInfo`, `on` bağlantı yaşam döngüsüdür. Root ayarı değiştirme, keyfi UID veya shell komutu yok. Admin/root ayar tavanı server/helper'da doğrulanır.

## Kapsam ve destek

| İşlev | Durum |
|---|---|
| WS-first handshake/login/session/logout | Uygulandı; `/ws/auth` ve tek kullanımlık cookie bridge |
| List/filter/type/category/extensions/sort/pagination | Uygulandı; helper pagination öncesi filtreler |
| Stat/readText/writeText/createText/createFolder/rename/move/copy/chmod | Uygulandı; text save revision zorunlu SDK alanı |
| Search | Uygulandı; recursive search sıradan list filtresinden ayrı |
| Upload progress/cancel/chunk ACK | Uygulandı; begin/chunk interim frame özel completion semantiği |
| Download | Uygulandı; streaming Response, UI kaydetme görünümünü seçer |
| ZIP/TAR.GZ create/extract, trash/restore/permanent delete | Uygulandı; feature/policy belirleyici |
| Helper quota get, condition/event observer | Uygulandı; ölçüm/hata/threshold; observer varken sınırlı polling |
| Managed/client state, browser storage | Uygulandı; kullanıcı namespace; path restore yeniden doğrulanır |
| Admin settings ve Linux user picker | Mevcut endpointlerle uygulandı |
| Ayrı web parola/enroll, kalıcı revoke, policy/history/undo/virtual management | SDK yüzeyi tanımlı; ilgili backend capability açılmadan FEATURE_DISABLED; devam eden plan |

Bu tablo işin bitiş raporu değildir. Son sütundaki backendler tamamlanmadan destek varmış gibi UI'de sunulmaz. Public sözleşme yeni SDK feature'ı için tasarlanmıştır; legacy wire API'nin bozularak yeniden adlandırılması değildir.

## Listeleme ve arama

```javascript
await api.files.list();
await api.files.list({
  filter: { name: 'report', type: 'file', extensions: ['pdf', 'txt'] },
  sort: { by: 'modifiedAt', direction: 'desc' },
  limit: 100,
  cursor: null,
});
await api.files.search({ path: 'projects', query: 'config', extension: 'toml' });
```

Filter kategori presetleri: All/Folders/Code/Images/Media/Documents/Archives/Hidden/Writable/Symlinks. `includeHidden` erişim izni vermez. Extensionlar dot içermeyen `pdf` veya compound `tar.gz`; en fazla64, her biri32 byte. Name query1024 byte. Limit helper üst sınırına tabidir. Cursor aynı filtre/sort/path ile kullanılır; mutable dizinlerde list snapshot garantisi yoktur. Unknown options sessiz ignored olmaz.

## Metin ve çakışma

```javascript
const file = await api.files.readText({ path: 'notes.txt' });
await api.files.writeText({
  path: 'notes.txt', content: editor.getValue(), expectedRevision: file.revision,
});
await api.files.createText({ path: 'new.txt', content: 'Yeni dosya' });
```

Read revision content+dosya kimliği/metadata'dır. Stale revision `REVISION_CONFLICT`; UI metni koruyup karşılaştırma/yeniden yükleme sunar. Create-only `missing` mevcut dosyayı overwrite etmez. Helper karşılaştırmayı transaction içinde yapar; dış writer ile son compare/publish yarışı için mutlak garanti yoktur.

## State ve gezinti

```javascript
await api.files.goTo('/musics');
api.files.getCurrentDirectory(); // son doğrulanmış '/musics'; host absolute path değil.
const unsubscribe = api.files.observeState(state => render(state));
await api.files.refreshState();
unsubscribe();

// Kendi UI store'unu kullanmak isteyen:
const custom = createClient({ stateMode: 'client', storage: null });
```

Managed default; client mode açık root/path verir. Browser storage kullanıcı/account-generation + origin kapsamında konum tercihi; secret/editor content tutulmaz. Restore path doğrulanır. Storage disabled/dolu/bozuksa RAM fallback. Cache getter filesystem anlık gerçeği değildir; stale field bunu belirtir. Başka kullanıcıya ait state gösterilmez.

## İki bildirim biçimi

```javascript
api.files.on('quota.warning.entered', quota => showToast(quota));
api.files.observeQuotaWarning(({ active, quota, status }) => {
  active ? showPersistentAlert(quota, status) : hidePersistentAlert();
});
```

Condition abonelikte hemen mevcut durum verir; quota.get/sonraki polling değişimleri yansır. Eşik payload'dadır, event adına %80 gömülmez. Ölçüm hatasında eski active condition silinmez; unknown/stale döner. Observer varken default polling60s; `quotaPollMs:0` otomatik polling'i kapatır. Bağlantı kapanışı ve unsubscribe timer'ı temizler. Server/frontend transport olayları: server.shutdown, session.revoked, connection.changed, files.changed.

## İptal ve mutasyon güvenliği

```javascript
const controller = new AbortController();
await api.files.upload({ directory: 'uploads', file,
  signal: controller.signal, onProgress: p => showProgress(p),
});
const response = await api.files.download({ path: 'uploads/report.pdf' });
// response.body stream; UI kendi save/preview davranışını seçer.
```

Abort/timeout beklemeyi sonlandırır; genel mutasyonun yapılmadığını kanıtlamaz. `operationMayHaveCompleted` kontrolü vardır; körlemesine retry yok. Upload başarısızlığında abort cleanup denenir; kopmada helper/server lease staging temizliğini yapar. Sayfa state restore upload resume değildir.

## Hatalar, sürümleme ve detay referansı

TealDriveError: code/params/requestId/retryable/status. Rust legacy bitişik hata adları SDK'da kararlı snake-case kodlara normalize edilir. Kodlar UI i18n sözlüğünde çevrilir; raw HTML gösterilmez. Handshake olmadan HANDSHAKE_REQUIRED; auth olmadan AUTH_REQUIRED.

Major yalnızca uyumsuz değişimde; minor yeni uyumlu feature; patch düzeltme. Ortak options/result/state/event/hata şemaları genişlemeyi kolaylaştırır. UI geliştiricisi yeni feature görünümünü ekler; mevcut list filter'ı yeniden yazmaz.

Bütün imzalar ve tipler [client.d.ts](../../src/ui/sdk/client.d.ts), runtime [client.js](../../src/ui/sdk/client.js), kullanım taslağı [sdk-api.js](../../sdk-api.js). [STATE.md](STATE.md), [TEST_RESULTS.md](TEST_RESULTS.md), [IMPLEMENTATION_DECISIONS.md](IMPLEMENTATION_DECISIONS.md).

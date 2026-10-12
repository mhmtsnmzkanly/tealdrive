/**
 * TealDrive SDK API taslağı — kullanım/JSDoc referansı, SDK implementasyonu değildir.
 * Kaynak: TASARIM.md ve plan.md. Bu adlar/hatalar henüz mevcut server sözleşmesi değildir.
 * Import edilince ağ çağrısı veya dosya değişikliği yapmaz. Her örnek ayrı çağrılabilir;
 * mutasyon örnekleri gerçek SDK ile çalıştırılırsa veri değiştirir. Placeholder'ları değiştirin.
 *
 * Hedef SDK: import { createClient } from '/sdk/v1/tealdrive.js';
 * const API = createClient(); await API.handshake(); await API.auth.login(...);
 * handshake WS bağlantısını ve sürümlü protokolü başlatır; login aynı WS üzerinde yapılır.
 * Pre-auth kanalda yalnızca sınırlı handshake/auth; dosya/admin mesajları AUTH_REQUIRED.
 * İleride anahtar anlaşması handshake aşamasına eklenebilir; bu taslak kripto implementasyonu değildir.
 * Handshake tamamlanmadan ağ yöntemleri HANDSHAKE_REQUIRED verir; on/disconnect istisnadır.
 * Login olmadan dosya/yönetim yöntemleri AUTH_REQUIRED verir.
 * HTTPS/WSS şifrelemeyi sağlar; handshake özel kripto veya HTTP üzerinde güvenli JS sağlamaz.
 * TLS_REQUIRED, CONNECTION_FAILED, PROTOCOL_UNSUPPORTED transport hatalarıdır.
 * Ortak ağ hataları: CONNECTION_FAILED, SERVER_SHUTTING_DOWN, TIMEOUT, VALIDATION_FAILED.
 * Mutasyonlar ayrıca AUDIT_UNAVAILABLE veya OPERATION_STATUS_UNKNOWN verebilir.
 * Cevap kaybında aynı mutasyonu körlemesine tekrar etmeyin; idempotency/durum sorgusu gerekir.
 * createClient/connect tasarımının güncel önerisi: connect başlangıcını handshake içinde topla.
 *
 * @typedef {Object} FileRef
 * @property {string} rootId Oturumdan gelen kök kimliği; OS path'i değildir.
 * @property {string} path Köke göre relative yol; absolute path/.. yasaktır.
 * @typedef {Object} MutationResult
 * @property {string} operationId İşlem/audit korelasyon kimliği.
 * @property {string} [revision] İşlem sonucundaki güncel dosya revision'ı.
 * @typedef {Object} QuotaView
 * @property {number|null} limitBytes DB/OS kota limiti; bilinmiyorsa null.
 * @property {number|null} usedBytes Helper ölçümü; hata sıfır kullanım değildir.
 * @property {number} reservedBytes Devam eden işlem rezervasyonları; ölçümden ayrı.
 * @property {number|null} usagePercent Sonlu limit varsa kullanım yüzdesi.
 * @property {number} warningPercent Ayarlanabilir uyarı eşiği; örnek 80.
 * @property {boolean} warning Eşik aşılmış mı?
 * @property {string|null} measuredAt Helper ölçüm zamanı.
 * @property {boolean} stale Cache sonucu eski mi?
 * @typedef {Object} TealDriveError
 * @property {string} code Çeviri anahtarı/kararlı kod.
 * @property {Object} params Güvenli yapılandırılmış çeviri parametreleri; HTML değildir.
 * @property {string} requestId Teşhis/audit kimliği.
 * @property {boolean} retryable Otomatik mutasyon tekrarı izni anlamına gelmez.
 *
 * ServerInfo, Session, FileEntry, TrashEntry, AuditEvent, UndoCandidate, PolicyView,
 * AccountView, BudgetView hedef SDK'nın ayrıca yayımlayacağı veri tipleridir.
 * Revision/expectedState tokenleri opaque kabul edilir; güvenlik kararı helper'dadır.
 * Şifreleri kaynak kodu, console, localStorage/sessionStorage içine kaydetmeyin.
 */

export const examples = {
  // API.user.validatePath('/musics'); — Helper doğrulamalı ön kontrol; goPath tekrar doğrular.
  /** @param {Object} API @returns {Promise<Object>} @throws {TealDriveError} AUTH_REQUIRED, INVALID_PATH, PERMISSION_DENIED. */
  validatePath: (API) => API.user.validatePath('/musics'),

  // API.user.goPath('/musics'); — Managed gezintiyi helper doğrulamasıyla değiştirir.
  /** @param {Object} API @returns {Promise<Object>} @throws {TealDriveError} AUTH_REQUIRED, NOT_FOUND, PERMISSION_DENIED, STATE_CONFLICT. */
  goPath: (API) => API.user.goPath('/musics'),

  // API.user.currentDir(); — Son doğrulanmış sanal path; stale state güncel disk garantisi değildir.
  /** @param {Object} API @returns {string} @throws {TealDriveError} STATE_NOT_READY. */
  currentDir: (API) => API.user.currentDir(),

  // API.user.observeState(callback); — Mevcut snapshot hemen, sonraki değişiklikler geldikçe.
  /** @param {Object} API @returns {function(): void} Abonelikten çıkış. */
  observeState: (API) => API.user.observeState((state) => console.log(state.currentDirectory)),

  // API.user.warningQuota(callback); — Kalıcı condition; active=false gelince UI uyarıyı kaldırır.
  /** @param {Object} API @returns {function(): void} Abonelikten çıkış. */
  warningQuota: (API) => API.user.warningQuota((warning) => console.log(warning.active, warning.quota)),

  // API.user.on('quota.warning.entered', callback); — Eşiğin aşılma olayı; yüzde payload'da.
  /** @param {Object} API @returns {function(): void} Abonelikten çıkış. */
  userEvent: (API) => API.user.on('quota.warning.entered', (quota) => console.log(quota.usagePercent)),

  // API.handshake(); — Sunucu bağlantısını ve protokol/yetenek anlaşmasını başlatır. Giriş ve dosya çağrılarından önce gereklidir. TLS şifrelemesini kendisi oluşturmaz.
  /**
   * Sunucu bağlantısını ve protokol/yetenek anlaşmasını başlatır. Giriş ve dosya çağrılarından önce gereklidir. TLS şifrelemesini kendisi oluşturmaz.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.handshake();
   * @description SDK yöntemi parametreleri: Parametre yok.
   * @returns {Promise<ServerInfo>} Örnek çağrının sonucu.
   * @throws {TealDriveError} TLS_REQUIRED, PROTOCOL_UNSUPPORTED, CONNECTION_FAILED.
   */
  handshake: (API) => API.handshake(),

  // API.getInfo(); — Handshake ile edinilen kamusal sunucu ve API bilgilerini döndürür.
  /**
   * Handshake ile edinilen kamusal sunucu ve API bilgilerini döndürür.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.getInfo();
   * @description SDK yöntemi parametreleri: Parametre yok.
   * @returns {Promise<ServerInfo>} Örnek çağrının sonucu.
   * @throws {TealDriveError} HANDSHAKE_REQUIRED.
   */
  getInfo: (API) => API.getInfo(),

  // API.on("server.shutdown", ({ deadlineSeconds }) => console.log(deadlineSeconds)); — Olay dinleyicisi ekler; dönen fonksiyon aboneliği kaldırır. quota.warning, session.revoked, connection.changed de desteklenir.
  /**
   * Olay dinleyicisi ekler; dönen fonksiyon aboneliği kaldırır. quota.warning, session.revoked, connection.changed de desteklenir.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.on("server.shutdown", ({ deadlineSeconds }) => console.log(deadlineSeconds));
   * @description SDK yöntemi parametreleri: event: string; callback: (payload: object) => void.
   * @returns {function(): void} Örnek çağrının sonucu.
   * @throws {TealDriveError} INVALID_EVENT.
   */
  on: (API) => API.on("server.shutdown", ({ deadlineSeconds }) => console.log(deadlineSeconds)),

  // API.disconnect(); — Bağlantıyı kapatır; oturumu sonlandırmaz. Tekrar handshake gerektirir.
  /**
   * Bağlantıyı kapatır; oturumu sonlandırmaz. Tekrar handshake gerektirir.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.disconnect();
   * @description SDK yöntemi parametreleri: Parametre yok.
   * @returns {Promise<void>} Örnek çağrının sonucu.
   * @throws {TealDriveError} CONNECTION_FAILED.
   */
  disconnect: (API) => API.disconnect(),

  // API.auth.getSession(); — Aktif oturum, kökler ve yetenekleri döndürür; oturum yoksa null.
  /**
   * Aktif oturum, kökler ve yetenekleri döndürür; oturum yoksa null.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.auth.getSession();
   * @description SDK yöntemi parametreleri: Parametre yok.
   * @returns {Promise<Session|null>} Örnek çağrının sonucu.
   * @throws {TealDriveError} HANDSHAKE_REQUIRED, CONNECTION_FAILED.
   */
  getSession: (API) => API.auth.getSession(),

  // API.auth.login({ username: "alice", password: "<prompt ile alınan parola>" }); — Sunucunun seçtiği PAM veya web parolası yöntemiyle giriş yapar; plaintext parola kodda kalıcı tutulmaz.
  /**
   * Sunucunun seçtiği PAM veya web parolası yöntemiyle giriş yapar; plaintext parola kodda kalıcı tutulmaz.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.auth.login({ username: "alice", password: "<prompt ile alınan parola>" });
   * @description SDK yöntemi parametreleri: options: { username: string, password: string }.
   * @returns {Promise<Session>} Örnek çağrının sonucu.
   * @throws {TealDriveError} HANDSHAKE_REQUIRED, INVALID_CREDENTIALS, RATE_LIMITED, ENROLLMENT_REQUIRED, AUTH_UNAVAILABLE.
   */
  login: (API) => API.auth.login({ username: "alice", password: "<prompt ile alınan parola>" }),

  // API.auth.beginEnrollment({ username: "alice", linuxPassword: "<prompt ile alınan Linux parolası>" }); — İlk PAM doğrulamasından sonra tek kullanımlık kayıt yetkisi üretir; dosya erişim oturumu vermez.
  /**
   * İlk PAM doğrulamasından sonra tek kullanımlık kayıt yetkisi üretir; dosya erişim oturumu vermez.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.auth.beginEnrollment({ username: "alice", linuxPassword: "<prompt ile alınan Linux parolası>" });
   * @description SDK yöntemi parametreleri: options: { username: string, linuxPassword: string }.
   * @returns {Promise<{ id: string, expiresAt: string }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} HANDSHAKE_REQUIRED, INVALID_CREDENTIALS, ALREADY_ENROLLED, REGISTRATION_DENIED, RATE_LIMITED.
   */
  beginEnrollment: (API) => API.auth.beginEnrollment({ username: "alice", linuxPassword: "<prompt ile alınan Linux parolası>" }),

  // API.auth.completeEnrollment({ enrollmentId: "enroll_example", webPassword: "<yeni web parolası>" }); — Web parolasını oluşturur; kayıt yetkisi tek kullanımlıktır. Session alınmadan dosya çağrısı yapılmaz.
  /**
   * Web parolasını oluşturur; kayıt yetkisi tek kullanımlıktır. Session alınmadan dosya çağrısı yapılmaz.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.auth.completeEnrollment({ enrollmentId: "enroll_example", webPassword: "<yeni web parolası>" });
   * @description SDK yöntemi parametreleri: options: { enrollmentId: string, webPassword: string }.
   * @returns {Promise<{ enrolled: boolean }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} HANDSHAKE_REQUIRED, ENROLLMENT_EXPIRED, ENROLLMENT_USED, PASSWORD_POLICY_DENIED, SAME_AS_LINUX_PASSWORD.
   */
  completeEnrollment: (API) => API.auth.completeEnrollment({ enrollmentId: "enroll_example", webPassword: "<yeni web parolası>" }),

  // API.auth.changePassword({ currentPassword: "<mevcut web parolası>", newPassword: "<yeni web parolası>" }); — Web parolasını değiştirir ve eski oturum/biletleri iptal eder; yeniden giriş gerekir.
  /**
   * Web parolasını değiştirir ve eski oturum/biletleri iptal eder; yeniden giriş gerekir.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.auth.changePassword({ currentPassword: "<mevcut web parolası>", newPassword: "<yeni web parolası>" });
   * @description SDK yöntemi parametreleri: options: { currentPassword: string, newPassword: string }.
   * @returns {Promise<void>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, INVALID_CREDENTIALS, PASSWORD_POLICY_DENIED, AUDIT_UNAVAILABLE, FEATURE_DISABLED.
   */
  changePassword: (API) => API.auth.changePassword({ currentPassword: "<mevcut web parolası>", newPassword: "<yeni web parolası>" }),

  // API.auth.logout(); — Mevcut oturumu sonlandırır; hesabın web erişimini kalıcı kapatmaz.
  /**
   * Mevcut oturumu sonlandırır; hesabın web erişimini kalıcı kapatmaz.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.auth.logout();
   * @description SDK yöntemi parametreleri: Parametre yok.
   * @returns {Promise<void>} Örnek çağrının sonucu.
   * @throws {TealDriveError} HANDSHAKE_REQUIRED, CONNECTION_FAILED.
   */
  logout: (API) => API.auth.logout(),

  // API.auth.revokeSelf(); — Yalnızca gerçek Linux kullanıcısı: kendi web erişimini ve tüm oturum/biletlerini iptal eder; Linux hesabını silmez.
  /**
   * Yalnızca gerçek Linux kullanıcısının kendi erişimini iptal eder; sanal hesapta admin gerekir.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.auth.revokeSelf();
   * @description SDK yöntemi parametreleri: Parametre yok.
   * @returns {Promise<void>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, REVOKE_NOT_ALLOWED, AUDIT_UNAVAILABLE.
   */
  revokeSelf: (API) => API.auth.revokeSelf(),

  // API.user.list({ rootId: "home", path: "", limit: 100, cursor: null }); — Kök içindeki dizini sayfalı listeler; yasaklı öğeleri göstermez.
  /**
   * Kök içindeki dizini sayfalı listeler; yasaklı öğeleri göstermez.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.user.list({ rootId: "home", path: "", limit: 100, cursor: null });
   * @description SDK yöntemi parametreleri: options: { rootId: string, path: string, limit?: number, cursor?: string|null }.
   * @returns {Promise<{ items: FileEntry[], nextCursor: string|null }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, INVALID_PATH, NOT_FOUND, PERMISSION_DENIED.
   */
  list: (API) => API.user.list({ rootId: "home", path: "", limit: 100, cursor: null }),

  // API.user.stat({ rootId: "home", path: "projects/app/config.toml" }); — Dosya metadata, revision ve işlem yeteneklerini döndürür.
  /**
   * Dosya metadata, revision ve işlem yeteneklerini döndürür.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.user.stat({ rootId: "home", path: "projects/app/config.toml" });
   * @description SDK yöntemi parametreleri: options: { rootId: string, path: string }.
   * @returns {Promise<FileEntry>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, INVALID_PATH, NOT_FOUND, PERMISSION_DENIED.
   */
  stat: (API) => API.user.stat({ rootId: "home", path: "projects/app/config.toml" }),

  // API.user.readText({ rootId: "home", path: "projects/app/.env" }); — Metni ve aynı içeriğe ait revision tokenini döndürür.
  /**
   * Metni ve aynı içeriğe ait revision tokenini döndürür.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.user.readText({ rootId: "home", path: "projects/app/.env" });
   * @description SDK yöntemi parametreleri: options: { rootId: string, path: string }.
   * @returns {Promise<{ content: string, revision: string, metadata: object, capabilities: object }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, PERMISSION_DENIED, FILE_TOO_LARGE, INVALID_ENCODING, NOT_FOUND.
   */
  readText: (API) => API.user.readText({ rootId: "home", path: "projects/app/.env" }),

  // API.user.writeText({ rootId: "home", path: "notes.md", content: "Yeni içerik", expectedRevision: "revision_from_readText" }); — Beklenen sürüm hâlâ geçerliyse metni kaydeder; conflict durumunda üzerine yazmaz.
  /**
   * Beklenen sürüm hâlâ geçerliyse metni kaydeder; conflict durumunda üzerine yazmaz.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.user.writeText({ rootId: "home", path: "notes.md", content: "Yeni içerik", expectedRevision: "revision_from_readText" });
   * @description SDK yöntemi parametreleri: options: { rootId: string, path: string, content: string, expectedRevision: string }.
   * @returns {Promise<MutationResult>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, REVISION_CONFLICT, PERMISSION_DENIED, QUOTA_EXCEEDED, FILE_TOO_LARGE.
   */
  writeText: (API) => API.user.writeText({ rootId: "home", path: "notes.md", content: "Yeni içerik", expectedRevision: "revision_from_readText" }),

  // API.user.createFolder({ rootId: "home", path: "projects/new-app" }); — Yeni klasör oluşturur; mevcut hedef üzerine yazmaz.
  /**
   * Yeni klasör oluşturur; mevcut hedef üzerine yazmaz.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.user.createFolder({ rootId: "home", path: "projects/new-app" });
   * @description SDK yöntemi parametreleri: options: { rootId: string, path: string }.
   * @returns {Promise<MutationResult>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, INVALID_PATH, ALREADY_EXISTS, PERMISSION_DENIED, QUOTA_EXCEEDED.
   */
  createFolder: (API) => API.user.createFolder({ rootId: "home", path: "projects/new-app" }),

  // API.user.rename({ rootId: "home", path: "old.md", newName: "new.md", expectedRevision: "current_revision" }); — Aynı dizinde ad değiştirir; newName yol değil tek dosya adıdır.
  /**
   * Aynı dizinde ad değiştirir; newName yol değil tek dosya adıdır.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.user.rename({ rootId: "home", path: "old.md", newName: "new.md", expectedRevision: "current_revision" });
   * @description SDK yöntemi parametreleri: options: { rootId: string, path: string, newName: string, expectedRevision: string }.
   * @returns {Promise<MutationResult>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, INVALID_NAME, REVISION_CONFLICT, ALREADY_EXISTS, PERMISSION_DENIED.
   */
  rename: (API) => API.user.rename({ rootId: "home", path: "old.md", newName: "new.md", expectedRevision: "current_revision" }),

  // API.user.move({ source: { rootId: "home", path: "new.md" }, destination: { rootId: "home", path: "archive/new.md" }, expectedRevision: "current_revision" }); — Dosya/klasörü taşır; kaynak, hedef ve alt içerik politikaları uygulanır.
  /**
   * Dosya/klasörü taşır; kaynak, hedef ve alt içerik politikaları uygulanır.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.user.move({ source: { rootId: "home", path: "new.md" }, destination: { rootId: "home", path: "archive/new.md" }, expectedRevision: "current_revision" });
   * @description SDK yöntemi parametreleri: options: { source: FileRef, destination: FileRef, expectedRevision: string }.
   * @returns {Promise<MutationResult>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, REVISION_CONFLICT, ALREADY_EXISTS, PERMISSION_DENIED, QUOTA_EXCEEDED.
   */
  move: (API) => API.user.move({ source: { rootId: "home", path: "new.md" }, destination: { rootId: "home", path: "archive/new.md" }, expectedRevision: "current_revision" }),

  // API.user.copy({ source: { rootId: "home", path: "notes.md" }, destination: { rootId: "home", path: "notes-copy.md" }, expectedRevision: "current_revision" }); — Dosya/klasörü kopyalar; yasaklı içerik dolaylı olarak okunamaz.
  /**
   * Dosya/klasörü kopyalar; yasaklı içerik dolaylı olarak okunamaz.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.user.copy({ source: { rootId: "home", path: "notes.md" }, destination: { rootId: "home", path: "notes-copy.md" }, expectedRevision: "current_revision" });
   * @description SDK yöntemi parametreleri: options: { source: FileRef, destination: FileRef, expectedRevision: string }.
   * @returns {Promise<MutationResult>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, REVISION_CONFLICT, ALREADY_EXISTS, PERMISSION_DENIED, QUOTA_EXCEEDED.
   */
  copy: (API) => API.user.copy({ source: { rootId: "home", path: "notes.md" }, destination: { rootId: "home", path: "notes-copy.md" }, expectedRevision: "current_revision" }),

  // API.user.chmod({ rootId: "home", path: "notes.md", mode: 0o640, expectedRevision: "current_revision" }); — Normal izin bitlerini değiştirir; özel bitler yasaktır. UI riskli seçimde önceden uyarır.
  /**
   * Normal izin bitlerini değiştirir; özel bitler yasaktır. UI riskli seçimde önceden uyarır.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.user.chmod({ rootId: "home", path: "notes.md", mode: 0o640, expectedRevision: "current_revision" });
   * @description SDK yöntemi parametreleri: options: { rootId: string, path: string, mode: number (0..0o777), expectedRevision: string }.
   * @returns {Promise<MutationResult>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, INVALID_MODE, SPECIAL_BITS_DENIED, REVISION_CONFLICT, PERMISSION_DENIED.
   */
  chmod: (API) => API.user.chmod({ rootId: "home", path: "notes.md", mode: 0o640, expectedRevision: "current_revision" }),

  // API.user.search({ rootId: "home", path: "projects", query: "config", limit: 100 }); — İzinli ağaçta dosya adı arar; yasaklı dizinlere girmez.
  /**
   * İzinli ağaçta dosya adı arar; yasaklı dizinlere girmez.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.user.search({ rootId: "home", path: "projects", query: "config", limit: 100 });
   * @description SDK yöntemi parametreleri: options: { rootId: string, path: string, query: string, limit?: number }.
   * @returns {Promise<{ items: FileEntry[], truncated: boolean }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, INVALID_PATH, PERMISSION_DENIED, INVALID_QUERY.
   */
  search: (API) => API.user.search({ rootId: "home", path: "projects", query: "config", limit: 100 }),

  // API.user.upload({ rootId: "home", directory: "uploads", file: new File(["örnek"], "example.txt"), signal: new AbortController().signal, onProgress: ({ transferredBytes, totalBytes }) => console.log(transferredBytes, totalBytes) }); — File/Blob transferini yürütür; başarı yalnızca publish onayından sonra döner.
  /**
   * File/Blob transferini yürütür; başarı yalnızca publish onayından sonra döner.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.user.upload({ rootId: "home", directory: "uploads", file: new File(["örnek"], "example.txt"), signal: new AbortController().signal, onProgress: ({ transferredBytes, totalBytes }) => console.log(transferredBytes, totalBytes) });
   * @description SDK yöntemi parametreleri: options: { rootId: string, directory: string, file: File, signal?: AbortSignal, onProgress?: function }.
   * @returns {Promise<MutationResult>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, FILE_TOO_LARGE, QUOTA_EXCEEDED, PERMISSION_DENIED, ABORTED, TRANSFER_FAILED.
   */
  upload: (API) => API.user.upload({ rootId: "home", directory: "uploads", file: new File(["örnek"], "example.txt"), signal: new AbortController().signal, onProgress: ({ transferredBytes, totalBytes }) => console.log(transferredBytes, totalBytes) }),

  // API.user.download({ rootId: "home", path: "uploads/report.pdf" }); — Tarayıcı indirmesini başlatır; dosyanın tamamını JS belleğine almak zorunlu değildir. Sonuç kullanıcı diske kaydetti demek değildir.
  /**
   * Tarayıcı indirmesini başlatır; dosyanın tamamını JS belleğine almak zorunlu değildir. Sonuç kullanıcı diske kaydetti demek değildir.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.user.download({ rootId: "home", path: "uploads/report.pdf" });
   * @description SDK yöntemi parametreleri: options: { rootId: string, path: string }.
   * @returns {Promise<void>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, NOT_FOUND, PERMISSION_DENIED, TRANSFER_FAILED.
   */
  download: (API) => API.user.download({ rootId: "home", path: "uploads/report.pdf" }),

  // API.user.archives.create({ sources: [{ rootId: "home", path: "notes" }], destination: { rootId: "home", path: "notes.zip" }, format: "zip" }); — İzinli içerikten arşiv oluşturur; yasaklı alt dosya sızdırmaz.
  /**
   * İzinli içerikten arşiv oluşturur; yasaklı alt dosya sızdırmaz.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.user.archives.create({ sources: [{ rootId: "home", path: "notes" }], destination: { rootId: "home", path: "notes.zip" }, format: "zip" });
   * @description SDK yöntemi parametreleri: options: { sources: FileRef[], destination: FileRef, format: "zip"|"tar.gz" }.
   * @returns {Promise<MutationResult>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, FEATURE_DISABLED, PERMISSION_DENIED, QUOTA_EXCEEDED, ALREADY_EXISTS.
   */
  archiveCreate: (API) => API.user.archives.create({ sources: [{ rootId: "home", path: "notes" }], destination: { rootId: "home", path: "notes.zip" }, format: "zip" }),

  // API.user.archives.extract({ source: { rootId: "home", path: "notes.zip" }, destination: { rootId: "home", path: "extracted" }, expectedRevision: "archive_revision" }); — Arşivi hedefe güvenli açar; path escape ve kota denetlenir.
  /**
   * Arşivi hedefe güvenli açar; path escape ve kota denetlenir.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.user.archives.extract({ source: { rootId: "home", path: "notes.zip" }, destination: { rootId: "home", path: "extracted" }, expectedRevision: "archive_revision" });
   * @description SDK yöntemi parametreleri: options: { source: FileRef, destination: FileRef, expectedRevision: string }.
   * @returns {Promise<MutationResult>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, FEATURE_DISABLED, INVALID_ARCHIVE, REVISION_CONFLICT, PERMISSION_DENIED, QUOTA_EXCEEDED.
   */
  archiveExtract: (API) => API.user.archives.extract({ source: { rootId: "home", path: "notes.zip" }, destination: { rootId: "home", path: "extracted" }, expectedRevision: "archive_revision" }),

  // API.user.trash.move({ rootId: "home", path: "old.md", expectedRevision: "current_revision" }); — Dosya/klasörü kendi çöp kutusuna taşır; kalıcı silmez.
  /**
   * Dosya/klasörü kendi çöp kutusuna taşır; kalıcı silmez.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.user.trash.move({ rootId: "home", path: "old.md", expectedRevision: "current_revision" });
   * @description SDK yöntemi parametreleri: options: { rootId: string, path: string, expectedRevision: string }.
   * @returns {Promise<{ operationId: string, trashId: string }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, REVISION_CONFLICT, PERMISSION_DENIED, QUOTA_EXCEEDED.
   */
  trashMove: (API) => API.user.trash.move({ rootId: "home", path: "old.md", expectedRevision: "current_revision" }),

  // API.user.trash.list({ rootId: "home", limit: 50, cursor: null }); — Kendi çöp kutusunu listeler; iç depo yolunu açmaz.
  /**
   * Kendi çöp kutusunu listeler; iç depo yolunu açmaz.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.user.trash.list({ rootId: "home", limit: 50, cursor: null });
   * @description SDK yöntemi parametreleri: options: { rootId: string, limit?: number, cursor?: string|null }.
   * @returns {Promise<{ items: TrashEntry[], nextCursor: string|null }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, PERMISSION_DENIED.
   */
  trashList: (API) => API.user.trash.list({ rootId: "home", limit: 50, cursor: null }),

  // API.user.trash.restore({ rootId: "home", trashId: "trash_example" }); — Saklanan içerik ve güncel politika uygunsa geri yükler; dolu hedefin üzerine yazmaz.
  /**
   * Saklanan içerik ve güncel politika uygunsa geri yükler; dolu hedefin üzerine yazmaz.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.user.trash.restore({ rootId: "home", trashId: "trash_example" });
   * @description SDK yöntemi parametreleri: options: { rootId: string, trashId: string }.
   * @returns {Promise<MutationResult>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, NOT_FOUND, ALREADY_EXISTS, UNDO_STATE_MISMATCH, PERMISSION_DENIED.
   */
  trashRestore: (API) => API.user.trash.restore({ rootId: "home", trashId: "trash_example" }),

  // API.user.trash.deletePermanently({ rootId: "home", trashId: "trash_example" }); — İzin verilmişse çöp kutusu içeriğini kalıcı siler.
  /**
   * İzin verilmişse çöp kutusu içeriğini kalıcı siler.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.user.trash.deletePermanently({ rootId: "home", trashId: "trash_example" });
   * @description SDK yöntemi parametreleri: options: { rootId: string, trashId: string }.
   * @returns {Promise<MutationResult>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, FEATURE_DISABLED, PERMISSION_DENIED, NOT_FOUND.
   */
  trashDelete: (API) => API.user.trash.deletePermanently({ rootId: "home", trashId: "trash_example" }),

  // API.user.audit.listOwn({ limit: 50, cursor: null, action: "ChmodFile" }); — Yalnızca kendi audit olaylarını sayfalı sorgular; başka username/UID parametresi yoktur.
  /**
   * Yalnızca kendi audit olaylarını sayfalı sorgular; başka username/UID parametresi yoktur.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.user.audit.listOwn({ limit: 50, cursor: null, action: "ChmodFile" });
   * @description SDK yöntemi parametreleri: options: { limit?: number, cursor?: string|null, action?: string }.
   * @returns {Promise<{ events: AuditEvent[], nextCursor: string|null }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, INVALID_FILTER, AUDIT_UNAVAILABLE.
   */
  auditList: (API) => API.user.audit.listOwn({ limit: 50, cursor: null, action: "ChmodFile" }),

  // API.user.undo.listOwn({ limit: 20, cursor: null }); — Geri alma adayları canUndo/reasonCode/expectedState ile döner.
  /**
   * Geri alma adayları canUndo/reasonCode/expectedState ile döner.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.user.undo.listOwn({ limit: 20, cursor: null });
   * @description SDK yöntemi parametreleri: options: { limit?: number, cursor?: string|null }.
   * @returns {Promise<{ items: UndoCandidate[], nextCursor: string|null }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, FEATURE_DISABLED.
   */
  undoList: (API) => API.user.undo.listOwn({ limit: 20, cursor: null }),

  // API.user.undo.apply({ operationId: "op_example", expectedState: "state_from_candidate" }); — Beklenen durumu helper uygulama anında tekrar doğrular; eski audit silinmez.
  /**
   * Beklenen durumu helper uygulama anında tekrar doğrular; eski audit silinmez.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.user.undo.apply({ operationId: "op_example", expectedState: "state_from_candidate" });
   * @description SDK yöntemi parametreleri: options: { operationId: string, expectedState: string }.
   * @returns {Promise<MutationResult>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, FEATURE_DISABLED, UNDO_STATE_MISMATCH, UNDO_EXPIRED, PERMISSION_DENIED, ALREADY_EXISTS.
   */
  undoApply: (API) => API.user.undo.apply({ operationId: "op_example", expectedState: "state_from_candidate" }),

  // API.user.policy.getOwn(); — Etkin kişisel/admin/root kural kaynaklarını güvenli biçimde döndürür.
  /**
   * Etkin kişisel/admin/root kural kaynaklarını güvenli biçimde döndürür.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.user.policy.getOwn();
   * @description SDK yöntemi parametreleri: Parametre yok.
   * @returns {Promise<PolicyView>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED.
   */
  policyGet: (API) => API.user.policy.getOwn(),

  // API.user.policy.addPersonalRule({ path: "private", recursive: true, deny: ["discover", "read", "write"] }); — Yalnızca kişisel katmana kısıt ekler; yeni yetki vermez.
  /**
   * Yalnızca kişisel katmana kısıt ekler; yeni yetki vermez.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.user.policy.addPersonalRule({ path: "private", recursive: true, deny: ["discover", "read", "write"] });
   * @description SDK yöntemi parametreleri: options: { path: string, recursive: boolean, deny: string[] }.
   * @returns {Promise<{ ruleId: string, revision: string }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, INVALID_RULE, AUDIT_UNAVAILABLE.
   */
  policyAdd: (API) => API.user.policy.addPersonalRule({ path: "private", recursive: true, deny: ["discover", "read", "write"] }),

  // API.user.policy.removePersonalRule({ ruleId: "personal_rule", expectedRevision: "policy_revision" }); — Kendi kuralını kaldırır; üst katman yasakları geçerli kalır.
  /**
   * Kendi kuralını kaldırır; üst katman yasakları geçerli kalır.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.user.policy.removePersonalRule({ ruleId: "personal_rule", expectedRevision: "policy_revision" });
   * @description SDK yöntemi parametreleri: options: { ruleId: string, expectedRevision: string }.
   * @returns {Promise<{ revision: string }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, PERMISSION_DENIED, REVISION_CONFLICT, AUDIT_UNAVAILABLE.
   */
  policyRemove: (API) => API.user.policy.removePersonalRule({ ruleId: "personal_rule", expectedRevision: "policy_revision" }),

  // API.user.quota.getOwn(); — Kota DB kayıtlarından, gerçek kullanım helper ölçümünden gelir; eşik örneğin yüzde 80.
  /**
   * Kota DB kayıtlarından, gerçek kullanım helper ölçümünden gelir; eşik örneğin yüzde 80.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.user.quota.getOwn();
   * @description SDK yöntemi parametreleri: Parametre yok.
   * @returns {Promise<QuotaView>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, QUOTA_MEASUREMENT_UNAVAILABLE.
   */
  quotaGet: (API) => API.user.quota.getOwn(),

  // API.admin.settings.get(); — Webden değişebilir ayarları ve revision bilgisini döndürür; kritik config/secrets yoktur.
  /**
   * Webden değişebilir ayarları ve revision bilgisini döndürür; kritik config/secrets yoktur.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.admin.settings.get();
   * @description SDK yöntemi parametreleri: Parametre yok.
   * @returns {Promise<{ settings: object, revision: string }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, ADMIN_REQUIRED.
   */
  settingsGet: (API) => API.admin.settings.get(),

  // API.admin.settings.patch({ expectedRevision: "settings_revision", changes: { quota_notifications: { warning_percent: 80 } } }); — Yalnızca izinli çalışma ayarlarını tavan içinde günceller.
  /**
   * Yalnızca izinli çalışma ayarlarını tavan içinde günceller.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.admin.settings.patch({ expectedRevision: "settings_revision", changes: { quota_notifications: { warning_percent: 80 } } });
   * @description SDK yöntemi parametreleri: options: { expectedRevision: string, changes: object }.
   * @returns {Promise<{ settings: object, revision: string }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, ADMIN_REQUIRED, CRITICAL_SETTING_DENIED, REVISION_CONFLICT, VALIDATION_FAILED.
   */
  settingsPatch: (API) => API.admin.settings.patch({ expectedRevision: "settings_revision", changes: { quota_notifications: { warning_percent: 80 } } }),

  // API.admin.systemUsers.list({ limit: 50, cursor: null }); — Yöneticiye atanmış kapsamdaki kullanıcıları listeler; servis adayları otomatik açılmaz.
  /**
   * Yöneticiye atanmış kapsamdaki kullanıcıları listeler; servis adayları otomatik açılmaz.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.admin.systemUsers.list({ limit: 50, cursor: null });
   * @description SDK yöntemi parametreleri: options: { limit?: number, cursor?: string|null }.
   * @returns {Promise<{ items: AccountView[], nextCursor: string|null }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, ADMIN_REQUIRED.
   */
  adminUsersList: (API) => API.admin.systemUsers.list({ limit: 50, cursor: null }),

  // API.admin.systemUsers.allow({ accountId: "acct_alice" }); — Root tarafından devredilmiş kapsamda web erişim onayı verir; admin rolü vermez.
  /**
   * Root tarafından devredilmiş kapsamda web erişim onayı verir; admin rolü vermez.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.admin.systemUsers.allow({ accountId: "acct_alice" });
   * @description SDK yöntemi parametreleri: options: { accountId: string }.
   * @returns {Promise<AccountView>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, ADMIN_REQUIRED, PERMISSION_DENIED, AUDIT_UNAVAILABLE.
   */
  adminUsersAllow: (API) => API.admin.systemUsers.allow({ accountId: "acct_alice" }),

  // API.admin.systemUsers.deny({ accountId: "acct_alice" }); — Atanmış kapsamdaki erişimi kapatır; aktif oturum/biletleri iptal eder.
  /**
   * Atanmış kapsamdaki erişimi kapatır; aktif oturum/biletleri iptal eder.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.admin.systemUsers.deny({ accountId: "acct_alice" });
   * @description SDK yöntemi parametreleri: options: { accountId: string }.
   * @returns {Promise<void>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, ADMIN_REQUIRED, PERMISSION_DENIED, AUDIT_UNAVAILABLE.
   */
  adminUsersDeny: (API) => API.admin.systemUsers.deny({ accountId: "acct_alice" }),

  // API.admin.virtualUsers.list({ limit: 50, cursor: null }); — Yalnızca yönettiği sanal alt hesapları listeler.
  /**
   * Yalnızca yönettiği sanal alt hesapları listeler.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.admin.virtualUsers.list({ limit: 50, cursor: null });
   * @description SDK yöntemi parametreleri: options: { limit?: number, cursor?: string|null }.
   * @returns {Promise<{ items: AccountView[], nextCursor: string|null }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, ADMIN_REQUIRED.
   */
  virtualList: (API) => API.admin.virtualUsers.list({ limit: 50, cursor: null }),

  // API.admin.virtualUsers.create({ name: "customer-one", quotaBytes: 20 * 1024 ** 3, idempotencyKey: "unique_request_key" }); — Ayrı UID/GID ile awaiting_password alt hesap sağlar; admin oluşturamaz.
  /**
   * Ayrı UID/GID ile awaiting_password alt hesap sağlar; admin oluşturamaz.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.admin.virtualUsers.create({ name: "customer-one", quotaBytes: 20 * 1024 ** 3, idempotencyKey: "unique_request_key" });
   * @description SDK yöntemi parametreleri: options: { name: string, quotaBytes: number, idempotencyKey: string }.
   * @returns {Promise<AccountView>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, ADMIN_REQUIRED, QUOTA_BUDGET_EXCEEDED, INVALID_NAME, AUDIT_UNAVAILABLE, PROVISIONING_FAILED.
   */
  virtualCreate: (API) => API.admin.virtualUsers.create({ name: "customer-one", quotaBytes: 20 * 1024 ** 3, idempotencyKey: "unique_request_key" }),

  // API.admin.virtualUsers.setQuota({ accountId: "acct_customer", quotaBytes: 10 * 1024 ** 3, expectedRevision: "account_revision" }); — Admin bütçesi içinde sanal alt hesabın kotasını değiştirir; root havuzunu değiştirmez.
  /**
   * Admin bütçesi içinde sanal alt hesabın kotasını değiştirir; root havuzunu değiştirmez.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.admin.virtualUsers.setQuota({ accountId: "acct_customer", quotaBytes: 10 * 1024 ** 3, expectedRevision: "account_revision" });
   * @description SDK yöntemi parametreleri: options: { accountId: string, quotaBytes: number, expectedRevision: string }.
   * @returns {Promise<AccountView>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, ADMIN_REQUIRED, PERMISSION_DENIED, QUOTA_BUDGET_EXCEEDED, QUOTA_BELOW_USAGE, REVISION_CONFLICT.
   */
  virtualQuota: (API) => API.admin.virtualUsers.setQuota({ accountId: "acct_customer", quotaBytes: 10 * 1024 ** 3, expectedRevision: "account_revision" }),

  // API.admin.virtualUsers.disable({ accountId: "acct_customer" }); — Sanal alt hesabı kapatır ve oturumlarını iptal eder; veriyi silmez.
  /**
   * Sanal alt hesabı kapatır ve oturumlarını iptal eder; veriyi silmez.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.admin.virtualUsers.disable({ accountId: "acct_customer" });
   * @description SDK yöntemi parametreleri: options: { accountId: string }.
   * @returns {Promise<void>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, ADMIN_REQUIRED, PERMISSION_DENIED, AUDIT_UNAVAILABLE.
   */
  virtualDisable: (API) => API.admin.virtualUsers.disable({ accountId: "acct_customer" }),

  // API.admin.virtualUsers.enable({ accountId: "acct_customer" }); — Kendi alt hesabını root engeli yoksa etkinleştirir; eski biletleri canlandırmaz.
  /**
   * Kendi alt hesabını root engeli yoksa etkinleştirir; eski biletleri canlandırmaz.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.admin.virtualUsers.enable({ accountId: "acct_customer" });
   * @description SDK yöntemi parametreleri: options: { accountId: string }.
   * @returns {Promise<AccountView>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, ADMIN_REQUIRED, PERMISSION_DENIED, AUDIT_UNAVAILABLE.
   */
  virtualEnable: (API) => API.admin.virtualUsers.enable({ accountId: "acct_customer" }),

  // API.admin.policy.get({ accountId: "acct_customer" }); — Atanmış kapsamdaki hesabın güvenli politika görünümünü döndürür.
  /**
   * Atanmış kapsamdaki hesabın güvenli politika görünümünü döndürür.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.admin.policy.get({ accountId: "acct_customer" });
   * @description SDK yöntemi parametreleri: options: { accountId: string }.
   * @returns {Promise<PolicyView>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, ADMIN_REQUIRED, PERMISSION_DENIED.
   */
  adminPolicyGet: (API) => API.admin.policy.get({ accountId: "acct_customer" }),

  // API.admin.policy.addRule({ accountId: "acct_customer", path: "production", recursive: true, deny: ["chmod", "delete", "move"] }); — Sadece admin katmanına kural ekler; root tavanını değiştirmez.
  /**
   * Sadece admin katmanına kural ekler; root tavanını değiştirmez.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.admin.policy.addRule({ accountId: "acct_customer", path: "production", recursive: true, deny: ["chmod", "delete", "move"] });
   * @description SDK yöntemi parametreleri: options: { accountId: string, path: string, recursive: boolean, deny: string[] }.
   * @returns {Promise<{ ruleId: string, revision: string }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, ADMIN_REQUIRED, PERMISSION_DENIED, INVALID_RULE, AUDIT_UNAVAILABLE.
   */
  adminPolicyAdd: (API) => API.admin.policy.addRule({ accountId: "acct_customer", path: "production", recursive: true, deny: ["chmod", "delete", "move"] }),

  // API.admin.policy.removeRule({ accountId: "acct_customer", ruleId: "admin_rule", expectedRevision: "policy_revision" }); — Yönetmeye yetkili olduğu admin kuralını kaldırır; root/kullanıcı kurallarını kaldırmaz.
  /**
   * Yönetmeye yetkili olduğu admin kuralını kaldırır; root/kullanıcı kurallarını kaldırmaz.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.admin.policy.removeRule({ accountId: "acct_customer", ruleId: "admin_rule", expectedRevision: "policy_revision" });
   * @description SDK yöntemi parametreleri: options: { accountId: string, ruleId: string, expectedRevision: string }.
   * @returns {Promise<{ revision: string }>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, ADMIN_REQUIRED, PERMISSION_DENIED, REVISION_CONFLICT, AUDIT_UNAVAILABLE.
   */
  adminPolicyRemove: (API) => API.admin.policy.removeRule({ accountId: "acct_customer", ruleId: "admin_rule", expectedRevision: "policy_revision" }),

  // API.admin.quota.getBudget(); — Root atanmış bütçe, alt hesap tahsisleri ve helper kullanım özetini döndürür.
  /**
   * Root atanmış bütçe, alt hesap tahsisleri ve helper kullanım özetini döndürür.
   * @param {Object} API Başlatılacak/başlatılmış gerçek SDK istemcisi.
   * @example API.admin.quota.getBudget();
   * @description SDK yöntemi parametreleri: Parametre yok.
   * @returns {Promise<BudgetView>} Örnek çağrının sonucu.
   * @throws {TealDriveError} AUTH_REQUIRED, ADMIN_REQUIRED, QUOTA_MEASUREMENT_UNAVAILABLE.
   */
  adminBudget: (API) => API.admin.quota.getBudget(),

};

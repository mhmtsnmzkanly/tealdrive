# Operasyon yardımcıları

Bu araçlar geliştirme testlerinden ayrıdır. Gerçek Root oturumu ister; veri/parola/config silmez. Kurulum yönetiminin asıl aracı `tealdrive-wizard`dır.

```sh
# Önce onaylanan kaynak sürümden build:
cargo build --locked --release --bins
sudo python3 tools/ops/package-release.py \
  --bin-dir /root/tealdrive/target/release \
  --output /root/approved-tealdrive-release --version release-20261013
sudo /root/approved-tealdrive-release/tealdrive-wizard install \
  --bundle /root/approved-tealdrive-release

# Mevcut Linux kullanıcısının legacy kökünden NSS home'a no-replace kopyalama:
# Önce backup alın ve dış writer/server'ı durdurun. Kaynak yerinde kalır.
sudo python3 tools/ops/migrate-home.py --user alice --source /srv/old-alice-files
```

Release helper dört ELF ve manifest SHA256 üretir; checksum kaynak güvenini/imzayı sağlamaz, source/build Root tarafından onaylanır. Mevcut output üzerine yazılmaz.

Migration helper her iki kökün seçilen kullanıcıya ait gerçek, çakışmayan dizin olduğunu doğrular. Bütün UID/GID'leri kalıcı düşürüp ek grupları temizledikten sonra kopyalar. Yeni regular file atomik no-replace link ile yayınlanır; farklı mevcut dosyada durur. Symlink korunur, hedefi kopyalanmaz; special file reddedilir. Eski kaynağı veya önce kopyalanmış dosyaları hata halinde silmez. Arbitrary UID/home seçimi veya Root olarak kullanıcı içeriği işleme yapmaz.

# KohdaLab Camera 0.1.23

## GUI control API / GUI 制御 API

Authenticated `control.status` requests now use the session snapshot already
collected for the same GUI update. The pending frame remains available to the
preview and capture logic. The loopback regression test verifies both the
status reply and GUI frame delivery.

認証済みの `control.status` は、同じ GUI 更新で取得したセッションの
スナップショットを参照します。未処理のフレームはプレビューと保存処理に
渡ります。ループバック回帰テストでは、状態応答と GUI へのフレームの
到達を確認します。

## Distribution / 配布

The Windows x64 Setup and portable ZIP are unsigned. The release also includes
a source ZIP and SHA-256 checksums. The USB library remains nusb 0.2.7.

Windows x64 の Setup と portable ZIP は未署名です。ソース ZIP と SHA-256
チェックサムも配布します。USB ライブラリは nusb 0.2.7 を使用します。

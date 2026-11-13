# KohdaLab Camera 1.0.2

Clarify OS setup and the read-only external API, and update the common Python SDK. / OS別導入案内と参照専用APIの説明、共通Python SDKを更新しました。

---

# KohdaLab Camera 0.1.25

## Rendering performance / 描画性能

The v2 renderer now advances source coordinates along each row when the image
needs no downsampling. A box containing one sample emits that sample directly.
The rotation, reflection, Bayer interpolation, color processing and rounding
retain their existing results.

縮小が不要な v2 描画では、行に沿って元画像の座標を進めます。縮小ブロックが
1画素の場合は、その処理済み画素を直接出力します。回転・反転、Bayer 補間、
色処理と丸めの結果は従来どおりです。

On one macOS development machine, the median synthetic 1024 × 768 preview
render time across eight orientations fell from 3.80 to 2.24 ms per frame.
For a 2048 × 1536 input downsampled to a 1024-pixel long edge, the median
was 11.27 ms before and 11.48 ms after. Rendered pixel hashes matched 0.1.24
across all 16 timed cases and the aggregate of 160 additional cases. These
measurements cover CPU rendering of synthetic inputs.

macOS 開発機1台で、1024 × 768 の合成画像を8方向で描画したときの中央値は
1フレームあたり 3.80 ms から 2.24 ms に短縮しました。2048 × 1536 を長辺1024に
縮小した場合の中央値は変更前 11.27 ms、変更後 11.48 ms でした。計測16条件の
画素ハッシュと追加160条件の集約ハッシュは 0.1.24 と一致しました。測定対象は
合成入力の CPU 描画処理です。

## Distribution / 配布

Windows x64 Setup and portable ZIP are unsigned. The source ZIP and SHA-256
checksums accompany the release. The USB library remains nusb 0.2.7.

Windows x64 の Setup と portable ZIP は未署名です。ソース ZIP と SHA-256
チェックサムも配布します。USB ライブラリは nusb 0.2.7 です。

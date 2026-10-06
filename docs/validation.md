# 実機検証記録

## 2026-10-06: USBキュー・画像処理・プレビューの追加高速化

最終EXE: 1024×768で取得・実プレビュー更新とも35.427fps。定常90.016秒で各3,189枚、USB復帰0・プレビューキュー破棄0・欠損破棄20。露光/ゲインの変更と読戻し、PNG/TIFF/センサーTIFFの保存全画素一致、終了コード0。結果はcaptures/fps-optimization/fps-native-portable/。Windowsフルの既定は垂直3/水平1024、Macフルは垂直1023/水平336。両OSとも87テスト成功。

- Windowsの垂直3行で転送サイズ・本数を再比較。64本8KiBは10秒302枚、欠損破棄0・全画素一致。4本128KiB、32本128/256KiB、64本32KiBでは長さが一致してもテスト画像の画素が一致しないものがあり、不採用。
- 64本8KiB・水平336クロックは20秒試験2回で各604枚、欠損破棄0、全画素一致。水平21クロックでは707枚・35.349fps・欠損破棄6・全画素一致。128本4KiBは711枚・35.550fps、128本8KiBは705枚・35.250fps。差が小さいためCPUコールバック回数の少ない64本8KiBを採用し、メーカー仕様の最小水平21クロックへ変更。解像度とビニングは維持。
- 画像変換をuint16の近傍加算とキャッシュした色補正LUTへ変更。従来の補間、段階ごとのuint8切捨て、反射境界、RGB補正順序を維持。旧実装との120条件全画素比較に成功。固定乱数画像・4色順序の変更前ハッシュを自動テストで確認。Macで1024×768・既定補正の処理は約9.79→6.69ms/枚。
- プロセス応答の専用読取りスレッドとQueue通知によりWindowsの短い待機の反復を除去。応答途中の停止・子プロセス終了・不正長の終了も検証。水平336・旧USBキューの通常映像60.044秒試験で1,560枚をアプリへ受渡し（25.98fps）、プレビューキュー破棄0、USB復帰0、露光・ゲイン読戻しとPNG/TIFF/センサーTIFFの全画素一致に成功。
- Qtプレビューは33→16msのPreciseTimer。実際のプレビュー更新を数え、取得速度と区別して画面・GUI検証JSONへ表示する。モニターやRustDeskの表示速度を測る値ではない。
- Macにも垂直3行・画像処理・応答スレッド・プレビューの共通改善を反映。Macの水平336・32本512KiBは維持。カメラはWindowsに接続したままのため、Macの新設定の実機fpsは未測定。フルセンサーは両OSで垂直1023・水平336。行時間が変わるので、露光のms換算は未校正。

- 新しい通常画像試験: 水平21・垂直3・64本8KiBで60.051秒1,925枚（32.06fps）、USB復帰0、欠損破棄7、プレビューキュー破棄125。全解像度2048×1536保存を再生成RGBと全画素比較し、高速ライブへ復帰、USB復帰0。
- ワーカーから元センサーだけを送って親の取得スレッドでRGBを生成する経路へ変更。センサー768KiB+条件を送るため、RGBを含む従来約3MiBから転送量を約1/4に削減。条件は撮影時のスナップショットを使い、float32ホワイトバランスも保存前と一致させる。全画素の旧実装ハッシュ一致を確認。60.040秒2,012枚（33.51fps）、USB復帰0、欠損破棄9、プレビューキュー破棄36、PNG/TIFF/センサーTIFFの全画素一致。
- Mac・Windowsとも全87テスト成功。元センサーだけを送る最終経路でも、2048×1536保存の全画素一致・高速ライブ復帰・USB復帰0を再確認。Mac arm64の自己完結appをビルドし、独立ワーカー・模擬フレーム取得・保存・正常終了・アドホック署名整合性を確認。Mac実機の新設定は未測定。

- 最後に共通Cレンダラーを追加。Bayer補間・色補正LUT・反射境界・回転/反転を1回の処理で実行し、ctypes呼出し中はGILを解放する。Macで1024×768は変更前約9.83→1.91ms/枚。4色順序、カラー/グレー、4回転、各反転、3補正、偶数/奇数寸法の1,920条件で変更前画像と全画素一致。Cと再現ビルドスクリプトを同梱。
- ネイティブDLLを使う通常Qtイベントループ・ソース版90秒試験: 定常90.016秒3,187完全フレーム、取得/実プレビュー更新とも35.405fps。USB復帰0、欠損破棄22、プレビューキュー破棄0。露光/ゲイン変更と復元、PNG/TIFF/センサーTIFFの保存全画素一致。ネイティブ導入前のEXEは取得35.372fpsに対し実プレビュー28.551fps、キュー破棄614だった。

- 最終のフル解像度再試験で完全フレーム待機が失敗。8KiB/64本・フルの従来垂直1023/水平336では、テスト画像の規定長フレームにも画素位相ずれがあり、保存RGBの再生成一致だけでは生データの完全性を証明できなかった。失敗を受入成功に含めない。転送サイズ変更や垂直3だけでは解消せず、フル専用の水平1024・垂直3が20秒176枚・8.800fps・全3,145,728画素一致、欠損破棄5となった。水平2047は65枚・3.250fps、低クロック8001は91枚・4.550fps、どちらも全画素一致。Windowsフル専用に水平1024/垂直3を採用。Macフルは以前の設定を維持する。

- Windowsフルの水平1024/垂直3を30秒ずつ再試験: 263枚（8.767fps）と270枚（9.000fps）、全533枚・全3,145,728画素一致、不正画素0。欠損破棄8/1。通常画像の最終フル解像度保存も成功、RGB再生成全画素一致・高速ライブ復帰・USB復帰0。結果はcaptures/fps-full-confirm/とcaptures/fps-full-profile-final/。当初の保存タイムアウトと不正テスト画像を採用判断から除外した。

## 2026-10-06: Windowsライブfps改善

- メーカーのGR300W.ax / GR300W64.sysとMeasureSを静的解析し、Micron MT9T001のタイミング仕様と照合。Windowsの1024×768ライブで、Reg06のフレーム間待機を1023行から仕様上の最小3行へ変更。解像度・ビニング・露光128行・ゲイン1倍・完全な画像サイズの判定を維持。Macとフルセンサーの既定値は1023行を維持。レジスター読戻しを確認する。
- 既知のセンサーテスト画像を全786,432画素比較: 3行の20秒試験2回で374枚と368枚、全枚一致・USB復帰なし。1023行の比較は78枚。Windowsの1msタイマーを変えない比較も20秒309枚で全枚一致。
- 通常画像・分離ワーカーの90.048秒試験: 1,544枚をアプリへ受渡し（約17.1fps）、最終完全フレームシーケンス2,534。USB復帰0、現在のストリームの欠損破棄93、プレビュー破棄990。露光256行/ゲイン2倍→128行/1倍の読戻し、PNG・TIFF・センサーTIFFの全画素一致、ワーカー終了コード0を確認。キューは32×512KiB RAW_IO、OSタイマー設定は変更しない。
- 再ビルドしたportable EXEのGUI実機試験: 定常90.595秒で完全フレーム2,588枚（28.567fps）、全体93.009秒、終了コード0。1024×768、USB復帰0、現在のストリームの欠損破棄131、プレビュー破棄1,011。露光・ゲインの変更/復元とレジスター読戻し、PNG・TIFF・センサーTIFFの全画素一致を確認。結果とGUI画像はcaptures/windows-fps-portable/。
- GUI表示とUSBの完全フレーム速度は別であり、全シーケンスを表示する保証はない。欠損破棄は残る。画像の補間・パディングや判定緩和は行っていない。
- Mac・Windowsとも全79テスト成功。実験ソース・結果はcaptures/windows-fps-diagnostics/、通常画像の結果はcaptures/windows-fps-source/。解析の根拠はwindows-driver-analysis.mdを参照。

## 2026-10-06: Windowsの低fps切り分け

- GUI・画像処理・プロセス間通信を外し、USB受信のみ測定。32本512KiBは10秒22完全フレーム・106欠損破棄。1msタイマー設定では50完全・79破棄。256KiB/32本は64完全・66破棄。キュー8/32/64本、転送サイズ128/192/256/384/512/768KiB、AboveNormal優先度を比較したが欠損は解消しなかった。短時間の最高値6.4fpsを安定性能や採用設定とは扱わない。
- 切り分け専用のC++ DLLで、Pythonコールバック・画像処理を介さず、同じlibusbハンドルで非同期RAW_IO受信。512KiBでは15.056秒57完全・134破棄、256KiBでは15.046秒80完全・114破棄。各回エラー0、終了時の未完了転送0。データコピーを省いた受信でも改善しなかった。
- C++試験の受信量は約10.0MB/s（1024×768×8bitで約12.8fps分）だが、短パケット境界間のペイロードが規定サイズに届かない画像が多い。Windowsの現接続におけるUSB受信経路の問題が残る。WinUSB/libusb・ホストコントローラー・ハブ・カメラブリッジのどこで欠損するかは未確定。同じlibusbを利用するC++比較だけでWinUSB固有の原因とは断定しない。
- Macの過去実機記録は1024×768で約12.6〜12.9fps。Windows portableの約5fpsは性能面で未達。補間・サイズ判定の緩和・本番設定変更は行っていない。
- 診断ソースと結果はcaptures/windows-fps-diagnostics/に保存。C++ DLLは切り分け用で配布物に含めない。本番ソース・EXEはこの試験で変更していない。

## 2026-10-06: tetra Windows x64 portable実機検証（最新）

- Windows 11 Home build26200、Intel Core i5-1035G1。旧GR300W64.sysはHVCI互換性拒否でエラー39。対象0547:4D33へWindows標準WinUSBを割り当て、デバイスは正常・エラー0。メモリ整合性は有効のまま。旧oem73.infはDocuments/kohdalab-camera/driver-backupへ保存済み。
- MacとWindowsの自動テストは各77件成功。Windows子プロセスの途中EOF、RAW_IO上限・パケット長・有効化失敗時の中止も確認。
- 初期WinUSB通常キューでは欠損が多く、完全フレームを取得できなかった。libusb1.0.30のRAW_IOを、インターフェースclaim後・転送開始前に有効化。上限とUSBパケット長に合わせて転送バッファを制限する。
- ソース版実機: 30.127秒、176フレーム、1024×768、USB復帰0回、欠損破棄168。露光256行・ゲイン2倍への変更と128行・1倍への復元をレジスター読戻しで検証。PNG・TIFF・元センサーTIFFを保存し、読戻し全画素一致。正常終了。
- portable EXE実機: 通常Qtイベントループ、専用セットアップ、同梱camera-worker.exeを通して20.746秒、最終シーケンス126、表示で観測96シーケンス、USB復帰0回、欠損破棄99。露光・ゲイン変更と復元、PNG/TIFF/センサーTIFF保存一致、GUI停止・終了を確認。
- 最終フォント修正版EXE: 20.555秒、最終シーケンス103、表示で観測82シーケンス、USB復帰0回、欠損破棄113。露光・ゲインと保存の検証成功。Windows標準Segoe UIをOSから読み込み、フォント自体は再配布しない。変更後のGUI/IPCテストは両OSで各12件成功。
- どちらも欠損フレームを補間・パディングしていない。画像サイズの判定基準は維持。USB停止は再現しなかったが、現在のWindows接続でフレーム欠落は残る。長時間運転・Windowsフルセンサー撮影は未検証。露光のms換算と光学的ゲイン校正は未検証。
- 低クロック8008では短い比較で欠損0だったが約0.5fps。8001/8002や64/256/1024KiB転送サイズの比較は欠損解消にならず、現在は検証済み8000・32本512KiBを使用。
- 再現・証拠: scripts/validate_windows_capture.py、portableの--validate-hardware、captures/windows-raw-io/validation.json、captures/windows-portable/validation.jsonとgui.png。撮影結果はGit対象外。
- 配布はEXE、同梱Python/Qt/libusb、WinUSBセットアップ、交換可能なlibwdi DLL、ライセンスと完全な対応ソースを含む。セットアップ対象は0547:4D33のみ。OEMドライバーがある場合は自動エクスポートに成功してから置換する。


## 2026-10-06: 映像パイプ局所復帰と高設定の維持（最新）

- 単発の映像エラーでカメラ全体をリセットする処理を見直し、待機転送の完了後にEP82の停止を解除して受信だけを再開する。センサーとブリッジを止めず、露光・ゲインを維持。status1だけでなくSTALL/status4とsubmit -9も対象。受信を準備してから映像送信を開始する。
- バースト状のエラーは2秒以内・最大32回の局所復帰を試行。改善しない場合の全体再初期化と低設定への復帰は残す。初期の短い再試行条件では長めの試験が失敗し、その失敗を受入成功には含めていない。
- `validate_pipe_controls.py --execute --hold-seconds 40` / `captures/pipe-controls/validation.json`: **173.905秒・2,100枚・設定維持・USBデバイスリセット0回・正常終了**。128行/1倍→300行/2倍→600行/4倍→1200行/8倍→128行/1倍。高設定は各40秒。各段階の実センサー読取りを照合、保存PNGは元センサーデータから再生成して全画素一致。
- 映像パイプの局所復帰は71回。USB転送エラーがなくなったわけではなく、失われたフレームは復元していない。結果は、このハブ接続の3分弱の運転継続と設定維持の検証であり、長時間無人運転やUSBリンクそのものの改善の証明ではない。
- `captures/full-still-pipe/validation.json`: 300行/2倍を実センサーで確認して2048×1536保存、RGB全画素一致、1024×768ライブへ復帰。局所復帰1回。
- 自動テスト74件成功。読取りアドレス、復帰順序、設定維持、連続エラーと再試行上限、STALL/-9処理を含む。

## 2026-10-06: ハブを変えない追加診断

- メーカーと同じ128KiB/4本、開始順序、低速クロック、設定同期、旧libusb、macOS標準IOUSBHost API、ブリッジ再同期、ソフトウェアトリガ、640×480を比較。いずれも高露光・高ゲインの映像受信エラーを解消しなかった。通常の受信方式は変更していない。詳細はcamera-software-research.mdへ記録。
- レジスター読取りのアドレスをwValueからwIndexへ訂正。チップIDは0番のため以前の試験では誤りを検出できなかった。起動時に露光・ゲインを実センサーから読み戻して照合し、受信障害時にも状態を記録する。
- `captures/pipe-diagnosis-final/diagnosis.json`: 300行/2倍で79枚・7.192秒後に映像受信エラー。障害直前/直後の07=0002、08=0000、09=012C、35=0010、0A=8000、05=0150、06=03FF、1E=8040は一致。制御応答は正常。設定値が勝手に変わった状態でも、機器全体が切断された状態でもない。原因がハブと確定したわけではない。
- `captures/diagnosed-controls/validation.json`: 画像補正と保存一致、600行/2倍の実レジスター照合、1回の受信エラーから128行/1倍へ復帰して530枚までライブ継続、正常終了。これはUSB停止の解消を意味しない。
- 自動テスト69件成功。

## 2026-10-06: 読めるメニューと低転送量モード（最新）

- macOSネイティブメニューとカスタム配色の組合せで選択肢が小さくなる表示を、Qt Fusion・最小幅・明示的なメニュー配色で修正。文字を10pt、設定幅を330–360pxへ変更。タブ内容はスクロール可能。
- 標準カメラモードをメーカーの2×ビニング（0x22/0x23=0x11）、垂直ブランキング1023へ変更。視野を維持して出力1024×768、転送量はフル画像の1/4。180度回転と生センサー保存を維持。フル2048×1536は接続前に選択可能。
- `captures/vendor-binning-live/validation.json`: **180.151秒・2,327フレーム・USB復帰0回・正常終了**。通常Qtイベントループ、同じハブ接続で検証。
- `captures/full-still/validation.json`: 2048×1536 RGB静止画を保存、元センサーTIFFから再生成したRGBと全画素一致。1024×768ライブへ復帰、USB復帰0回。`validate_full_still.py --execute`で通常Qtイベントループから操作。
- 高露光・高ゲインは、低転送量モードでも停止を再現。`captures/mode-controls/` と `captures/mode-controls-reset/` は不合格。設定変更時のUSBリセットは改善せず、採用していない。起動前に300行/2倍、600行/4倍を指定した試験でも停止した。
- USB受信エラーの復帰には、転送終了・デバイス解放後に対象カメラのUSBリセットを追加。完了前のバッファ/ハンドルを解放しない。リセット回数と失敗理由をメタデータに記録。
- 画像処理分離、4本転送、64KiB転送、同期読み、フル画像の周期延長だけでは改善せず、実験コードは除去。採用受信方式は元の32本×512KiB。
- USB停止原因の完全な解決は未達。別ポート/別ハブ/直接接続の比較が必要で、接続経路が原因だと確定したわけではない。
- ソフトウェアテスト: 67件成功（設定モード、幅のあるメニュー、スクロール、USBリセット順序を追加）。



## 2026-10-06: kohdalab-ivに合わせたUIと180度回転（最新）

- kohdalab-iv `src/kohdalab_iv/apps/iv_gui.py`（main）を確認。黒/グレーの配色、9pt文字、小さい余白、グループ化された操作ボタン、左設定／中央表示／右ログの配置をカメラ用に反映。
- 左設定と右ログを矢印で折りたたみ可能。既存の英語タブと隠されたAdvanced設定を維持。数値入力はkohdalab-iv同様、編集途中に反映しない方式。
- 回転は180度固定。回転選択欄を削除。画像補正後のRGB表示・保存へ適用し、元センサーTIFFは変更しない。
- 自動テスト: **63 passed**。180度設定、回転選択欄の削除、設定/ログパネルの折りたたみを含む。
- 実機確認の画像と保存一致検証: `captures/iv-style-final/`。


## 2026-10-06: 英語UIと明るさ・反転の改善

- GUIを英語へ統一し、Capture / Image / Export / Advancedへ整理。露光・アナログゲイン・接続APIはAdvancedへ移動。
- 初期値は露光128行、アナログゲイン1倍、画像Brightness +1.5 EV、Gamma 2.2。左右反転、上下反転、時計回り0/90/180/270度の回転を追加。
- `captures/refined-ui-live/validation.json`: 実際のQtイベントループ90.907秒、985フレーム、USB復帰0回、正常終了。テスト中の画面はハードウェア項目移動前の版。
- `captures/refined-controls/validation.json`: 45秒、449フレーム。Brightness +2 EV・左右/上下反転・90度回転のRGBを元センサーデータから再生成し、保存PNGと全画素一致。センサーTIFFは変換前データと一致。
- 同試験で600行/2倍を適用するとUSBエラーが発生。128行/1倍へ戻して1回の復帰後、ライブ表示を継続。要求値・適用値・復帰回数をJSONへ保存し、UIの値も実際の復帰設定へ戻す。
- 単独設定比較でも300/600行、2倍以上のゲインでUSB停止を再現。8001/8002/8008クロックや水平ブランキング増加では解消しなかった。実験設定は採用していない。ハブはFresco Logic、480Mbps。直接接続での比較は行っていない。
- **高露光・高ゲインのUSB停止原因は未解決**。新しい明るさ機能はソフトウェア処理であり、センサーの露光を増やしたり、失われた情報を復元するものではない。
- 最新の自動テスト: **62 passed**。画像処理、ゲイン符号化、設定失敗時の復元、USB停止時の安定設定復帰、英語タブ構成を含む。

以下は以前の版での検証履歴。露光65535行や生ゲイン値を使うGUIは現在の仕様とは異なる。


2026-10-06、macOS arm64、Python 3.14.6、uv。接続機器: SHODENSHA 3.1M USB2.0 Camera、0547:4D33、MT9T001 chip1621、独自USB bulk-IN82。UVCではありません。

## 開発途中の設定・保存・停止の機能確認（最新結果は末尾）

- `uv run --frozen --extra dev --extra usb --extra gui pytest -q`: **53 passed**。取得・保存・GUI終了、制御の検証、Bayer変換、センサー元データの保持、USBエラー回復、キャンセル完了後の解放を検証。
- `validate_legacy_endurance.py --execute --cycles 4 --seconds 30 --output captures/endurance-final`: **4接続、合計568枚、120秒、failure=null、全回停止成功**。センサー2048×1536、通常約5.5fps。自発的USBエラー・自動復帰は0回。
- 各接続で128行/ゲイン8 → 128行/16 → 256行/8 → 64行/8をGUIから適用。各回PNGとTIFFを1枚ずつ、合計8画像＋8センサーTIFF＋8JSON。保存RGBを元データから再生成し、全画素一致を確認。
- 1回目の平均センサー値: 27.66 → 45.47（ゲイン2倍）→ 48.04（露光2倍）→ 17.55（露光半分）。4回とも同様の応答。
- `validate_legacy_features.py --execute`: ホワイトバランス、グレー/カラー切替、表示ガンマ1/2.2、RGB保存とセンサーデータ照合を確認。
- 同試験で意図的なUSB status1エラーを注入し、実機のキャンセル・切断・再初期化・画像取得まで復帰。これは意図的な故障注入であり、自発的エラーの観測回数には含めない。
- 最大露光65535行の設定中に停止し、**0.12秒でワーカー終了**。再接続後、ウィンドウを1回閉じる操作で撮影終了・USB解放・画面終了まで確認。

実機試験は明示的な`--execute`が必要です。ファームウェア・EEPROMは書き換えず、揮発性センサー設定とUSBブリッジだけを操作しています。

## カラーとフレーム位置

- 3枚のセンサー内蔵テスト画像は、全画素が偶数列64・奇数列191の交互パターンに一致。2048×1536の8bit配列としての切出しを確認。
- `validate_bayer_phase.py --execute`: 全色ゲイン8の基準画像と、青レジスター2C=32・赤2D=16の画像を比較。2×2画素群の中央値は基準 `[[17,14],[14,17]]`、変更後 `[[17,19],[32,17]]`。
- 左下画素だけ青ゲインに、右上画素だけ赤ゲインに応答。USBで受け取る配列は **GRBG** と確定。
- RGBは双線形補間、任意のホワイトバランス、表示ガンマで生成。センサーTIFFは変換前の8bitをそのまま保存。元の10bitセンサー値すべてが保存されるわけではない。

## 転送停止への対策と経緯

初期の低速クロック8008では、macOSログに`device not responding (0xe00002ed)`が出て、開始や設定再開で停止する試行があった。純正ドライバーと同じ8001に戻し、待機転送を4本から16本に増加、明示的キャンセルと最大3回の再初期化を追加した。低速設定だけが根本原因とは断定しない。

その前の標準クロック・グレー版でも、3回×60秒、930枚と6保存が成功した（captures/endurance-vendor-clock）。カラー追加後の中間版では、1回目310枚が成功し、2接続目でstatus1が発生。有限回復処理を追加した後の当該版は上記568枚・4接続を完了した。成功した試験だけを残すことはせず、この失敗経緯を記録する。

回復中は新しい画像の取得が止まり、受信できていない画像を補完しない。復帰回数と最終転送エラーは画面・JSONに記録。回復できないエラーや機器消失は撮影を停止して表示する。

## 未検証の範囲

Windows 11実機、色標準による色精度校正、実露光時間のms校正、長時間の無人運転、物理的な抜線・再挿入による自動復帰は未検証。メーカーWindowsドライバーを使う場合はOpenCV/DirectShow経由を先に確認し、Zadig等で置き換えない。macOSでのこの実機試験をWindowsの合格とは扱わない。

動画記録、計測・スケール校正、署名済みアプリ配布、16bit保存は今回の実装範囲外。

## 通常のQtイベントループによる追加検証

手動processEventsを使うGUI試験だけでは不十分だったため、`validate_live_event_loop.py`で実際の`QApplication.exec()`による連続ライブを検証した。描画を新しい画像だけに限定し、macOSの撮影用Foundation activity、USB取得のプロセス分離、32本の待機転送、開始前のclear_halt(82)を追加した。activityは撮影終了時に解除し、システム・ディスプレイのアイドルスリープ設定は変更しない。

- 描画限定のみ: 180秒910枚、復帰10回、最後まで撮影・正常停止。
- 8002クロック・32転送でも転送停止が再発。省電力activityだけでも解消しなかった。原因をApp Napと断定しない。
- プロセス分離＋clear_halt追加: 120秒643枚、復帰3回、最後まで撮影・正常停止。画面・保存の機能は使えるが、無中断運転の合格とは扱わない。
- プロセス分離版 `validate_legacy_features.py --output captures/features-process-final`: 色補正、グレーPNG、カラーTIFFと元データ照合、意図的エラーからの実USB復帰が成功。長い露光中の停止0.293秒、再接続・自動ウィンドウ終了も成功。

IORegistryではFresco Logic製USB2.0ハブの下に対象カメラを検出し、リンクは480Mbps。接続経路の影響を切り分けるため、USB抜き差しと可能なら直接接続での比較をユーザーに依頼した。ハブが原因と確定したわけではない。WindowsでのUSBトレース比較や別経路の試験はまだ行っていない。

## 高速設定8000による改善

メーカーfilterのSpeedフラグが選ぶ高速設定8000（除算なし）を試したところ、通常のQtイベントループで90秒・968枚、自発的USB停止/復帰0回、正常停止を確認（captures/live-event-loop-fast-clock）。これを既定設定に変更した。

`validate_legacy_endurance.py --execute --cycles 4 --seconds 30 --output captures/endurance-fast-final`: **4接続、1,126枚、120秒、8保存、失敗なし、復帰0回**。RGB保存をセンサー元データから再生成して全画素一致、露光/ゲインの明るさ変化、停止・再接続が成功。通常約11fps。

低速設定の転送停止を省電力やハブだけが原因と断定しない。現在のハブ経由接続のまま高速設定で上記試験が成功している。元の環境変数KOHDA_CAMERA_CLOCKは実験用の上書きで、通常は指定しない。

## 最終受入: 通常のライブ運転（2026-10-06）

`validate_live_event_loop.py --execute --seconds 180 --require-clean --output captures/live-final-clean`: **180.753秒、1,976枚、約11fps、失敗なし、USB停止・復帰0回、正常終了**。実際の`QApplication.exec()`を使い、自発的な復帰が1回でもあれば不合格にする条件で成功した。高速8000、32本の512KiB受信、USB撮影の独立プロセス、カラーGRBG・表示ガンマ2.2が最終構成。

最新ソフトウェア検証: **53 passed**。プロセス間フレーム受渡し、子プロセス異常終了、撮影中の終了要求、活動状態の解除を含む。GUIで露光・ゲインの値は停止後にも保持する。

機能受入の成功は、このMacと接続中の実機・現在のハブ経由接続における結果。Windows実機・長時間無人運転・色精度・露光ms校正の未検証範囲は変わらない。

高速最終版の機能確認（captures/features-fast-final）も成功。ホワイトバランス、グレーPNG/カラーTIFFの画素照合、意図的エラーからの復帰、最大露光中の停止0.405秒、再接続・ウィンドウ終了を確認。通常運転の復帰0回と、意図的な障害注入の復帰は区別する。

設定変更が過去の取得フレームの撮影条件を書き換えないよう、親プロセスの制御キャッシュをdeep copyに修正。回帰テストを追加し、最新53テスト成功。実機10枚・露光変更・PNG/センサーTIFF/JSONの保存も再確認した。

最終GUI試験（captures/gui-final-controls-complete）では、露光128→256→64を実画像のメタデータで確認。画面更新で連番が飛んでも、全設定の反映と2保存が完了するまで検証を続ける。
# Smooth-display recovery comparison — 2026-10-06

The user chose 10–13fps over the experimental ~6fps clock. Alternating same-hub acquisition trials at 300 rows/2x compared the existing 32 × 512KiB queue and an eight-request candidate, twice each for 40s. Existing: 13 endpoint recoveries / 1,027 frames in total; candidate: 19 / 1,022. Delivered rates were 12.70–12.88fps, with zero bus resets and retained sensor controls. The candidate is not adopted. See `captures/recovery-normal/results.json` and `docs/camera-software-research.md` for the additional transfer-size, clock, and restart-delay trials and their failures.

The application was then relaunched at its existing stable defaults: 128 exposure rows, 1x analog gain, software brightness +1.5EV, fixed 180° rotation, 1024x768 preview, same Fresco Logic hub. Native Qt validation at 60.794s recorded 766 complete frames, 12.60fps including startup, **zero endpoint recoveries and zero bus resets**. Exposure/gain readbacks matched 128/8. Exported RGB exactly matched the displayed frame and reconstructed sensor image with 180° rotation. Mean displayed RGB was 128.20. Results and screenshots: `captures/smooth-stable-final/launch.json`, `live.png`, and `format-menu.png`. The GUI remains open at these settings.

This short run verifies a usable low-control profile, not indefinite stability or a correction of the underlying high-control transfer error. Software brightness changes preview/RGB export, not captured sensor data or optical signal. High exposure/gain remain available in Advanced; the fast-clock recovery rate at those settings has not been reliably reduced. The new opt-in `scripts/compare_usb_queues.py` records future alternating trials without changing application defaults; its command-line import/help was checked with the frozen uv environment.

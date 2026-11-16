"""#342 の実機用 JWW。公開のデータ形式から組み立て、製品の資源を使わない。"""

import struct


def jww_header():
    """JWW 700、A3、グループ0の縮尺1/100。表示と印刷の設定も明示する。"""
    # Jw_cad 7.02 の公開説明 https://www.jwcad.net/jwdatafmt.txt
    # 「色番号ごとの画面表示色、線幅」「色番号ごとのプリンタ出力色、線幅、実点半径」。
    # 以下は検証用に選んだ値であり、Jw_cad の既定値を再現するものではない。
    out = bytearray(b"JwwData.")

    def values(fmt, *items):
        out.extend(struct.pack("<" + fmt, *items))

    values("I", 700)
    out.append(0)  # 空の CString（メモ）
    values("II", 3, 0)  # A3・書き込むグループ0
    for group in range(16):
        values("IIdI", 3 if group == 0 else 0, 0, 100.0, 0)
        for layer in range(16):
            values("II", 3 if group == layer == 0 else 0, 0)
    out.extend(bytes(80))  # 軸角・寸法の設定。寸法は置かない。
    values("I", 100)  # 描画する線の最大幅
    values("3dII5d", 0.0, 0.0, 1.0, 0, 0, 0.0, 10.0, 10.0, 0.0, 0.0)
    out.extend(bytes(256 + 16))  # レイヤ名・グループ名の空の CString
    out.extend(bytes(28 + 16 + 4))  # 日影・天空・2.5Dの未使用設定
    values("3d", 0.5, 0.0, 0.0)  # 保存時の画面倍率・原点
    values("3d", 0.5, 0.0, 0.0)  # 記憶した範囲の倍率・原点
    for _ in range(8):
        values("3dI", 0.5, 0.0, 0.0, 0)  # マークジャンプ
    out.extend(bytes(56))  # 文字の描画状態
    out.extend(bytes(80 + 8))  # 複線の設定

    # COLORREF は 0x00BBGGRR。白背景で、ペン1〜8を黒、9をグレーにする。
    colors = [0x00FFFFFF, *([0] * 8), 0x00808080]
    for color in colors:
        values("II", color, 1)  # 画面線幅1（説明の範囲は1〜16）
    for color in colors:
        values("IId", color, 1, 0.2)  # 印刷線幅1・実点半径0.2 mm
    out.extend(bytes(8 * 16 + 5 * 20 + 4 * 16))  # 未使用の破線・ランダム線
    out.extend(bytes(10 * 4))  # 実点の指定半径描画は無効。ほかの印刷フラグも0。
    values("I", 10)  # 印刷フラグの11番目: 十の位1は300 dpi（Ver.6.00以降）
    out.extend(bytes(60 + 32 + 8))  # 作図時間・2.5D・最終寸法・ソリッド設定

    # 未使用のSXF拡張ペンにも、有効な幅と点半径を書く。
    for _ in range(257):
        values("II", 0, 1)
    for _ in range(257):
        out.append(0)  # 空の名前
        values("IId", 0, 1, 0.2)
    out.extend(bytes(33 * 16))  # SXF線種のパターン
    for _ in range(33):
        out.append(0)
        values("I10d", 0, *([0.0] * 10))  # 名前・セグメントのない線種
    for _ in range(10):
        values("3dI", 3.0, 3.0, 0.0, 1)  # 文字種の幅・高さ・間隔・色
    values("3dII", 3.0, 3.0, 0.0, 1, 1)  # 書き込む文字
    values("2dI6d", 1.0, 10.0, 0, *([0.0] * 6))  # 文字位置整理・基準点ずれ
    return bytes(out)


def jww():
    """既存キットと同じ、用紙の(0,0)から(100,50)への線分を1つ置く。"""
    # 公開説明の CDataSen と CArchive。クラス1・オブジェクト2を持つ一覧。
    tag = struct.pack("<3H", 0xFFFF, 1, 8) + b"CDataSen"
    base = struct.pack("<IB5H", 0, 1, 1, 0, 0, 0, 0)
    line = struct.pack("<4d", 0.0, 0.0, 100.0, 50.0)
    return jww_header() + struct.pack("<H", 1) + tag + base + line + bytes(6)
